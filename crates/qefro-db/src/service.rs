use crate::audit::AuditLogger;
use crate::jobs::{JobQueue, JobRegistry};
use crate::operation::{
    available_for_record, crud_operation_defs, execute_operation, operation_allowed,
    OperationRegistry,
};
use crate::repository::{record_id, EntityRepository, Page};
use qefro_core::{
    validate_record, EntityRegistry, FieldError, HookRegistry, OpContext, OperationDef, QefroError,
    QefroResult,
};
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
            audit: Arc::new(AuditLogger::new(pool)),
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
        let (record, events) = execute_operation(
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
        for event in events {
            self.events.publish(event).await?;
        }
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
        reject_client_tenant(&data)?;
        apply_defaults(&entity, &mut data);
        if let Some(wf) = self.workflows.for_entity(&entity.name) {
            if data.get(&wf.field).and_then(|v| v.as_str()).is_none() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(wf.field.clone(), json!(wf.initial));
                }
            }
        }
        validate_record(entity.business_fields(), &data, false)?;
        self.check_uniques(ctx, &entity, &data, None).await?;
        self.hooks
            .before_create(ctx, &entity.name, &mut data)
            .await?;
        let created = self.repo.insert(&entity, ctx, data).await?;
        let id = record_id(&created)?;
        if entity.audit {
            self.audit
                .record(ctx, &entity.name, Some(id), "create", None, Some(&created))
                .await?;
        }
        self.hooks.after_create(ctx, &entity.name, &created).await?;
        self.events
            .publish(
                DomainEvent::new(
                    format!("{}.created", snake(&entity.name)),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    created.clone(),
                )
                .with_user(ctx.user_id),
            )
            .await?;
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
        let current = self.repo.get(&entity, ctx, id).await?;
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
        self.check_uniques(ctx, &entity, &patch, Some(id)).await?;
        self.hooks
            .before_update(ctx, &entity.name, &current, &mut patch)
            .await?;
        let updated = self.repo.update(&entity, ctx, id, patch).await?;
        if entity.audit {
            self.audit
                .record(
                    ctx,
                    &entity.name,
                    Some(id),
                    "update",
                    Some(&current),
                    Some(&updated),
                )
                .await?;
        }
        self.hooks.after_update(ctx, &entity.name, &updated).await?;
        self.events
            .publish(
                DomainEvent::new(
                    format!("{}.updated", snake(&entity.name)),
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

    pub async fn delete(&self, ctx: &OpContext, entity_name: &str, id: Uuid) -> QefroResult<Value> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        self.permissions.check(ctx, &entity.name, Action::Delete)?;
        let current = self.repo.get(&entity, ctx, id).await?;
        self.hooks
            .before_delete(ctx, &entity.name, &current)
            .await?;
        let deleted = self.repo.delete(&entity, ctx, id).await?;
        if entity.audit {
            self.audit
                .record(ctx, &entity.name, Some(id), "delete", Some(&current), None)
                .await?;
        }
        self.hooks.after_delete(ctx, &entity.name, &deleted).await?;
        self.events
            .publish(
                DomainEvent::new(
                    format!("{}.deleted", snake(&entity.name)),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    deleted.clone(),
                )
                .with_user(ctx.user_id),
            )
            .await?;
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
        self.expand_many_to_one(ctx, entity, &mut record).await?;
        self.expand_one_to_many(ctx, entity, &mut record).await?;
        self.attach_workflow(ctx, entity, &mut record);
        self.attach_actions(ctx, entity, &mut record);
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
            .map(|d| d.to_client_json())
            .collect();
        if let Some(obj) = record.as_object_mut() {
            obj.insert("_actions".into(), json!(actions));
        }
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
        let value = self
            .repo
            .aggregate(&entity, ctx, &query, &card.metric, card.field.as_deref())
            .await?;
        Ok(json!({
            "title": card.title,
            "entity": card.entity,
            "metric": card.metric,
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

fn apply_defaults(entity: &qefro_core::EntityDef, data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in entity.stored_fields() {
        if !obj.contains_key(&field.name) {
            if let Some(default) = &field.default {
                obj.insert(field.name.clone(), default.clone());
            }
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
