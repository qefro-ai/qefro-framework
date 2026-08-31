use crate::audit::AuditLogger;
use crate::custom_fields::CustomFieldStore;
use crate::jobs::{JobQueue, JobRegistry};
use crate::operation::{
    available_for_record, crud_operation_defs, execute_operation_with, operation_allowed,
    ExecuteOpts, OperationRegistry,
};
use crate::outbox::Outbox;
use crate::repository::{record_id, EntityRepository, Page};
use chrono::Utc;
use qefro_auth::AuthService;
use qefro_core::{
    apply_entity_rules, canonicalize_datetime, existence_rules, is_person_link_field,
    person_backref_field, reject_readonly_writes, sanitize_html, strip_computed_fields,
    strip_secrets, strip_server_managed_fields, validate_party, validate_record, EntityRegistry,
    FieldError, FieldType, HookRegistry, OpContext, OperationDef, QefroError, QefroResult,
    PERSON_ENTITY, PERSON_LINK_FIELD, RELATED_ID_FIELD, RELATED_TYPE_FIELD, STATUS_CANCELLED,
    STATUS_COMPLETED, USER_ENTITY,
};
use qefro_events::{DomainEvent, InProcessEventBus};
use qefro_permissions::{Action, PermissionRegistry};
use qefro_search::Query;
use qefro_workflow::WorkflowRegistry;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Shared business operation layer used by HTTP and agent tools.
/// Authorization, tenant scoping, validation, workflow, hooks, events, and
/// audit all happen here — never in the transport.
#[derive(Clone)]
pub struct EntityService {
    pub(crate) registry: Arc<EntityRegistry>,
    pub(crate) repo: Arc<EntityRepository>,
    pub(crate) permissions: Arc<PermissionRegistry>,
    workflows: Arc<WorkflowRegistry>,
    hooks: Arc<HookRegistry>,
    events: InProcessEventBus,
    pub(crate) audit: Arc<AuditLogger>,
    pub(crate) activity: Arc<crate::activity::ActivityStore>,
    operations: Arc<OperationRegistry>,
    jobs: Arc<JobQueue>,
    job_handlers: Arc<JobRegistry>,
    outbox: Outbox,
    identity: Option<Arc<AuthService>>,
    custom_fields: Arc<CustomFieldStore>,
}

impl EntityService {
    pub fn new(
        pool: PgPool,
        registry: Arc<EntityRegistry>,
        permissions: Arc<PermissionRegistry>,
        workflows: Arc<WorkflowRegistry>,
        hooks: Arc<HookRegistry>,
        events: InProcessEventBus,
    ) -> Self {
        Self {
            jobs: Arc::new(JobQueue::new(pool.clone())),
            repo: Arc::new(EntityRepository::new(pool.clone())),
            audit: Arc::new(AuditLogger::new(pool.clone())),
            activity: Arc::new(crate::activity::ActivityStore::new(pool.clone())),
            outbox: Outbox::new(pool.clone()),
            custom_fields: Arc::new(CustomFieldStore::new(pool)),
            registry,
            permissions,
            workflows,
            hooks,
            events,
            operations: Arc::new(OperationRegistry::new()),
            job_handlers: Arc::new(JobRegistry::new()),
            identity: None,
        }
    }

    pub fn with_operations(mut self, operations: Arc<OperationRegistry>) -> Self {
        self.operations = operations;
        self
    }

    pub fn with_jobs(mut self, jobs: Arc<JobQueue>, handlers: Arc<JobRegistry>) -> Self {
        self.jobs = jobs;
        self.job_handlers = handlers;
        self
    }

    pub fn with_identity(mut self, identity: Arc<AuthService>) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_custom_fields(mut self, store: Arc<CustomFieldStore>) -> Self {
        self.custom_fields = store;
        self
    }

    pub fn custom_fields(&self) -> Arc<CustomFieldStore> {
        self.custom_fields.clone()
    }

    /// Base EntityDef plus tenant custom fields. Application custom fields are already on the registry.
    pub async fn entity_for(
        &self,
        ctx: &OpContext,
        name: &str,
    ) -> QefroResult<Arc<qefro_core::EntityDef>> {
        let base = self.registry.get(name)?;
        let extras = self
            .custom_fields
            .list_effective(ctx.tenant_id, &base.name)
            .await?;
        if extras.is_empty() {
            return Ok(base);
        }
        Ok(Arc::new(qefro_core::merge_custom_fields(
            base.as_ref(),
            extras.as_ref(),
        )?))
    }

    pub(crate) fn identity_service(&self) -> Option<&AuthService> {
        self.identity.as_deref()
    }

    pub fn registry(&self) -> &EntityRegistry {
        &self.registry
    }

    pub fn permissions(&self) -> &PermissionRegistry {
        &self.permissions
    }

    pub fn workflows(&self) -> &WorkflowRegistry {
        &self.workflows
    }

    pub fn events(&self) -> &InProcessEventBus {
        &self.events
    }

    pub fn audit(&self) -> &AuditLogger {
        &self.audit
    }

    pub fn pool(&self) -> &PgPool {
        self.repo.pool()
    }

    pub fn operations(&self) -> &OperationRegistry {
        &self.operations
    }

    pub fn job_queue(&self) -> Arc<JobQueue> {
        self.jobs.clone()
    }

    pub fn job_handlers(&self) -> Arc<JobRegistry> {
        self.job_handlers.clone()
    }

    pub fn outbox(&self) -> &Outbox {
        &self.outbox
    }

    pub async fn dispatch_outbox(&self) -> QefroResult<usize> {
        self.outbox.dispatch_pending(&self.events, 100).await
    }

    pub async fn availability(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        params: &std::collections::HashMap<String, String>,
    ) -> QefroResult<Value> {
        crate::scheduling::availability(self, ctx, entity_name, params).await
    }

    pub async fn execute(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        name: &str,
        input: Value,
    ) -> QefroResult<Value> {
        self.execute_with(ctx, entity_name, id, name, input, ExecuteOpts::default())
            .await
    }

    pub async fn execute_with(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        name: &str,
        input: Value,
        opts: ExecuteOpts,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        reject_client_tenant(&input)?;
        let (record, _events) = execute_operation_with(
            &self.repo,
            &self.registry,
            &self.permissions,
            &self.workflows,
            &self.hooks,
            &self.operations,
            &self.jobs,
            &self.audit,
            &self.activity,
            ctx,
            &entity.name,
            id,
            name,
            input,
            opts,
        )
        .await?;
        let _ = self.dispatch_outbox().await;
        if record.get("id").is_none() {
            return Ok(record);
        }
        let mut presented = self.present(ctx, &entity, record.clone()).await?;
        if let Some(op) = record.get("_operation") {
            if let Some(obj) = presented.as_object_mut() {
                obj.insert("_operation".into(), op.clone());
            }
        }
        Ok(presented)
    }

    pub async fn get_operation_run(
        &self,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<crate::operation_run::OperationRun> {
        crate::operation_run::OperationRunStore::new(self.pool().clone())
            .get(ctx, id)
            .await
    }

    pub fn list_operations(&self, ctx: &OpContext) -> Vec<OperationDef> {
        let mut out = Vec::new();
        for entity in self.registry.list() {
            if !ctx.allows_app(entity.module.as_deref()) {
                continue;
            }
            for def in crud_operation_defs(&entity) {
                let action = match def.kind.as_str() {
                    "create" => Action::Create,
                    "get" => Action::Read,
                    "find" => Action::List,
                    "update" => Action::Update,
                    "delete" => Action::Delete,
                    _ => Action::Update,
                };
                if self.permissions.check(ctx, &entity.name, action).is_ok() {
                    out.push(def);
                }
            }
            for binding in self.operations.for_entity(&entity.name) {
                if operation_allowed(&self.permissions, ctx, &binding.def) {
                    out.push(binding.def.clone());
                }
            }
        }
        out
    }

    pub fn record_actions(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record: &Value,
    ) -> Vec<OperationDef> {
        available_for_record(
            &self.operations,
            &self.permissions,
            &self.workflows,
            ctx,
            entity_name,
            record,
        )
    }

    pub async fn list(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        query: Query,
    ) -> QefroResult<Page> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        if entity.name == USER_ENTITY {
            return self.list_users(ctx, query).await;
        }
        let mut query = query.sanitize(&entity)?;
        resolve_query_placeholders(&mut query, ctx);
        self.apply_row_policy_filters(ctx, &entity, &mut query);
        let mut page = self.repo.list(&entity, ctx, &query).await?;
        for item in &mut page.items {
            coerce_numeric_json(&entity, item);
            strip_secrets(Some(&entity), item);
            self.strip_forbidden_fields(ctx, &entity, item);
        }
        self.expand_many_to_one_batch(ctx, &entity, &mut page.items)
            .await?;
        self.attach_attachment_counts(ctx, &entity, &mut page.items)
            .await;
        for item in &mut page.items {
            self.attach_workflow(ctx, &entity, item);
        }
        Ok(page)
    }

    pub async fn get(&self, ctx: &OpContext, entity_name: &str, id: Uuid) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Read)?;
        if entity.name == USER_ENTITY {
            let record = self.get_user(ctx, id).await?;
            return self.present(ctx, &entity, record).await;
        }
        let record = self.repo.get(&entity, ctx, id).await?;
        self.enforce_row_policy(ctx, &entity, &record)?;
        self.present(ctx, &entity, record).await
    }

    pub async fn create(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        mut data: Value,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Create)?;
        if entity.name == USER_ENTITY {
            return self.create_user(ctx, data).await;
        }
        if entity.singleton {
            if self.repo.get_singleton(&entity, ctx).await?.is_some() {
                return Err(QefroError::conflict(format!(
                    "{} is a singleton; use PATCH /api/v1/settings/{}",
                    entity.name, entity.slug
                )));
            }
        }
        reject_client_tenant(&data)?;
        self.reject_forbidden_writes(ctx, &entity, &data)?;
        validate_party(&entity, &data, None)?;
        if entity.name == PERSON_ENTITY {
            self.link_person_account(ctx, &mut data).await?;
        }
        let children = extract_children(&entity, &mut data);
        strip_computed(&entity, &mut data);
        prepare_record(&entity, &mut data, ctx);
        if let Some(wf) = self.workflows.for_entity(&entity.name) {
            if let Some(status) = data.get(&wf.field).and_then(|v| v.as_str()) {
                if !status.is_empty() && status != wf.initial {
                    return Err(QefroError::bad_request(format!(
                        "field '{}' is workflow-managed; use a transition",
                        wf.field
                    )));
                }
            }
            if let Some(obj) = data.as_object_mut() {
                obj.insert(wf.field.clone(), json!(wf.initial));
            }
        }
        validate_record(entity.business_fields(), &data, false)?;
        apply_entity_rules(entity.business_fields(), &entity.validation, &data, false)?;
        self.check_relation_existence(ctx, &entity, &data).await?;
        self.check_assignment(ctx, &entity, &data).await?;
        self.validate_child_payloads(ctx, &entity, &children, false)
            .await?;
        self.check_uniques(ctx, &entity, &data, None).await?;
        self.hooks
            .before_create(ctx, &entity.name, &mut data)
            .await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        if let Err(e) = crate::scheduling::enforce_in_tx(
            &mut tx,
            &self.repo,
            &self.registry,
            ctx,
            &entity,
            &data,
            None,
        )
        .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Some(naming) = &entity.naming {
            if naming.assign_on != "submit" {
                let number = crate::numbering::allocate(
                    &mut tx,
                    ctx.tenant_id,
                    &entity.name,
                    naming,
                    Utc::now(),
                )
                .await?;
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(naming.field.clone(), json!(number));
                }
            }
        }
        let created = match self.repo.insert_tx(&mut tx, &entity, ctx, data).await {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        let id = record_id(&created)?;
        let stored_children = match self
            .write_children(&mut tx, ctx, &entity, id, children, true)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        let created = match self
            .recalculate_computed(&mut tx, ctx, &entity, created, &stored_children)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(
                    &mut tx,
                    ctx,
                    &entity.name,
                    Some(id),
                    "create",
                    None,
                    Some(&created),
                )
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = self
            .write_activity_tx(
                &mut tx,
                ctx,
                &entity,
                id,
                crate::activity::TYPE_CREATED,
                None,
                Some(&created),
                None,
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        let mut events = mutation_events(
            &entity.name,
            id,
            ctx,
            created.clone(),
            "created",
            "entity.created",
        );
        if crate::activity::assignment_changed(None, Some(&created)) {
            events.push(
                DomainEvent::new(
                    format!("{}.assigned", snake(&entity.name)),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({ "assigned_to": created.get("assigned_to") }),
                )
                .with_user(ctx.user_id),
            );
            events.push(
                DomainEvent::new(
                    "entity.assigned".to_string(),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({ "assigned_to": created.get("assigned_to") }),
                )
                .with_user(ctx.user_id),
            );
        }
        if let Err(e) = Outbox::enqueue_many_tx(&mut tx, &events).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) = self
            .enqueue_due_reminder_tx(&mut tx, ctx, &entity, &created)
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) =
            crate::scheduling::enqueue_reminder_tx(&self.jobs, &mut tx, ctx, &entity, &created)
                .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        self.hooks.after_create(ctx, &entity.name, &created).await?;
        let _ = self.dispatch_outbox().await;
        Ok(self.present(ctx, &entity, created).await?)
    }

    pub async fn update(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        mut patch: Value,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Update)?;
        if entity.name == USER_ENTITY {
            return self.update_user(ctx, id, patch).await;
        }
        reject_client_tenant(&patch)?;
        self.reject_forbidden_writes(ctx, &entity, &patch)?;
        let children = extract_children(&entity, &mut patch);
        strip_computed(&entity, &mut patch);
        canonicalize_values(&entity, &mut patch, ctx);
        sanitize_values(&entity, &mut patch);
        let current = self.repo.get(&entity, ctx, id).await?;
        self.enforce_row_policy(ctx, &entity, &current)?;
        reject_readonly_writes(entity.business_fields(), Some(&current), &patch)?;
        if let Some(expected) = patch
            .as_object_mut()
            .and_then(|o| o.remove("_expected_updated_at"))
        {
            let current_ts = current
                .get("updated_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let expected = expected.as_str().unwrap_or("");
            if !expected.is_empty() && current_ts != expected {
                return Err(QefroError::conflict(
                    "Record changed by another user. Reload before saving.",
                ));
            }
        }
        validate_party(&entity, &patch, Some(&current))?;
        if let Some(doc) = &entity.document {
            let status = current.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if doc.is_locked(status) {
                self.reject_locked_writes(&entity, &patch, &children)?;
            }
        }
        if let Some(wf) = self.workflows.for_entity(&entity.name) {
            if let Some(obj) = patch.as_object_mut() {
                if obj.contains_key(&wf.field) {
                    return Err(QefroError::bad_request(format!(
                        "field '{}' is workflow-managed; use a transition",
                        wf.field
                    )));
                }
            }
        }
        validate_record(entity.business_fields(), &patch, true)?;
        let mut merged = current.clone();
        if let (Some(dst), Some(src)) = (merged.as_object_mut(), patch.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        apply_entity_rules(entity.business_fields(), &entity.validation, &merged, true)?;
        crate::scheduling::prepare_record(&entity, &mut merged, &ctx.timezone);
        self.check_relation_existence(ctx, &entity, &merged).await?;
        self.check_assignment(ctx, &entity, &merged).await?;
        self.validate_child_payloads(ctx, &entity, &children, true)
            .await?;
        self.check_uniques(ctx, &entity, &patch, Some(id)).await?;
        self.hooks
            .before_update(ctx, &entity.name, &current, &mut patch)
            .await?;

        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        if let Err(e) = crate::scheduling::enforce_in_tx(
            &mut tx,
            &self.repo,
            &self.registry,
            ctx,
            &entity,
            &merged,
            Some(id),
        )
        .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        let updated = match self.repo.update_tx(&mut tx, &entity, ctx, id, patch).await {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        let stored_children = match self
            .write_children(&mut tx, ctx, &entity, id, children, false)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        let updated = match self
            .recalculate_computed(&mut tx, ctx, &entity, updated, &stored_children)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(
                    &mut tx,
                    ctx,
                    &entity.name,
                    Some(id),
                    "update",
                    Some(&current),
                    Some(&updated),
                )
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = self
            .write_activity_tx(
                &mut tx,
                ctx,
                &entity,
                id,
                crate::activity::TYPE_UPDATED,
                Some(&current),
                Some(&updated),
                None,
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        let mut events = mutation_events(
            &entity.name,
            id,
            ctx,
            updated.clone(),
            "updated",
            "entity.updated",
        );
        if crate::activity::assignment_changed(Some(&current), Some(&updated)) {
            events.push(
                DomainEvent::new(
                    format!("{}.assigned", snake(&entity.name)),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({
                        "assigned_to": updated.get("assigned_to"),
                    }),
                )
                .with_user(ctx.user_id),
            );
            events.push(
                DomainEvent::new(
                    "entity.assigned".to_string(),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({ "assigned_to": updated.get("assigned_to") }),
                )
                .with_user(ctx.user_id),
            );
        }
        if let Err(e) = Outbox::enqueue_many_tx(&mut tx, &events).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) = self
            .enqueue_due_reminder_tx(&mut tx, ctx, &entity, &updated)
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) =
            crate::scheduling::enqueue_reminder_tx(&self.jobs, &mut tx, ctx, &entity, &updated)
                .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        self.hooks.after_update(ctx, &entity.name, &updated).await?;
        let _ = self.dispatch_outbox().await;
        Ok(self.present(ctx, &entity, updated).await?)
    }

    pub async fn delete(&self, ctx: &OpContext, entity_name: &str, id: Uuid) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Delete)?;
        if entity.name == USER_ENTITY {
            return self.delete_user(ctx, id).await;
        }
        let current = self.repo.get(&entity, ctx, id).await?;
        self.enforce_row_policy(ctx, &entity, &current)?;
        self.hooks
            .before_delete(ctx, &entity.name, &current)
            .await?;
        let mut tx = self
            .repo
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let deleted = match self.repo.delete_tx(&mut tx, &entity, ctx, id).await {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(
                    &mut tx,
                    ctx,
                    &entity.name,
                    Some(id),
                    "delete",
                    Some(&current),
                    None,
                )
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = self
            .write_activity_tx(
                &mut tx,
                ctx,
                &entity,
                id,
                crate::activity::TYPE_DELETED,
                Some(&current),
                None,
                None,
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        let events = mutation_events(
            &entity.name,
            id,
            ctx,
            deleted.clone(),
            "deleted",
            "entity.deleted",
        );
        if let Err(e) = Outbox::enqueue_many_tx(&mut tx, &events).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        if entity.soft_delete {
            let _ = self.soft_delete_children(ctx, &entity, id).await;
        }
        self.hooks.after_delete(ctx, &entity.name, &deleted).await?;
        let _ = self.dispatch_outbox().await;
        Ok(deleted)
    }

    pub async fn transition(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        transition: &str,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        if self.operations.try_get(&entity.name, transition).is_some() {
            return self
                .execute(ctx, &entity.name, id, transition, json!({}))
                .await;
        }
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Update)?;
        let mut current = self.repo.get(&entity, ctx, id).await?;
        self.enforce_row_policy(ctx, &entity, &current)?;
        self.expand_child_tables(ctx, &entity, &mut current).await?;
        let wf = self
            .workflows
            .for_entity(&entity.name)
            .ok_or_else(|| QefroError::not_found(format!("no workflow for {}", entity.name)))?;
        let from = current
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial)
            .to_string();
        if let Some(t) = wf.find_transition(&from, transition) {
            t.guard_allows(&current)?;
        }
        let to = self.workflows.apply(&entity.name, &from, transition, ctx)?;
        let mut patch = serde_json::Map::new();
        patch.insert(wf.field.clone(), json!(to.clone()));
        if entity.get_field("completed_at").is_some() {
            if to == STATUS_COMPLETED {
                patch.insert("completed_at".into(), json!(Utc::now().to_rfc3339()));
            } else if from == STATUS_COMPLETED {
                patch.insert("completed_at".into(), Value::Null);
            }
        }
        let patch = Value::Object(patch);
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let updated = match self.repo.update_tx(&mut tx, &entity, ctx, id, patch).await {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(
                    &mut tx,
                    ctx,
                    &entity.name,
                    Some(id),
                    &format!("transition:{transition}"),
                    Some(&current),
                    Some(&updated),
                )
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = self
            .write_activity_tx(
                &mut tx,
                ctx,
                &entity,
                id,
                crate::activity::TYPE_WORKFLOW,
                Some(&current),
                Some(&updated),
                Some(json!({ "from": from, "to": to, "transition": transition })),
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        let mut events = mutation_events(
            &entity.name,
            id,
            ctx,
            updated.clone(),
            transition,
            "workflow.transitioned",
        );
        if let Some(evt) = events.get_mut(1) {
            if let Some(obj) = evt.payload.as_object_mut() {
                obj.insert("from".into(), json!(from));
                obj.insert("to".into(), json!(to));
                obj.insert("transition".into(), json!(transition));
            }
        }
        if let Err(e) = Outbox::enqueue_many_tx(&mut tx, &events).await {
            let _ = tx.rollback().await;
            return Err(e);
        }
        if let Err(e) = self
            .enqueue_due_reminder_tx(&mut tx, ctx, &entity, &updated)
            .await
        {
            let _ = tx.rollback().await;
            return Err(e);
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let _ = self.dispatch_outbox().await;
        Ok(self.present(ctx, &entity, updated).await?)
    }

    async fn present(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        mut record: Value,
    ) -> QefroResult<Value> {
        coerce_numeric_json(entity, &mut record);
        strip_secrets(Some(entity), &mut record);
        self.expand_many_to_one(ctx, entity, &mut record).await?;
        self.expand_related_record(ctx, entity, &mut record).await?;
        self.expand_one_to_many(ctx, entity, &mut record).await?;
        self.expand_child_tables(ctx, entity, &mut record).await?;
        self.attach_workflow(ctx, entity, &mut record);
        self.attach_actions(ctx, entity, &mut record);
        self.attach_permissions(ctx, entity, &mut record);
        self.attach_links(ctx, entity, &mut record).await?;
        self.strip_forbidden_fields(ctx, entity, &mut record);
        Ok(record)
    }

    pub(crate) async fn check_uniques(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        data: &Value,
        exclude: Option<Uuid>,
    ) -> QefroResult<()> {
        let mut errors = Vec::new();
        for field in entity.stored_fields() {
            if !field.unique {
                continue;
            }
            let Some(value) = data.get(&field.name) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            if self
                .repo
                .exists_unique(entity, ctx, &field.name, value, exclude)
                .await?
            {
                errors.push(FieldError::new(
                    &field.name,
                    "unique",
                    format!("{} must be unique", field.label),
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::validation(errors))
        }
    }

    pub fn workflow_snapshot(&self, ctx: &OpContext, entity_name: &str, record: &Value) -> Value {
        let entity = match self.registry.try_get(entity_name) {
            Some(e) => e,
            None => return json!(null),
        };
        self.workflow_json(ctx, &entity, record)
    }

    fn attach_workflow(&self, ctx: &OpContext, entity: &qefro_core::EntityDef, record: &mut Value) {
        let snap = self.workflow_json(ctx, entity, record);
        if !snap.is_null() {
            if let Some(obj) = record.as_object_mut() {
                obj.insert("_workflow".into(), snap);
            }
        }
    }

    fn attach_actions(&self, ctx: &OpContext, entity: &qefro_core::EntityDef, record: &mut Value) {
        let mut actions: Vec<Value> = self
            .record_actions(ctx, &entity.name, record)
            .into_iter()
            .map(|mut d| {
                if let Some(meta) = entity.actions.iter().find(|a| {
                    a.name == d.name
                        || a.operation == d.name
                        || a.name == d.workflow_transition.clone().unwrap_or_default()
                }) {
                    if !meta.label.is_empty() {
                        d.label = meta.label.clone();
                    }
                    d.icon = meta.icon.clone().or(d.icon);
                    if let Some(conf) = &meta.confirmation {
                        d.requires_confirmation = conf.required;
                        if !conf.message.is_empty() {
                            d.confirmation_message = Some(conf.message.clone());
                        }
                    }
                    if !meta.roles.is_empty() && !ctx.is_admin() {
                        if !meta.roles.iter().any(|r| ctx.has_role(r)) {
                            return None;
                        }
                    }
                }
                Some(d.to_client_json())
            })
            .flatten()
            .collect();
        if (!entity.print_formats.is_empty() || entity.document.is_some())
            && self
                .permissions
                .allows(&ctx.roles, &entity.name, Action::Read)
        {
            actions.push(
                OperationDef::new("generate_document", &entity.name)
                    .label("Generate PDF")
                    .description("Render this record as a PDF and attach it")
                    .to_client_json(),
            );
        }
        if let Some(obj) = record.as_object_mut() {
            obj.insert("_actions".into(), json!(actions));
        }
    }

    fn attach_permissions(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) {
        if let Some(obj) = record.as_object_mut() {
            obj.insert(
                "_permissions".into(),
                json!({
                    "update": self.permissions.allows(&ctx.roles, &entity.name, Action::Update),
                    "delete": self.permissions.allows(&ctx.roles, &entity.name, Action::Delete),
                }),
            );
        }
    }

    async fn attach_links(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        let Some(id) = record.get("id").cloned() else {
            return Ok(());
        };
        let mut links = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut defs = entity.links.clone();
        for spec in self.one_to_many_specs(entity) {
            if seen.insert(spec.target.name.clone()) {
                defs.push(qefro_core::LinkDef::new(
                    spec.label.clone(),
                    spec.target.name.clone(),
                    spec.inverse.clone(),
                ));
            }
        }
        for link in defs {
            if !seen.insert(format!("{}:{}", link.entity, link.relation)) {
                continue;
            }
            let Ok(target) = self.registry.get(&link.entity) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &target.name, Action::List)
                .is_err()
            {
                continue;
            }
            let mut query = qefro_search::Query::default();
            query.page_size = 1;
            query.filters.push(qefro_search::Filter::Eq {
                field: link.relation.clone(),
                value: id.clone(),
            });
            for extra in &link.filters {
                query.filters.push(qefro_search::Filter::Eq {
                    field: extra.field.clone(),
                    value: json!(extra.value.clone()),
                });
            }
            let total = self
                .repo
                .list(&target, ctx, &query)
                .await
                .map(|p| p.total)
                .unwrap_or(0);
            links.push(json!({
                "label": link.label,
                "entity": target.name,
                "slug": target.slug,
                "relation": link.relation,
                "total": total,
                "columns": link.columns,
                "limit": link.limit,
                "filters": link.filters,
            }));
        }
        if let Some(obj) = record.as_object_mut() {
            obj.insert("_links".into(), json!(links));
        }
        Ok(())
    }

    fn strip_forbidden_fields(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) {
        let Some(obj) = record.as_object_mut() else {
            return;
        };
        for field in &entity.fields {
            if field.permission_level == 0 {
                continue;
            }
            if !self
                .permissions
                .can_read_field(ctx, &entity.name, field.permission_level)
            {
                obj.remove(&field.name);
                if let Some(expanded) = obj.get_mut("_expanded").and_then(|v| v.as_object_mut()) {
                    expanded.remove(&field.name);
                }
            }
        }
    }

    fn reject_forbidden_writes(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        data: &Value,
    ) -> QefroResult<()> {
        let Some(obj) = data.as_object() else {
            return Ok(());
        };
        let mut errors = Vec::new();
        for key in obj.keys() {
            if key.starts_with('_') {
                continue;
            }
            let Some(field) = entity.get_field(key) else {
                continue;
            };
            if field.system || field.computed {
                continue;
            }
            if !self
                .permissions
                .can_write_field(ctx, &entity.name, field.permission_level)
            {
                errors.push(FieldError::new(
                    key,
                    "forbidden",
                    format!("not allowed to write {}", field.label),
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::forbidden(errors[0].message.clone()))
        }
    }

    fn reject_locked_writes(
        &self,
        entity: &qefro_core::EntityDef,
        patch: &Value,
        children: &std::collections::HashMap<String, Vec<Value>>,
    ) -> QefroResult<()> {
        let mut errors = Vec::new();
        if let Some(obj) = patch.as_object() {
            for key in obj.keys() {
                if key.starts_with('_') {
                    continue;
                }
                let Some(field) = entity.get_field(key) else {
                    continue;
                };
                if field.system || field.computed || field.allow_on_submit {
                    continue;
                }
                errors.push(FieldError::new(
                    key,
                    "locked",
                    format!(
                        "{} cannot be edited in a locked document state",
                        field.label
                    ),
                ));
            }
        }
        for name in children.keys() {
            let Some(field) = entity.get_field(name) else {
                continue;
            };
            if field.allow_on_submit {
                continue;
            }
            errors.push(FieldError::new(
                name,
                "locked",
                format!(
                    "{} cannot be edited in a locked document state",
                    field.label
                ),
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::locked(errors))
        }
    }

    async fn write_activity_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        id: Uuid,
        activity_type: &str,
        old: Option<&Value>,
        new: Option<&Value>,
        extra: Option<Value>,
    ) -> QefroResult<()> {
        if !entity.activity {
            return Ok(());
        }
        let activity_type = if activity_type == crate::activity::TYPE_UPDATED
            && crate::activity::assignment_changed(old, new)
        {
            crate::activity::TYPE_ASSIGNMENT
        } else {
            activity_type
        };
        let (message, metadata) =
            crate::activity::mutation_activity(&entity.label, activity_type, old, new, extra);
        self.activity
            .record_tx(tx, ctx, &entity.name, id, activity_type, &message, metadata)
            .await?;
        if let Some(related) = new.or(old) {
            let _ = self
                .write_related_activity_tx(tx, ctx, entity, related, activity_type, &message)
                .await;
        }
        Ok(())
    }

    pub async fn get_singleton(&self, ctx: &OpContext, entity_name: &str) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Read)?;
        if !entity.singleton {
            return Err(QefroError::bad_request(format!(
                "{} is not a singleton",
                entity.name
            )));
        }
        if let Some(record) = self.repo.get_singleton(&entity, ctx).await? {
            return self.present(ctx, &entity, record).await;
        }
        self.create(ctx, entity_name, json!({})).await
    }

    pub async fn patch_singleton(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        patch: Value,
    ) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        if !entity.singleton {
            return Err(QefroError::bad_request(format!(
                "{} is not a singleton",
                entity.name
            )));
        }
        let current = match self.repo.get_singleton(&entity, ctx).await? {
            Some(row) => row,
            None => self.create(ctx, entity_name, json!({})).await?,
        };
        let id = record_id(&current)?;
        self.update(ctx, entity_name, id, patch).await
    }

    pub fn entity_by_slug(&self, slug: &str) -> QefroResult<qefro_core::EntityDef> {
        self.registry
            .list()
            .into_iter()
            .find(|e| e.slug == slug || e.name.eq_ignore_ascii_case(slug))
            .map(|e| (*e).clone())
            .ok_or_else(|| QefroError::not_found(format!("entity '{slug}' not found")))
    }

    fn workflow_json(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &Value,
    ) -> Value {
        let Some(wf) = self.workflows.for_entity(&entity.name) else {
            return json!(null);
        };
        let current = record
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial)
            .to_string();
        let transitions: Vec<Value> = wf
            .allowed_from(&current, ctx)
            .into_iter()
            .map(|t| {
                json!({
                    "id": t.name,
                    "name": t.name,
                    "label": if t.label.is_empty() { t.name.clone() } else { t.label.clone() },
                    "from": t.from,
                    "from_state": t.from,
                    "to": t.to,
                    "to_state": t.to,
                    "allowed_roles": t.allowed_roles,
                    "permissions": t.allowed_roles,
                    "requires_confirmation": t.confirmation,
                    "confirmation": t.confirmation,
                    "confirmation_message": if t.confirmation_message.is_empty() {
                        Value::Null
                    } else {
                        json!(t.confirmation_message)
                    },
                })
            })
            .collect();
        json!({
            "name": wf.name,
            "field": wf.field,
            "current": current,
            "transitions": transitions,
        })
    }

    async fn expand_many_to_one(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        use qefro_core::RelationKind;
        let mut expansions = serde_json::Map::new();
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &target.name, Action::Read)
                .is_err()
            {
                continue;
            }
            if let Ok(related) = self.repo.get(&target, ctx, id).await {
                let nested = self
                    .nested_relation_expansions(ctx, &target, &related)
                    .await?;
                expansions.insert(
                    field.name.clone(),
                    relation_expansion(&target, id, &related, nested),
                );
            }
        }
        if !expansions.is_empty() {
            if let Some(obj) = record.as_object_mut() {
                obj.insert("_expanded".into(), Value::Object(expansions));
            }
        }
        Ok(())
    }

    pub(crate) async fn expand_many_to_one_batch(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        records: &mut [Value],
    ) -> QefroResult<()> {
        use qefro_core::RelationKind;
        use std::collections::HashSet;
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &target.name, Action::Read)
                .is_err()
            {
                continue;
            }
            let mut ids = HashSet::new();
            for record in records.iter() {
                if let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) {
                    if let Ok(id) = Uuid::parse_str(id_str) {
                        ids.insert(id);
                    }
                }
            }
            let fetched = self
                .repo
                .list_by_ids(&target, ctx, &ids.into_iter().collect::<Vec<_>>())
                .await?;
            let nested_by_id = self
                .nested_relation_expansions_batch(ctx, &target, &fetched)
                .await?;
            let mut labels = std::collections::HashMap::new();
            for related in fetched {
                if let Ok(id) = record_id(&related) {
                    labels.insert(id, related);
                }
            }
            for record in records.iter_mut() {
                let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Ok(id) = Uuid::parse_str(id_str) else {
                    continue;
                };
                if let Some(related) = labels.get(&id) {
                    if let Some(obj) = record.as_object_mut() {
                        let expanded = obj.entry("_expanded").or_insert_with(|| json!({}));
                        if let Some(map) = expanded.as_object_mut() {
                            let nested = nested_by_id.get(&id).cloned().unwrap_or_default();
                            map.insert(
                                field.name.clone(),
                                relation_expansion(&target, id, related, nested),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn expand_one_to_many(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        use qefro_search::{Filter, Query};
        let Some(id) = record.get("id").and_then(|v| v.as_str()) else {
            return Ok(());
        };
        let mut related = serde_json::Map::new();
        for spec in self.one_to_many_specs(entity) {
            if self
                .permissions
                .check(ctx, &spec.target.name, Action::List)
                .is_err()
            {
                continue;
            }
            let link = entity
                .links
                .iter()
                .find(|l| l.entity == spec.target.name && l.relation == spec.inverse);
            let mut query = Query::default();
            query.page_size = link.and_then(|l| l.limit).unwrap_or(50).min(50);
            query.filters.push(Filter::Eq {
                field: spec.inverse.clone(),
                value: json!(id),
            });
            if let Some(link) = link {
                for extra in &link.filters {
                    query.filters.push(Filter::Eq {
                        field: extra.field.clone(),
                        value: json!(extra.value.clone()),
                    });
                }
            }
            if let Ok(mut page) = self.repo.list(&spec.target, ctx, &query).await {
                for item in &mut page.items {
                    strip_secrets(Some(&spec.target), item);
                }
                let label = link
                    .map(|l| l.label.clone())
                    .unwrap_or_else(|| spec.label.clone());
                let columns = link.map(|l| l.columns.clone()).unwrap_or_default();
                related.insert(
                    spec.name.clone(),
                    json!({
                        "entity": spec.target.name,
                        "slug": spec.target.slug,
                        "label": label,
                        "items": page.items,
                        "total": page.total,
                        "columns": columns,
                        "limit": link.and_then(|l| l.limit),
                        "filters": link.map(|l| l.filters.clone()).unwrap_or_default(),
                    }),
                );
            }
        }
        if !related.is_empty() {
            if let Some(obj) = record.as_object_mut() {
                obj.insert("_related".into(), Value::Object(related));
            }
        }
        Ok(())
    }

    fn one_to_many_specs(&self, entity: &qefro_core::EntityDef) -> Vec<RelatedSpec> {
        use qefro_core::RelationKind;
        let mut specs = Vec::new();
        let mut seen_targets = std::collections::HashSet::new();
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::OneToMany {
                continue;
            }
            let Some(inverse) = &rel.inverse_field else {
                continue;
            };
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if !seen_targets.insert(target.name.clone()) {
                continue;
            }
            let label = if field.ui.label.is_empty() {
                field.label.clone()
            } else {
                field.ui.label.clone()
            };
            specs.push(RelatedSpec {
                name: field.name.clone(),
                label,
                target,
                inverse: inverse.clone(),
            });
        }
        if entity.name == PERSON_ENTITY {
            for other in self.registry.list() {
                if other.name == PERSON_ENTITY {
                    continue;
                }
                if seen_targets.contains(&other.name) {
                    continue;
                }
                if !other.fields.iter().any(is_person_link_field) {
                    continue;
                }
                let field = person_backref_field(&other);
                seen_targets.insert(other.name.clone());
                specs.push(RelatedSpec {
                    name: field.name,
                    label: field.label,
                    target: other,
                    inverse: PERSON_LINK_FIELD.into(),
                });
            }
        }
        specs
    }

    async fn nested_relation_expansions(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &Value,
    ) -> QefroResult<serde_json::Map<String, Value>> {
        use qefro_core::RelationKind;
        let mut nested = serde_json::Map::new();
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &target.name, Action::Read)
                .is_err()
            {
                continue;
            }
            if let Ok(related) = self.repo.get(&target, ctx, id).await {
                nested.insert(
                    field.name.clone(),
                    relation_expansion(&target, id, &related, serde_json::Map::new()),
                );
            }
        }
        Ok(nested)
    }

    async fn nested_relation_expansions_batch(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        records: &[Value],
    ) -> QefroResult<std::collections::HashMap<Uuid, serde_json::Map<String, Value>>> {
        use qefro_core::RelationKind;
        use std::collections::{HashMap, HashSet};
        let mut nested_by_id: HashMap<Uuid, serde_json::Map<String, Value>> = HashMap::new();
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &target.name, Action::Read)
                .is_err()
            {
                continue;
            }
            let mut nested_ids = HashSet::new();
            let mut owners: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
            for related in records {
                let Ok(owner_id) = record_id(related) else {
                    continue;
                };
                let Some(id_str) = related.get(&field.name).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Ok(nid) = Uuid::parse_str(id_str) else {
                    continue;
                };
                nested_ids.insert(nid);
                owners.entry(nid).or_default().push(owner_id);
            }
            if nested_ids.is_empty() {
                continue;
            }
            let fetched = self
                .repo
                .list_by_ids(&target, ctx, &nested_ids.into_iter().collect::<Vec<_>>())
                .await?;
            for nested_row in fetched {
                let Ok(nid) = record_id(&nested_row) else {
                    continue;
                };
                let exp = relation_expansion(&target, nid, &nested_row, serde_json::Map::new());
                if let Some(owner_ids) = owners.get(&nid) {
                    for owner in owner_ids {
                        nested_by_id
                            .entry(*owner)
                            .or_default()
                            .insert(field.name.clone(), exp.clone());
                    }
                }
            }
        }
        Ok(nested_by_id)
    }

    async fn expand_child_tables(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        use qefro_core::RelationKind;
        let Some(id) = record
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        else {
            return Ok(());
        };
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ChildTable {
                continue;
            }
            let inverse = rel
                .inverse_field
                .clone()
                .unwrap_or_else(|| "parent_id".into());
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            let mut query = child_rows_query(&target, &inverse, &id);
            query.page_size = 200;
            if let Ok(mut page) = self.repo.list(&target, ctx, &query).await {
                for item in &mut page.items {
                    coerce_numeric_json(&target, item);
                }
                if let Some(obj) = record.as_object_mut() {
                    obj.insert(field.name.clone(), json!(page.items));
                }
            }
        }
        Ok(())
    }

    async fn validate_child_payloads(
        &self,
        _ctx: &OpContext,
        parent: &qefro_core::EntityDef,
        children: &std::collections::HashMap<String, Vec<Value>>,
        partial: bool,
    ) -> QefroResult<()> {
        let mut errors = Vec::new();
        for field in &parent.fields {
            if !field.is_child_table() {
                continue;
            }
            let Some(rows) = children.get(&field.name) else {
                continue;
            };
            let Some(rel) = &field.relation else { continue };
            let child = self.registry.get(&rel.target_entity)?;
            let inverse = rel
                .inverse_field
                .clone()
                .unwrap_or_else(|| "parent_id".into());
            let fields: Vec<_> = child
                .business_fields()
                .iter()
                .filter(|f| {
                    f.name != inverse
                        && f.name != "parent_id"
                        && f.relation.as_ref().map(|r| r.target_entity.as_str())
                            != Some(parent.name.as_str())
                })
                .cloned()
                .collect();
            for (i, row) in rows.iter().enumerate() {
                let mut row_errors = Vec::new();
                if let Err(qefro_core::QefroError::Validation { fields, .. }) =
                    validate_record(&fields, row, partial)
                {
                    row_errors.extend(fields);
                }
                if let Err(qefro_core::QefroError::Validation { fields, .. }) =
                    apply_entity_rules(&fields, &child.validation, row, partial)
                {
                    row_errors.extend(fields);
                }
                for err in row_errors {
                    let mut mapped = FieldError::new(
                        format!("{}.{}.{}", field.name, i, err.field),
                        err.code,
                        err.message,
                    )
                    .with_entity(&child.name);
                    if let Some(rule) = err.rule {
                        mapped = mapped.with_rule(rule);
                    }
                    errors.push(mapped);
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::validation(errors))
        }
    }

    async fn write_children(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        parent: &qefro_core::EntityDef,
        parent_id: Uuid,
        children: std::collections::HashMap<String, Vec<Value>>,
        is_create: bool,
    ) -> QefroResult<std::collections::HashMap<String, Vec<Value>>> {
        use qefro_core::RelationKind;
        use std::collections::{HashMap, HashSet};
        let mut stored: HashMap<String, Vec<Value>> = HashMap::new();
        for field in &parent.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ChildTable {
                continue;
            }
            let child = self.registry.get(&rel.target_entity)?;
            let inverse = rel
                .inverse_field
                .clone()
                .unwrap_or_else(|| "parent_id".into());
            if !children.contains_key(&field.name) {
                if is_create {
                    stored.insert(field.name.clone(), Vec::new());
                } else {
                    let mut query = child_rows_query(&child, &inverse, &parent_id.to_string());
                    query.page_size = 200;
                    let page = self.repo.list_tx(tx, &child, ctx, &query).await?;
                    stored.insert(field.name.clone(), page.items);
                }
                continue;
            }
            let incoming = children.get(&field.name).cloned().unwrap_or_default();
            let mut query = child_rows_query(&child, &inverse, &parent_id.to_string());
            query.page_size = 500;
            let existing = if is_create {
                Vec::new()
            } else {
                self.repo.list_tx(tx, &child, ctx, &query).await?.items
            };
            let existing_ids: HashSet<Uuid> = existing
                .iter()
                .filter_map(|row| record_id(row).ok())
                .collect();
            let mut seen = HashSet::new();
            let mut out_rows = Vec::new();
            for (i, mut row) in incoming.into_iter().enumerate() {
                reject_client_tenant(&row)?;
                strip_computed(&child, &mut row);
                if let Some(obj) = row.as_object_mut() {
                    obj.insert(inverse.clone(), json!(parent_id.to_string()));
                    if child.get_field("sort_order").is_some() {
                        obj.insert("sort_order".into(), json!(i as i64));
                    }
                }
                qefro_core::apply_computed_fields(
                    child.business_fields(),
                    &mut row,
                    &HashMap::new(),
                )?;
                let row_id = row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());
                let saved = if let Some(cid) = row_id {
                    if !existing_ids.contains(&cid) {
                        return Err(QefroError::forbidden(
                            "cannot attach a child that does not belong to this document",
                        ));
                    }
                    let current = self.repo.get_tx(tx, &child, ctx, cid, true).await?;
                    let parent_ok = current
                        .get(&inverse)
                        .and_then(|v| v.as_str())
                        .map(|s| s == parent_id.to_string())
                        .unwrap_or(false);
                    if !parent_ok {
                        return Err(QefroError::forbidden(
                            "cannot attach a child from another document",
                        ));
                    }
                    seen.insert(cid);
                    if let Some(obj) = row.as_object_mut() {
                        obj.remove("id");
                    }
                    self.repo.update_tx(tx, &child, ctx, cid, row).await?
                } else {
                    self.repo.insert_tx(tx, &child, ctx, row).await?
                };
                out_rows.push(saved);
            }
            if !is_create {
                for old in existing {
                    let oid = record_id(&old)?;
                    if !seen.contains(&oid) {
                        self.repo.delete_tx(tx, &child, ctx, oid).await?;
                    }
                }
            }
            stored.insert(field.name.clone(), out_rows);
        }
        Ok(stored)
    }

    async fn recalculate_computed(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        mut record: Value,
        children: &std::collections::HashMap<String, Vec<Value>>,
    ) -> QefroResult<Value> {
        if !entity.fields.iter().any(|f| f.computed) {
            return Ok(record);
        }
        qefro_core::apply_computed_fields(entity.business_fields(), &mut record, children)?;
        let mut patch = serde_json::Map::new();
        for field in entity.stored_fields() {
            if field.computed {
                if let Some(v) = record.get(&field.name) {
                    patch.insert(field.name.clone(), v.clone());
                }
            }
        }
        if patch.is_empty() {
            return Ok(record);
        }
        let id = record_id(&record)?;
        self.repo
            .update_tx(tx, entity, ctx, id, Value::Object(patch))
            .await
    }

    async fn soft_delete_children(
        &self,
        ctx: &OpContext,
        parent: &qefro_core::EntityDef,
        parent_id: Uuid,
    ) -> QefroResult<()> {
        use qefro_core::RelationKind;
        for field in &parent.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::ChildTable {
                continue;
            }
            let Ok(child) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            let inverse = rel
                .inverse_field
                .clone()
                .unwrap_or_else(|| "parent_id".into());
            let mut query = child_rows_query(&child, &inverse, &parent_id.to_string());
            query.page_size = 500;
            let page = self.repo.list(&child, ctx, &query).await?;
            for row in page.items {
                if let Ok(cid) = record_id(&row) {
                    let _ = self.repo.delete(&child, ctx, cid).await;
                }
            }
        }
        Ok(())
    }

    pub async fn run_report(
        &self,
        ctx: &OpContext,
        report: &qefro_core::ReportDef,
        filters: Value,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, &report.entity).await?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        crate::reports::validate_report(&entity, report)?;
        for field in report.fields.iter().chain(report.group_by.iter()) {
            if let Some(def) = entity.get_field(field) {
                if def.ui.hidden {
                    return Err(QefroError::forbidden(format!(
                        "field '{field}' is not visible"
                    )));
                }
            }
        }
        let mut combined = report.filters.clone();
        if let Some(items) = filters.as_array() {
            combined.extend(items.iter().cloned());
        }
        let parsed = crate::reports::filters_from_json(&entity, &Value::Array(combined))?;
        let mut query = qefro_search::Query::default();
        query.filters = parsed;
        query.page_size = 500;
        self.apply_row_policy_filters(ctx, &entity, &mut query);
        let rows = crate::reports::execute_report(
            self.pool(),
            &entity,
            entity.tenant_owned.then_some(ctx.tenant_id),
            report,
            &query,
        )
        .await?;
        let series: Vec<Value> = rows
            .iter()
            .map(|row| {
                let label = report
                    .group_by
                    .first()
                    .and_then(|g| row.get(g))
                    .cloned()
                    .unwrap_or(json!(""));
                let value = report
                    .aggregations
                    .keys()
                    .next()
                    .and_then(|k| row.get(k))
                    .cloned()
                    .unwrap_or(json!(0));
                json!({ "label": label, "value": value })
            })
            .collect();
        Ok(json!({
            "name": report.name,
            "label": report.label,
            "entity": report.entity,
            "chart": report.chart,
            "group_by": report.group_by,
            "rows": rows,
            "series": series,
        }))
    }

    pub async fn print_document(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        format_name: Option<&str>,
        extra_formats: &[qefro_core::PrintFormat],
    ) -> QefroResult<(qefro_core::PrintFormat, Value, Vec<Value>)> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::Read)?;
        let mut record = self.get(ctx, entity_name, id).await?;
        let format = qefro_core::resolve_print_format(
            &entity.name,
            format_name,
            &entity.print_formats,
            extra_formats,
        )
        .or_else(|| {
            if entity.document.is_some() {
                Some(qefro_core::PrintFormat::new(
                    format!("{} Standard", entity.label),
                    &entity.name,
                ))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            QefroError::not_found(format!("no document template for '{}'", entity.name))
        })?;
        self.hydrate_print_relations(ctx, &entity, &format, &mut record, 0)
            .await?;
        let table = format.item_table.as_deref().or_else(|| {
            entity
                .fields
                .iter()
                .find(|f| f.is_child_table())
                .map(|f| f.name.as_str())
        });
        let items = table
            .and_then(|name| record.get(name))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok((format, record, items))
    }

    async fn hydrate_print_relations(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        format: &qefro_core::PrintFormat,
        record: &mut Value,
        depth: usize,
    ) -> QefroResult<()> {
        if depth >= 4 {
            return Ok(());
        }
        use qefro_core::RelationKind;
        let referenced = print_relation_aliases(format);
        for field in &entity.fields {
            let Some(rel) = &field.relation else {
                continue;
            };
            if rel.kind != RelationKind::ManyToOne {
                continue;
            }
            let alias = field
                .name
                .strip_suffix("_id")
                .unwrap_or(field.name.as_str());
            if !referenced.is_empty()
                && !referenced.contains(&field.name)
                && !referenced.contains(alias)
            {
                continue;
            }
            let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            if self
                .permissions
                .check(ctx, &rel.target_entity, Action::Read)
                .is_err()
            {
                continue;
            }
            let Ok(mut related) = self.get(ctx, &rel.target_entity, id).await else {
                continue;
            };
            if let Ok(target) = self.registry.get(&rel.target_entity) {
                Box::pin(self.hydrate_print_relations(
                    ctx,
                    &target,
                    format,
                    &mut related,
                    depth + 1,
                ))
                .await?;
            }
            if let Some(obj) = record.as_object_mut() {
                obj.insert(alias.to_string(), related.clone());
                let expanded = obj.entry("_expanded").or_insert_with(|| json!({}));
                if let Some(map) = expanded.as_object_mut() {
                    map.insert(field.name.clone(), related.clone());
                    map.insert(alias.to_string(), related);
                }
            }
        }
        Ok(())
    }

    pub async fn dashboard_card_value(
        &self,
        ctx: &OpContext,
        card: &qefro_core::DashboardCard,
    ) -> QefroResult<Value> {
        if !card.roles.is_empty() && !ctx.is_admin() && !card.roles.iter().any(|r| ctx.has_role(r))
        {
            return Err(QefroError::forbidden("dashboard card is not visible"));
        }
        let kind = normalize_card_kind(&card.kind);
        if kind == "audit" {
            if !ctx.is_admin() {
                return Err(QefroError::forbidden("audit widgets are Admin-only"));
            }
            return self.audit_card_value(ctx, card).await;
        }
        let entity = self.registry.get(&card.entity)?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        use qefro_search::parse_query;
        let mut raw: Vec<(String, String)> = card
            .filters
            .iter()
            .map(|f| {
                let value = match f.value.as_str() {
                    "today" => chrono::Utc::now().date_naive().to_string(),
                    "tomorrow" => {
                        (chrono::Utc::now().date_naive() + chrono::Days::new(1)).to_string()
                    }
                    "now" => chrono::Utc::now().to_rfc3339(),
                    "current_user" | "me" => ctx.user_id.to_string(),
                    _ => f.value.clone(),
                };
                (f.field.clone(), value)
            })
            .collect();
        raw.push(("page_size".into(), "1".into()));
        let mut query = parse_query(&entity, &raw)?;
        self.apply_row_policy_filters(ctx, &entity, &mut query);
        if matches!(kind.as_str(), "chart" | "status_breakdown" | "workflow") {
            let group_by = card
                .group_by
                .as_deref()
                .ok_or_else(|| QefroError::bad_request("chart cards require group_by"))?;
            let series = self
                .repo
                .aggregate_group_with(
                    &entity,
                    ctx,
                    &query,
                    group_by,
                    &card.metric,
                    card.field.as_deref(),
                )
                .await?;
            let value: f64 = series
                .iter()
                .filter_map(|row| row.get("value").and_then(|v| v.as_f64()))
                .sum();
            return Ok(json!({
                "title": card.title,
                "entity": card.entity,
                "metric": card.metric,
                "kind": kind,
                "chart": card.chart,
                "group_by": group_by,
                "filters": card.filters,
                "size": card.size,
                "series": series,
                "value": value,
            }));
        }
        if kind == "activity" {
            let limit = card.limit.unwrap_or(8).clamp(1, 50);
            let rows = self
                .list_recent_activity(ctx, Some(&entity.name), limit as i64)
                .await?;
            let items: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    json!({
                        "id": row.id,
                        "entity": row.entity_type,
                        "entity_id": row.entity_id,
                        "activity_type": row.activity_type,
                        "message": row.message,
                        "actor_name": row.actor_name,
                        "created_at": row.created_at,
                    })
                })
                .collect();
            let total = items.len();
            return Ok(json!({
                "title": card.title,
                "entity": card.entity,
                "metric": card.metric,
                "kind": "activity",
                "filters": card.filters,
                "size": card.size,
                "items": items,
                "total": total,
                "value": total,
            }));
        }
        if matches!(kind.as_str(), "list" | "table" | "saved_view") {
            let limit = card.limit.unwrap_or(8).clamp(1, 50);
            raw.pop();
            raw.push(("page_size".into(), limit.to_string()));
            let query = parse_query(&entity, &raw)?;
            let page = self.list(ctx, &entity.name, query).await?;
            return Ok(json!({
                "title": card.title,
                "entity": card.entity,
                "metric": card.metric,
                "kind": kind,
                "filters": card.filters,
                "size": card.size,
                "saved_view": card.saved_view,
                "items": page.items,
                "total": page.total,
                "value": page.total,
            }));
        }
        let value = self
            .repo
            .aggregate(&entity, ctx, &query, &card.metric, card.field.as_deref())
            .await?;
        Ok(json!({
            "title": card.title,
            "entity": card.entity,
            "metric": card.metric,
            "kind": if kind == "kpi" { "kpi" } else { "metric" },
            "filters": card.filters,
            "size": card.size,
            "value": value,
        }))
    }

    pub async fn entity_aggregates(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        group_by: &str,
        metric: &str,
        field: Option<&str>,
        mut query: Query,
    ) -> QefroResult<Value> {
        let entity = self.entity_for(ctx, entity_name).await?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        self.apply_row_policy_filters(ctx, &entity, &mut query);
        if entity.get_field(group_by).is_some_and(|f| f.custom)
            || (entity.get_field(group_by).is_none() && !entity.has_column(group_by))
        {
            return Err(QefroError::forbidden(format!(
                "unknown or unauthorized group_by field '{group_by}'"
            )));
        }
        let series = self
            .repo
            .aggregate_group_with(&entity, ctx, &query, group_by, metric, field)
            .await?;
        Ok(json!({
            "entity": entity.name,
            "group_by": group_by,
            "metric": metric,
            "field": field,
            "series": series,
        }))
    }

    async fn audit_card_value(
        &self,
        ctx: &OpContext,
        card: &qefro_core::DashboardCard,
    ) -> QefroResult<Value> {
        let items = self.audit.list(ctx, None, None, 200).await?;
        let today = chrono::Utc::now().date_naive();
        let value = match card.metric.as_str() {
            "failed" => items
                .iter()
                .filter(|r| {
                    r.action.contains("fail")
                        || r.action.contains("denied")
                        || r.action.contains("error")
                })
                .count(),
            "user_disabled" => items
                .iter()
                .filter(|r| r.action.contains("disable") || r.entity == qefro_core::USER_ENTITY)
                .filter(|r| {
                    r.created_at.date_naive() == today
                        && r.new_values
                            .as_ref()
                            .and_then(|v| v.get("enabled"))
                            .and_then(|v| v.as_bool())
                            == Some(false)
                })
                .count(),
            _ => items
                .iter()
                .filter(|r| r.created_at.date_naive() == today)
                .count(),
        };
        Ok(json!({
            "title": card.title,
            "entity": "_audit",
            "metric": card.metric,
            "kind": "audit",
            "size": card.size,
            "value": value as f64,
        }))
    }

    fn identity(&self) -> QefroResult<&AuthService> {
        self.identity
            .as_deref()
            .ok_or_else(|| QefroError::internal("identity directory is not configured"))
    }

    async fn list_users(&self, ctx: &OpContext, query: Query) -> QefroResult<Page> {
        let (items, total) = self
            .identity()?
            .list_tenant_users(
                ctx.tenant_id,
                query.search.as_deref(),
                query.page,
                query.page_size,
            )
            .await?;
        let entity = self.registry.get(USER_ENTITY)?;
        let mut items = items;
        for item in &mut items {
            strip_secrets(Some(&entity), item);
            self.strip_forbidden_fields(ctx, &entity, item);
            self.attach_workflow(ctx, &entity, item);
            self.attach_permissions(ctx, &entity, item);
        }
        Ok(Page {
            items,
            page: query.page.max(1),
            page_size: query.page_size.clamp(1, 200),
            total,
        })
    }

    async fn get_user(&self, ctx: &OpContext, id: Uuid) -> QefroResult<Value> {
        let mut record = self.identity()?.get_tenant_user(ctx.tenant_id, id).await?;
        strip_secrets(None, &mut record);
        Ok(record)
    }

    async fn create_user(&self, ctx: &OpContext, data: Value) -> QefroResult<Value> {
        reject_client_tenant(&data)?;
        let entity = self.registry.get(USER_ENTITY)?;
        self.reject_forbidden_writes(ctx, &entity, &data)?;
        let created = self.identity()?.create_tenant_user(ctx, &data).await?;
        let id = record_id(&created)?;
        if entity.audit {
            self.audit
                .record(ctx, USER_ENTITY, Some(id), "create", None, Some(&created))
                .await?;
        }
        if entity.activity {
            let (message, metadata) = crate::activity::mutation_activity(
                &entity.label,
                crate::activity::TYPE_CREATED,
                None,
                Some(&created),
                None,
            );
            let _ = self
                .activity
                .record(
                    ctx,
                    USER_ENTITY,
                    id,
                    crate::activity::TYPE_CREATED,
                    &message,
                    metadata,
                )
                .await;
        }
        self.present(ctx, &entity, created).await
    }

    async fn update_user(&self, ctx: &OpContext, id: Uuid, patch: Value) -> QefroResult<Value> {
        reject_client_tenant(&patch)?;
        let entity = self.registry.get(USER_ENTITY)?;
        self.reject_forbidden_writes(ctx, &entity, &patch)?;
        let current = self.identity()?.get_tenant_user(ctx.tenant_id, id).await?;
        let updated = self.identity()?.update_tenant_user(ctx, id, &patch).await?;
        if entity.audit {
            self.audit
                .record(
                    ctx,
                    USER_ENTITY,
                    Some(id),
                    "update",
                    Some(&current),
                    Some(&updated),
                )
                .await?;
        }
        if entity.activity {
            let (message, metadata) = crate::activity::mutation_activity(
                &entity.label,
                crate::activity::TYPE_UPDATED,
                Some(&current),
                Some(&updated),
                None,
            );
            let _ = self
                .activity
                .record(
                    ctx,
                    USER_ENTITY,
                    id,
                    crate::activity::TYPE_UPDATED,
                    &message,
                    metadata,
                )
                .await;
        }
        let was_enabled = current
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let now_enabled = updated
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if was_enabled && !now_enabled {
            let mut event = DomainEvent::new(
                "user.disabled",
                USER_ENTITY.to_string(),
                id,
                ctx.tenant_id,
                json!({ "enabled": false }),
            );
            event.user_id = Some(ctx.user_id);
            self.outbox.enqueue(&event).await?;
            let _ = self.dispatch_outbox().await;
        }
        self.present(ctx, &entity, updated).await
    }

    async fn delete_user(&self, ctx: &OpContext, id: Uuid) -> QefroResult<Value> {
        let entity = self.registry.get(USER_ENTITY)?;
        let deleted = self.identity()?.remove_tenant_membership(ctx, id).await?;
        if entity.audit {
            self.audit
                .record(ctx, USER_ENTITY, Some(id), "delete", Some(&deleted), None)
                .await?;
        }
        Ok(deleted)
    }

    async fn link_person_account(&self, ctx: &OpContext, data: &mut Value) -> QefroResult<()> {
        let create = data
            .get("create_account")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !create {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("create_account");
                obj.remove("password");
            }
            return Ok(());
        }
        self.permissions.check(ctx, USER_ENTITY, Action::Create)?;
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let email = data
            .get("email")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let password = data
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let payload = json!({
            "name": name,
            "email": email,
            "password": password,
            "roles": ["Staff"],
        });
        let user = self.identity()?.create_tenant_user(ctx, &payload).await?;
        if let Some(obj) = data.as_object_mut() {
            obj.insert("user_id".into(), user["id"].clone());
            obj.remove("create_account");
            obj.remove("password");
        }
        Ok(())
    }

    pub(crate) fn ensure_app(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
    ) -> QefroResult<()> {
        if ctx.allows_app(entity.module.as_deref()) {
            Ok(())
        } else {
            Err(QefroError::not_found(format!("{} not found", entity.name)))
        }
    }

    pub(crate) fn reject_worker_crud(&self, ctx: &OpContext) -> QefroResult<()> {
        if ctx.is_automation() {
            return Ok(());
        }
        if ctx.is_worker() {
            Err(QefroError::forbidden(
                "workers cannot perform generic entity mutations",
            ))
        } else {
            Ok(())
        }
    }

    async fn check_relation_existence(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        data: &Value,
    ) -> QefroResult<()> {
        let mut errors = Vec::new();
        for rule in existence_rules(&entity.validation) {
            let Some(field_name) = rule.field.as_deref() else {
                continue;
            };
            let Some(field) = entity.fields.iter().find(|f| f.name == field_name) else {
                continue;
            };
            let value = data.get(field_name);
            if value.is_none() || value == Some(&Value::Null) {
                continue;
            }
            let Some(id) = value
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                errors.push(FieldError::new(
                    field_name,
                    "exists",
                    format!("{} is not a valid id", field.label),
                ));
                continue;
            };
            let Some(rel) = &field.relation else {
                continue;
            };
            let target = self.registry.get(&rel.target_entity)?;
            match self.repo.get(&target, ctx, id).await {
                Ok(_) => {}
                Err(QefroError::NotFound { .. }) => {
                    errors.push(FieldError::new(
                        field_name,
                        "exists",
                        format!("{} must reference an existing {}", field.label, target.name),
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::validation(errors))
        }
    }

    async fn check_assignment(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        data: &Value,
    ) -> QefroResult<()> {
        if entity.get_field("assigned_to").is_none() {
            return Ok(());
        }
        let Some(raw) = data.get("assigned_to") else {
            return Ok(());
        };
        if raw.is_null() {
            return Ok(());
        }
        let Some(id) = raw.as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
            return Err(QefroError::bad_request("assigned_to must be a user id"));
        };
        if id != ctx.user_id {
            self.permissions.check(ctx, USER_ENTITY, Action::Read)?;
        }
        let Some(identity) = self.identity_service() else {
            return Ok(());
        };
        let user = match identity.get_tenant_user(ctx.tenant_id, id).await {
            Ok(user) => user,
            Err(QefroError::NotFound { .. }) => {
                return Err(QefroError::bad_request(
                    "cannot assign to a user outside this tenant",
                ));
            }
            Err(e) => return Err(e),
        };
        if user
            .get("enabled")
            .and_then(|v| v.as_bool())
            .is_some_and(|enabled| !enabled)
        {
            return Err(QefroError::bad_request("cannot assign to a disabled user"));
        }
        Ok(())
    }

    async fn enqueue_due_reminder_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &Value,
    ) -> QefroResult<()> {
        if entity.get_field("due_at").is_none() {
            return Ok(());
        }
        let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status.eq_ignore_ascii_case(STATUS_COMPLETED)
            || status.eq_ignore_ascii_case(STATUS_CANCELLED)
        {
            return Ok(());
        }
        let Some(due) = record
            .get("due_at")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            return Ok(());
        };
        let id = record_id(record)?;
        let key = format!("due:{}:{}:{}", entity.name, id, due);
        self.jobs
            .enqueue_tx(
                tx,
                ctx,
                crate::due::DUE_REMINDER_JOB,
                json!({
                    "entity": entity.name,
                    "record_id": id,
                    "due_at": due,
                    "run_at": due,
                    "idempotency_key": key,
                }),
            )
            .await?;
        Ok(())
    }

    async fn expand_related_record(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        if entity.get_field(RELATED_TYPE_FIELD).is_none()
            || entity.get_field(RELATED_ID_FIELD).is_none()
        {
            return Ok(());
        }
        let Some(type_name) = record
            .get(RELATED_TYPE_FIELD)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            return Ok(());
        };
        let Some(id) = record
            .get(RELATED_ID_FIELD)
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            return Ok(());
        };
        let Ok(target) = self.registry.get(type_name) else {
            return Ok(());
        };
        if self
            .permissions
            .check(ctx, &target.name, Action::Read)
            .is_err()
        {
            return Ok(());
        }
        let Ok(related) = self.repo.get(&target, ctx, id).await else {
            return Ok(());
        };
        let nested = self
            .nested_relation_expansions(ctx, &target, &related)
            .await
            .unwrap_or_default();
        let expansion = relation_expansion(&target, id, &related, nested);
        if let Some(obj) = record.as_object_mut() {
            let expanded = obj
                .entry("_expanded")
                .or_insert_with(|| json!({}))
                .as_object_mut();
            if let Some(map) = expanded {
                map.insert(RELATED_ID_FIELD.into(), expansion);
            }
        }
        Ok(())
    }

    async fn write_related_activity_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &Value,
        activity_type: &str,
        message: &str,
    ) -> QefroResult<()> {
        if entity.get_field(RELATED_TYPE_FIELD).is_none() {
            return Ok(());
        }
        let Some(type_name) = record
            .get(RELATED_TYPE_FIELD)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != entity.name)
        else {
            return Ok(());
        };
        let Some(related_id) = record
            .get(RELATED_ID_FIELD)
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            return Ok(());
        };
        let Ok(target) = self.registry.get(type_name) else {
            return Ok(());
        };
        if !target.activity {
            return Ok(());
        }
        self.activity
            .record_tx(
                tx,
                ctx,
                &target.name,
                related_id,
                crate::activity::TYPE_SYSTEM,
                message,
                json!({
                    "source_entity": entity.name,
                    "activity_type": activity_type,
                    "title": record.get("title"),
                }),
            )
            .await?;
        Ok(())
    }
}

struct RelatedSpec {
    name: String,
    label: String,
    target: Arc<qefro_core::EntityDef>,
    inverse: String,
}

fn print_relation_aliases(format: &qefro_core::PrintFormat) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    let mut push_path = |path: &str| {
        if let Some(first) = path.split('.').next() {
            let first = first.trim();
            if !first.is_empty() {
                names.insert(first.to_string());
                names.insert(format!("{first}_id"));
            }
        }
    };
    if let Some(body) = &format.body {
        for path in qefro_core::template_paths(body) {
            push_path(&path);
        }
    }
    for section in &format.sections {
        if let Some(rel) = &section.relation {
            push_path(rel);
        }
        for field in &section.fields {
            push_path(field);
        }
        if let Some(text) = &section.text {
            for path in qefro_core::template_paths(text) {
                push_path(&path);
            }
        }
        if let Some(when) = &section.show_when {
            push_path(when.split(['>', '<', '=', '!']).next().unwrap_or(""));
        }
    }
    names
}

fn relation_expansion(
    target: &qefro_core::EntityDef,
    id: Uuid,
    related: &Value,
    nested: serde_json::Map<String, Value>,
) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("id".into(), json!(id));
    map.insert("label".into(), json!(target.display_label(related)));
    map.insert("slug".into(), json!(target.slug.clone()));
    map.insert("entity".into(), json!(target.name.clone()));
    if let Some(enabled) = related.get("enabled") {
        if !enabled.is_null() {
            map.insert("enabled".into(), enabled.clone());
        }
    }
    if !nested.is_empty() {
        map.insert("_expanded".into(), Value::Object(nested));
    }
    Value::Object(map)
}

fn child_rows_query(
    child: &qefro_core::EntityDef,
    inverse: &str,
    parent_id: &str,
) -> qefro_search::Query {
    use qefro_search::{Filter, Query, Sort, SortDir};
    let mut query = Query::default();
    query.filters.push(Filter::Eq {
        field: inverse.to_string(),
        value: json!(parent_id),
    });
    if child.get_field("sort_order").is_some() {
        query.sort.push(Sort {
            field: "sort_order".into(),
            dir: SortDir::Asc,
        });
    }
    query.sort.push(Sort {
        field: "created_at".into(),
        dir: SortDir::Asc,
    });
    query
}

fn extract_children(
    entity: &qefro_core::EntityDef,
    data: &mut Value,
) -> std::collections::HashMap<String, Vec<Value>> {
    let mut out = std::collections::HashMap::new();
    let Some(obj) = data.as_object_mut() else {
        return out;
    };
    for field in &entity.fields {
        if !field.is_child_table() {
            continue;
        }
        if let Some(Value::Array(rows)) = obj.remove(&field.name) {
            out.insert(field.name.clone(), rows);
        }
    }
    out
}

fn strip_computed(entity: &qefro_core::EntityDef, data: &mut Value) {
    strip_computed_fields(&entity.fields, data);
    strip_server_managed_fields(&entity.fields, data);
}

fn coerce_numeric_json(entity: &qefro_core::EntityDef, record: &mut Value) {
    let Some(obj) = record.as_object_mut() else {
        return;
    };
    for field in entity
        .fields
        .iter()
        .filter(|f| f.stores_column() || f.custom)
    {
        if !field.field_type.is_numeric() {
            continue;
        }
        let Some(Value::String(raw)) = obj.get(&field.name) else {
            continue;
        };
        if let Ok(n) = raw.parse::<f64>() {
            obj.insert(field.name.clone(), json!(n));
        }
    }
}

fn prepare_record(entity: &qefro_core::EntityDef, data: &mut Value, ctx: &OpContext) {
    qefro_core::flatten_nested_custom(entity, data);
    apply_defaults(entity, data, ctx);
    canonicalize_values(entity, data, ctx);
    sanitize_values(entity, data);
    crate::scheduling::prepare_record(entity, data, &ctx.timezone);
}

fn apply_defaults(entity: &qefro_core::EntityDef, data: &mut Value, ctx: &OpContext) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in entity.fields.iter().filter(|f| {
        !f.system
            && !f.computed
            && (f.stores_column() || (f.custom && f.custom_status.in_effective_metadata()))
    }) {
        let missing = match obj.get(&field.name) {
            None => true,
            Some(Value::Null) => true,
            Some(Value::String(s)) if s.is_empty() => true,
            _ => false,
        };
        if !missing {
            continue;
        }
        if let Some(source) = &field.default_from {
            let value = match source.as_str() {
                "current_user" => json!(ctx.user_id.to_string()),
                "current_date" => json!(Utc::now().date_naive().to_string()),
                "current_datetime" => json!(Utc::now().to_rfc3339()),
                "tenant_timezone" => json!(ctx.timezone.clone()),
                "tenant_currency" => json!(ctx.currency.clone()),
                _ => continue,
            };
            obj.insert(field.name.clone(), value);
        } else if let Some(default) = &field.default {
            obj.insert(field.name.clone(), default.clone());
        }
    }
}

fn canonicalize_values(entity: &qefro_core::EntityDef, data: &mut Value, ctx: &OpContext) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in entity
        .fields
        .iter()
        .filter(|f| f.stores_column() || f.custom)
    {
        let Some(Value::String(raw)) = obj.get(&field.name) else {
            continue;
        };
        let raw = raw.clone();
        match field.field_type {
            FieldType::DateTime => {
                let tz = field
                    .ui
                    .widget_options
                    .timezone
                    .as_deref()
                    .filter(|tz| *tz != "utc")
                    .map(|_| ctx.timezone.as_str())
                    .unwrap_or("UTC");
                if let Some(dt) = canonicalize_datetime(&raw, tz) {
                    obj.insert(field.name.clone(), json!(dt.to_rfc3339()));
                }
            }
            FieldType::Time => {
                if raw.len() == 5 {
                    obj.insert(field.name.clone(), json!(format!("{raw}:00")));
                }
            }
            _ => {}
        }
    }
}

fn sanitize_values(entity: &qefro_core::EntityDef, data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in entity.stored_fields() {
        if !field.is_rich_text() {
            continue;
        }
        if let Some(Value::String(html)) = obj.get(&field.name) {
            let clean = sanitize_html(html);
            obj.insert(field.name.clone(), json!(clean));
        }
    }
}

fn reject_client_tenant(data: &Value) -> QefroResult<()> {
    if data.get("tenant_id").is_some() {
        return Err(QefroError::bad_request(
            "tenant_id cannot be set by the client",
        ));
    }
    Ok(())
}

fn snake(name: &str) -> String {
    qefro_core::ident::snake_case(name)
}

fn resolve_query_placeholders(query: &mut qefro_search::Query, ctx: &OpContext) {
    use qefro_search::Filter;
    let me = json!(ctx.user_id.to_string());
    let now = json!(Utc::now().to_rfc3339());
    for filter in &mut query.filters {
        match filter {
            Filter::Eq { value, .. }
            | Filter::Neq { value, .. }
            | Filter::Gt { value, .. }
            | Filter::Gte { value, .. }
            | Filter::Lt { value, .. }
            | Filter::Lte { value, .. } => {
                if let Some(s) = value.as_str() {
                    match s {
                        "current_user" | "me" => *value = me.clone(),
                        "now" => *value = now.clone(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn mutation_events(
    entity: &str,
    id: Uuid,
    ctx: &OpContext,
    mut payload: Value,
    specific: &str,
    framework: &str,
) -> Vec<DomainEvent> {
    strip_secrets(None, &mut payload);
    if ctx.is_automation() {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "_automation_depth".into(),
                json!(ctx.automation_depth.saturating_add(1)),
            );
        }
    }
    let specific_name = if specific.contains('.') {
        specific.to_string()
    } else {
        format!("{}.{}", snake(entity), specific)
    };
    vec![
        DomainEvent::new(
            specific_name,
            entity.to_string(),
            id,
            ctx.tenant_id,
            payload.clone(),
        )
        .with_user(ctx.user_id),
        DomainEvent::new(
            framework.to_string(),
            entity.to_string(),
            id,
            ctx.tenant_id,
            payload,
        )
        .with_user(ctx.user_id),
    ]
}

trait WithUser {
    fn with_user(self, user_id: Uuid) -> Self;
}

impl WithUser for DomainEvent {
    fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }
}

fn normalize_card_kind(kind: &str) -> String {
    match kind {
        "" => "metric".into(),
        "kpi" => "kpi".into(),
        "workflow" => "workflow".into(),
        other => other.into(),
    }
}
