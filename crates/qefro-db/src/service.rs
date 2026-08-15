use crate::audit::AuditLogger;
use crate::jobs::{JobQueue, JobRegistry};
use crate::operation::{
    available_for_record, crud_operation_defs, execute_operation, operation_allowed,
    OperationRegistry,
};
use crate::outbox::Outbox;
use crate::repository::{record_id, EntityRepository, Page};
use qefro_core::{
    canonicalize_datetime, sanitize_html, validate_record, EntityRegistry, FieldError, FieldType,
    HookRegistry, OpContext, OperationDef, QefroError, QefroResult,
};
use chrono::Utc;
use qefro_events::{DomainEvent, EventBus, InProcessEventBus};
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
    registry: Arc<EntityRegistry>,
    repo: Arc<EntityRepository>,
    permissions: Arc<PermissionRegistry>,
    workflows: Arc<WorkflowRegistry>,
    hooks: Arc<HookRegistry>,
    events: InProcessEventBus,
    audit: Arc<AuditLogger>,
    operations: Arc<OperationRegistry>,
    jobs: Arc<JobQueue>,
    job_handlers: Arc<JobRegistry>,
    outbox: Outbox,
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
            outbox: Outbox::new(pool),
            registry,
            permissions,
            workflows,
            hooks,
            events,
            operations: Arc::new(OperationRegistry::new()),
            job_handlers: Arc::new(JobRegistry::new()),
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

    pub async fn execute(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        name: &str,
        input: Value,
    ) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        reject_client_tenant(&input)?;
        let (record, _events) = execute_operation(
            &self.repo,
            &self.registry,
            &self.permissions,
            &self.workflows,
            &self.hooks,
            &self.operations,
            &self.jobs,
            &self.audit,
            ctx,
            &entity.name,
            id,
            name,
            input,
        )
        .await?;
        let _ = self.dispatch_outbox().await;
        self.present(ctx, &entity, record).await
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
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        let query = query.sanitize(&entity)?;
        let mut page = self.repo.list(&entity, ctx, &query).await?;
        for item in &mut page.items {
            coerce_numeric_json(&entity, item);
            self.strip_forbidden_fields(ctx, &entity, item);
        }
        self.expand_many_to_one_batch(ctx, &entity, &mut page.items)
            .await?;
        for item in &mut page.items {
            self.attach_workflow(ctx, &entity, item);
        }
        Ok(page)
    }

    pub async fn get(&self, ctx: &OpContext, entity_name: &str, id: Uuid) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Read)?;
        let record = self.repo.get(&entity, ctx, id).await?;
        self.present(ctx, &entity, record).await
    }

    pub async fn create(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        mut data: Value,
    ) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Create)?;
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
        let children = extract_children(&entity, &mut data);
        strip_computed(&entity, &mut data);
        prepare_record(&entity, &mut data, ctx);
        if let Some(wf) = self.workflows.for_entity(&entity.name) {
            if data.get(&wf.field).and_then(|v| v.as_str()).is_none() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(wf.field.clone(), json!(wf.initial));
                }
            }
        }
        validate_record(entity.business_fields(), &data, false)?;
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
        let event = DomainEvent::new(
            format!("{}.created", snake(&entity.name)),
            entity.name.clone(),
            id,
            ctx.tenant_id,
            created.clone(),
        )
        .with_user(ctx.user_id);
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(&mut tx, ctx, &entity.name, Some(id), "create", None, Some(&created))
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = Outbox::enqueue_tx(&mut tx, &event).await {
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
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Update)?;
        reject_client_tenant(&patch)?;
        self.reject_forbidden_writes(ctx, &entity, &patch)?;
        let children = extract_children(&entity, &mut patch);
        strip_computed(&entity, &mut patch);
        canonicalize_values(&entity, &mut patch, ctx);
        sanitize_values(&entity, &mut patch);
        let current = self.repo.get(&entity, ctx, id).await?;
        if let Some(doc) = &entity.document {
            let status = current
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
        let event = DomainEvent::new(
            format!("{}.updated", snake(&entity.name)),
            entity.name.clone(),
            id,
            ctx.tenant_id,
            updated.clone(),
        )
        .with_user(ctx.user_id);
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
        if let Err(e) = Outbox::enqueue_tx(&mut tx, &event).await {
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
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Delete)?;
        let current = self.repo.get(&entity, ctx, id).await?;
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
        let event = DomainEvent::new(
            format!("{}.deleted", snake(&entity.name)),
            entity.name.clone(),
            id,
            ctx.tenant_id,
            deleted.clone(),
        )
        .with_user(ctx.user_id);
        if entity.audit {
            if let Err(e) = self
                .audit
                .record_tx(&mut tx, ctx, &entity.name, Some(id), "delete", Some(&current), None)
                .await
            {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }
        if let Err(e) = Outbox::enqueue_tx(&mut tx, &event).await {
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
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        if self.operations.try_get(&entity.name, transition).is_some() {
            return self
                .execute(ctx, &entity.name, id, transition, json!({}))
                .await;
        }
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Update)?;
        let current = self.repo.get(&entity, ctx, id).await?;
        let wf = self
            .workflows
            .for_entity(&entity.name)
            .ok_or_else(|| QefroError::not_found(format!("no workflow for {}", entity.name)))?;
        let from = current
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial);
        let to = self.workflows.apply(&entity.name, from, transition, ctx)?;
        let patch = json!({ wf.field.clone(): to });
        let updated = self.repo.update(&entity, ctx, id, patch).await?;
        if entity.audit {
            self.audit
                .record(
                    ctx,
                    &entity.name,
                    Some(id),
                    &format!("transition:{transition}"),
                    Some(&current),
                    Some(&updated),
                )
                .await?;
        }
        self.events
            .publish(
                DomainEvent::new(
                    format!("{}.{transition}", snake(&entity.name)),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    updated.clone(),
                )
                .with_user(ctx.user_id),
            )
            .await?;
        Ok(self.present(ctx, &entity, updated).await?)
    }

    async fn present(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        mut record: Value,
    ) -> QefroResult<Value> {
        coerce_numeric_json(entity, &mut record);
        self.expand_many_to_one(ctx, entity, &mut record).await?;
        self.expand_one_to_many(ctx, entity, &mut record).await?;
        self.expand_child_tables(ctx, entity, &mut record).await?;
        self.attach_workflow(ctx, entity, &mut record);
        self.attach_actions(ctx, entity, &mut record);
        self.attach_links(ctx, entity, &mut record).await?;
        self.strip_forbidden_fields(ctx, entity, &mut record);
        Ok(record)
    }

    async fn check_uniques(
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
        let actions: Vec<Value> = self
            .record_actions(ctx, &entity.name, record)
            .into_iter()
            .map(|mut d| {
                if let Some(meta) = entity.actions.iter().find(|a| {
                    a.name == d.name || a.operation == d.name || a.name == d.workflow_transition.clone().unwrap_or_default()
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
        if let Some(obj) = record.as_object_mut() {
            obj.insert("_actions".into(), json!(actions));
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
        for field in &entity.fields {
            if let Some(rel) = &field.relation {
                if rel.kind == qefro_core::RelationKind::OneToMany {
                    if let Some(inverse) = &rel.inverse_field {
                        if seen.insert(rel.target_entity.clone()) {
                            defs.push(qefro_core::LinkDef::new(
                                field.ui.label.clone(),
                                rel.target_entity.clone(),
                                inverse.clone(),
                            ));
                        }
                    }
                }
            }
        }
        for link in defs {
            if !seen.insert(format!("{}:{}", link.entity, link.relation)) {
                continue;
            }
            let Ok(target) = self.registry.get(&link.entity) else {
                continue;
            };
            if self.permissions.check(ctx, &target.name, Action::List).is_err() {
                continue;
            }
            let mut query = qefro_search::Query::default();
            query.page_size = 1;
            query.filters.push(qefro_search::Filter::Eq {
                field: link.relation.clone(),
                value: id.clone(),
            });
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
                    format!("{} cannot be edited in a locked document state", field.label),
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
                format!("{} cannot be edited in a locked document state", field.label),
            ));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(QefroError::locked(errors))
        }
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

    fn workflow_json(&self, ctx: &OpContext, entity: &qefro_core::EntityDef, record: &Value) -> Value {
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
                    "name": t.name,
                    "label": if t.label.is_empty() { t.name.clone() } else { t.label.clone() },
                    "from": t.from,
                    "to": t.to,
                    "allowed_roles": t.allowed_roles,
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
            if self.permissions.check(ctx, &target.name, Action::Read).is_err() {
                continue;
            }
            if let Ok(related) = self.repo.get(&target, ctx, id).await {
                expansions.insert(
                    field.name.clone(),
                    json!({
                        "id": id,
                        "label": target.display_label(&related),
                        "slug": target.slug,
                        "entity": target.name,
                    }),
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

    async fn expand_many_to_one_batch(
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
            if self.permissions.check(ctx, &target.name, Action::Read).is_err() {
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
            let mut labels = std::collections::HashMap::new();
            for related in fetched {
                if let Ok(id) = record_id(&related) {
                    labels.insert(id, (target.display_label(&related), related));
                }
            }
            for record in records.iter_mut() {
                let Some(id_str) = record.get(&field.name).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Ok(id) = Uuid::parse_str(id_str) else { continue };
                if let Some((label, _)) = labels.get(&id) {
                    if let Some(obj) = record.as_object_mut() {
                        let expanded = obj
                            .entry("_expanded")
                            .or_insert_with(|| json!({}));
                        if let Some(map) = expanded.as_object_mut() {
                            map.insert(
                                field.name.clone(),
                                json!({
                                    "id": id,
                                    "label": label,
                                    "slug": target.slug,
                                    "entity": target.name,
                                }),
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
        use qefro_core::RelationKind;
        use qefro_search::{Filter, Query};
        let Some(id) = record.get("id").and_then(|v| v.as_str()) else {
            return Ok(());
        };
        let mut related = serde_json::Map::new();
        for field in &entity.fields {
            let Some(rel) = &field.relation else { continue };
            if rel.kind != RelationKind::OneToMany {
                continue;
            }
            let Some(inverse) = &rel.inverse_field else { continue };
            let Ok(target) = self.registry.get(&rel.target_entity) else {
                continue;
            };
            if self.permissions.check(ctx, &target.name, Action::List).is_err() {
                continue;
            }
            let mut query = Query::default();
            query.page_size = 50;
            query.filters.push(Filter::Eq {
                field: inverse.clone(),
                value: json!(id),
            });
            if let Ok(page) = self.repo.list(&target, ctx, &query).await {
                related.insert(
                    field.name.clone(),
                    json!({
                        "entity": target.name,
                        "slug": target.slug,
                        "items": page.items,
                        "total": page.total,
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

    async fn expand_child_tables(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) -> QefroResult<()> {
        use qefro_core::RelationKind;
        let Some(id) = record.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
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
                if let Err(qefro_core::QefroError::Validation { fields, .. }) =
                    validate_record(&fields, row, partial)
                {
                    for err in fields {
                        errors.push(FieldError::new(
                            format!("{}.{}.{}", field.name, i, err.field),
                            err.code,
                            err.message,
                        ));
                    }
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
        let entity = self.registry.get(&report.entity)?;
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
        let parsed = crate::reports::filters_from_json(&entity, &filters)?;
        let mut query = qefro_search::Query::default();
        query.filters = parsed;
        query.page_size = 500;
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
    ) -> QefroResult<(qefro_core::PrintFormat, Value, Vec<Value>)> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::Read)?;
        let record = self.get(ctx, entity_name, id).await?;
        let format = entity
            .print_formats
            .iter()
            .find(|f| format_name.map(|n| f.name == n).unwrap_or(true))
            .cloned()
            .or_else(|| {
                Some(qefro_core::PrintFormat::new(
                    format!("{} Standard", entity.label),
                    &entity.name,
                ))
            })
            .unwrap();
        let items = entity
            .fields
            .iter()
            .find(|f| f.is_child_table())
            .and_then(|f| record.get(&f.name))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok((format, record, items))
    }

    pub async fn dashboard_card_value(
        &self,
        ctx: &OpContext,
        card: &qefro_core::DashboardCard,
    ) -> QefroResult<Value> {
        use qefro_search::parse_query;
        let entity = self.registry.get(&card.entity)?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::List)?;
        let mut raw: Vec<(String, String)> = card
            .filters
            .iter()
            .map(|f| {
                let value = if f.value == "today" {
                    chrono::Utc::now().date_naive().to_string()
                } else {
                    f.value.clone()
                };
                (f.field.clone(), value)
            })
            .collect();
        raw.push(("page_size".into(), "1".into()));
        let query = parse_query(&entity, &raw)?;
        let kind = if card.kind.is_empty() {
            "metric"
        } else {
            card.kind.as_str()
        };
        if matches!(kind, "chart" | "status_breakdown") {
            let group_by = card.group_by.as_deref().ok_or_else(|| {
                QefroError::bad_request("chart cards require group_by")
            })?;
            let series = self
                .repo
                .aggregate_group(&entity, ctx, &query, group_by)
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
                "series": series,
                "value": value,
            }));
        }
        if matches!(kind, "list" | "table" | "activity") {
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
            "kind": "metric",
            "value": value,
        }))
    }

    fn ensure_app(&self, ctx: &OpContext, entity: &qefro_core::EntityDef) -> QefroResult<()> {
        if ctx.allows_app(entity.module.as_deref()) {
            Ok(())
        } else {
            Err(QefroError::not_found(format!("{} not found", entity.name)))
        }
    }

    fn reject_worker_crud(&self, ctx: &OpContext) -> QefroResult<()> {
        if ctx.is_worker() {
            Err(QefroError::forbidden(
                "workers cannot perform generic entity mutations",
            ))
        } else {
            Ok(())
        }
    }
}

fn child_rows_query(child: &qefro_core::EntityDef, inverse: &str, parent_id: &str) -> qefro_search::Query {
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
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in &entity.fields {
        if field.computed {
            obj.remove(&field.name);
        }
    }
}

fn coerce_numeric_json(entity: &qefro_core::EntityDef, record: &mut Value) {
    let Some(obj) = record.as_object_mut() else {
        return;
    };
    for field in entity.stored_fields() {
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
    apply_defaults(entity, data, ctx);
    canonicalize_values(entity, data, ctx);
    sanitize_values(entity, data);
}

fn apply_defaults(entity: &qefro_core::EntityDef, data: &mut Value, ctx: &OpContext) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in entity.stored_fields() {
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
    for field in entity.stored_fields() {
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

trait WithUser {
    fn with_user(self, user_id: Uuid) -> Self;
}

impl WithUser for DomainEvent {
    fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }
}
