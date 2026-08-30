use crate::jobs::{JobHandler, JobQueue};
use crate::operation_run::{
    OperationRun, OperationRunStore, OPERATION_EXECUTE_JOB, STATUS_COMPLETED, STATUS_FAILED,
    STATUS_QUEUED, STATUS_RUNNING,
};
use crate::repository::{record_id, EntityRepository};
use async_trait::async_trait;
use qefro_core::{
    apply_entity_rules, ident::snake_case, validate_record, EntityRegistry, HookRegistry,
    MeteringEvent, OpContext, OperationDef, QefroError, QefroResult, RELATED_ID_FIELD,
    RELATED_TYPE_FIELD, RowPolicy,
};
use qefro_events::DomainEvent;
use qefro_permissions::{Action, PermissionRegistry};
use qefro_search::{Filter, Query};
use qefro_workflow::WorkflowRegistry;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use uuid::Uuid;

const MAX_OPERATION_DEPTH: usize = 8;

#[async_trait]
pub trait OperationHandler: Send + Sync {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value>;
}

pub struct NoopOperationHandler;

#[async_trait]
impl OperationHandler for NoopOperationHandler {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        Ok(ctx.record.clone())
    }
}

pub struct OperationBinding {
    pub def: OperationDef,
    pub handler: Arc<dyn OperationHandler>,
}

#[derive(Default, Clone)]
pub struct OperationRegistry {
    by_entity: HashMap<String, HashMap<String, Arc<OperationBinding>>>,
}

impl OperationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: OperationDef, handler: Arc<dyn OperationHandler>) {
        self.by_entity
            .entry(def.entity.clone())
            .or_default()
            .insert(
                def.name.clone(),
                Arc::new(OperationBinding { def, handler }),
            );
    }

    pub fn try_get(&self, entity: &str, name: &str) -> Option<Arc<OperationBinding>> {
        self.by_entity.get(entity).and_then(|m| {
            m.get(name).cloned().or_else(|| {
                m.values()
                    .find(|b| {
                        b.def.tool_name == name
                            || b.def.workflow_transition.as_deref() == Some(name)
                    })
                    .cloned()
            })
        })
    }

    pub fn get(&self, entity: &str, name: &str) -> QefroResult<Arc<OperationBinding>> {
        self.try_get(entity, name)
            .ok_or_else(|| QefroError::not_found(format!("operation '{entity}.{name}' not found")))
    }

    pub fn for_entity(&self, entity: &str) -> Vec<Arc<OperationBinding>> {
        let mut items: Vec<_> = self
            .by_entity
            .get(entity)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        items.sort_by(|a, b| a.def.name.cmp(&b.def.name));
        items
    }

    pub fn all(&self) -> Vec<Arc<OperationBinding>> {
        let mut items = Vec::new();
        for map in self.by_entity.values() {
            items.extend(map.values().cloned());
        }
        items.sort_by(|a, b| {
            a.def
                .entity
                .cmp(&b.def.entity)
                .then(a.def.name.cmp(&b.def.name))
        });
        items
    }
}

/// Options for [`execute_operation`].
#[derive(Debug, Clone, Default)]
pub struct ExecuteOpts {
    pub idempotency_key: Option<String>,
    /// When true, run even if the def is `async` (used by the JobQueue worker).
    pub force_sync: bool,
    pub operation_id: Option<Uuid>,
}

/// Handler context. Auth, tenant, and RBAC are already enforced for the
/// primary operation. Nested `create`/`update`/`delete`/`execute` re-check
/// entity, action, workflow, and row-policy permissions on the same SQLx
/// transaction — never a second connection and never an elevated user.
pub struct OperationCtx<'a, 'conn: 'a> {
    pub auth: OpContext,
    pub def: OperationDef,
    pub entity: Arc<qefro_core::EntityDef>,
    pub record: Value,
    pub input: Value,
    pub pending_events: Vec<DomainEvent>,
    pub pending_jobs: Vec<(String, Value)>,
    pub operation_id: Uuid,
    result_message: Option<String>,
    result_navigate: Option<Value>,
    call_stack: Vec<String>,
    registry: &'a EntityRegistry,
    permissions: &'a PermissionRegistry,
    workflows: &'a WorkflowRegistry,
    hooks: &'a HookRegistry,
    operations: &'a OperationRegistry,
    repo: &'a EntityRepository,
    audit: &'a crate::audit::AuditLogger,
    activity: &'a crate::activity::ActivityStore,
    jobs: &'a JobQueue,
    tx: &'a mut Transaction<'conn, Postgres>,
}

impl<'a, 'conn: 'a> OperationCtx<'a, 'conn> {
    pub fn record_id(&self) -> QefroResult<Uuid> {
        record_id(&self.record)
    }

    pub fn fail(code: &str, message: impl Into<String>) -> QefroError {
        QefroError::business(code, message)
    }

    pub fn status(&self) -> &str {
        self.record
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    pub fn uuid_field(&self, field: &str) -> QefroResult<Uuid> {
        self.record
            .get(field)
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request(format!("{field} is required")))
    }

    pub fn set_field(&mut self, field: &str, value: Value) {
        if let Some(obj) = self.record.as_object_mut() {
            obj.insert(field.to_string(), value);
        }
    }

    pub fn emit(&mut self, name: impl Into<String>, payload: Value) {
        if let Ok(id) = self.record_id() {
            let mut event = DomainEvent::new(
                name,
                self.entity.name.clone(),
                id,
                self.auth.tenant_id,
                payload,
            );
            event.user_id = Some(self.auth.user_id);
            correlate_event(&mut event, self.operation_id, self.auth.request_id);
            self.pending_events.push(event);
        }
    }

    pub fn enqueue_job(&mut self, name: impl Into<String>, payload: Value) {
        self.pending_jobs.push((name.into(), payload));
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.result_message = Some(message.into());
    }

    pub fn set_navigate(&mut self, entity: &str, id: Uuid) {
        let slug = self
            .registry
            .get(entity)
            .map(|d| d.slug.clone())
            .unwrap_or_else(|_| entity.to_string());
        self.result_navigate = Some(json!({
            "entity": entity,
            "slug": slug,
            "id": id,
        }));
    }

    pub async fn set_progress(&mut self, progress: i32) -> QefroResult<()> {
        OperationRunStore::set_progress_tx(self.tx, &self.auth, self.operation_id, progress).await
    }

    pub fn apply_transition(&mut self, name: &str) -> QefroResult<String> {
        apply_transition_to(
            self.workflows,
            &self.auth,
            &self.entity.name,
            &mut self.record,
            name,
        )
    }

    pub async fn get(&mut self, entity: &str, id: Uuid) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.permissions.check(&self.auth, &def.name, Action::Read)?;
        let record = self
            .repo
            .get_tx(self.tx, &def, &self.auth, id, true)
            .await?;
        enforce_row_policy(&self.auth, &def, &record)?;
        Ok(record)
    }

    pub async fn update(&mut self, entity: &str, id: Uuid, mut patch: Value) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.permissions
            .check(&self.auth, &def.name, Action::Update)?;
        reject_client_tenant(&patch)?;
        let current = self
            .repo
            .get_tx(self.tx, &def, &self.auth, id, true)
            .await?;
        enforce_row_policy(&self.auth, &def, &current)?;
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
        if let Some(wf) = self.workflows.for_entity(&def.name) {
            if patch
                .as_object()
                .map(|o| o.contains_key(&wf.field))
                .unwrap_or(false)
            {
                return Err(QefroError::bad_request(format!(
                    "field '{}' is workflow-managed; use a transition",
                    wf.field
                )));
            }
        }
        validate_record(def.business_fields(), &patch, true)?;
        let mut merged = current.clone();
        if let (Some(dst), Some(src)) = (merged.as_object_mut(), patch.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        apply_entity_rules(def.business_fields(), &def.validation, &merged, true)?;
        self.check_cross_entity_refs(&def, &merged).await?;
        let updated = self
            .repo
            .update_tx(self.tx, &def, &self.auth, id, patch)
            .await?;
        self.record_side_effects(&def, Some(&current), &updated, "update")
            .await?;
        Ok(updated)
    }

    pub async fn create(&mut self, entity: &str, mut data: Value) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.permissions
            .check(&self.auth, &def.name, Action::Create)?;
        reject_client_tenant(&data)?;
        apply_op_defaults(&def, &mut data, &self.auth);
        if let Some(wf) = self.workflows.for_entity(&def.name) {
            if data.get(&wf.field).and_then(|v| v.as_str()).is_none() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert(wf.field.clone(), json!(wf.initial));
                }
            }
        }
        validate_record(def.business_fields(), &data, false)?;
        apply_entity_rules(def.business_fields(), &def.validation, &data, false)?;
        self.check_cross_entity_refs(&def, &data).await?;
        let created = self
            .repo
            .insert_tx(self.tx, &def, &self.auth, data)
            .await?;
        self.record_side_effects(&def, None, &created, "create")
            .await?;
        Ok(created)
    }

    pub async fn create_many(&mut self, entity: &str, rows: Vec<Value>) -> QefroResult<Vec<Value>> {
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(self.create(entity, row).await?);
        }
        Ok(out)
    }

    pub async fn delete(&mut self, entity: &str, id: Uuid) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.permissions
            .check(&self.auth, &def.name, Action::Delete)?;
        let current = self
            .repo
            .get_tx(self.tx, &def, &self.auth, id, true)
            .await?;
        enforce_row_policy(&self.auth, &def, &current)?;
        let deleted = self
            .repo
            .delete_tx(self.tx, &def, &self.auth, id)
            .await?;
        self.record_side_effects(&def, Some(&current), &deleted, "delete")
            .await?;
        Ok(deleted)
    }

    /// Invoke another registered operation on the same transaction.
    /// Cycles (`A → B → A`) are rejected.
    pub async fn execute(
        &mut self,
        entity: &str,
        id: Uuid,
        name: &str,
        input: Value,
    ) -> QefroResult<Value> {
        let key = format!("{entity}.{name}");
        if self.call_stack.iter().any(|s| s == &key) {
            return Err(QefroError::bad_request(format!(
                "operation cycle detected involving {key}"
            )));
        }
        if self.call_stack.len() >= MAX_OPERATION_DEPTH {
            return Err(QefroError::bad_request(
                "operation nesting exceeded the maximum depth",
            ));
        }
        reject_client_tenant(&input)?;
        let binding = self.operations.get(entity, name)?;
        if binding.def.is_async() {
            return Err(QefroError::bad_request(
                "asynchronous operations cannot be nested inside a transaction",
            ));
        }
        authorize_operation(self.permissions, &self.auth, &binding.def)?;
        let nested_entity = self.registry.get(entity)?;
        let mut stack = self.call_stack.clone();
        stack.push(key);
        let (record, events, jobs, _envelope) = execute_in_transaction(
            self.tx,
            self.repo,
            self.registry,
            self.permissions,
            self.workflows,
            self.hooks,
            self.operations,
            &binding,
            &nested_entity,
            &self.auth,
            id,
            input,
            self.audit,
            self.activity,
            self.jobs,
            self.operation_id,
            stack,
            false,
        )
        .await?;
        self.pending_events.extend(events);
        let _ = jobs;
        Ok(record)
    }

    pub async fn list(
        &mut self,
        entity: &str,
        field: &str,
        value: Value,
    ) -> QefroResult<Vec<Value>> {
        let def = self.registry.get(entity)?;
        self.permissions.check(&self.auth, &def.name, Action::List)?;
        let mut query = Query::default();
        query.page_size = 100;
        query.filters.push(Filter::Eq {
            field: field.into(),
            value,
        });
        let page = self
            .repo
            .list_tx(self.tx, &def, &self.auth, &query)
            .await?;
        Ok(page.items)
    }

    pub fn entity_def(&self, name: &str) -> QefroResult<std::sync::Arc<qefro_core::EntityDef>> {
        self.registry.get(name)
    }

    async fn check_cross_entity_refs(
        &mut self,
        entity: &qefro_core::EntityDef,
        data: &Value,
    ) -> QefroResult<()> {
        for field in entity.stored_fields() {
            let Some(rel) = &field.relation else {
                continue;
            };
            if rel.kind != qefro_core::RelationKind::ManyToOne {
                continue;
            }
            let Some(raw) = data.get(&field.name) else {
                continue;
            };
            if raw.is_null() {
                continue;
            }
            let Some(id) = raw.as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
                return Err(QefroError::bad_request(format!(
                    "{} must be a valid id",
                    field.label
                )));
            };
            let target = self.registry.get(&rel.target_entity)?;
            self.permissions
                .check(&self.auth, &target.name, Action::Read)?;
            let record = self
                .repo
                .get_tx(self.tx, &target, &self.auth, id, false)
                .await
                .map_err(|e| match e {
                    QefroError::NotFound { .. } => QefroError::not_found(format!(
                        "{} must reference an existing {}",
                        field.label, target.name
                    )),
                    other => other,
                })?;
            enforce_row_policy(&self.auth, &target, &record)?;
        }
        if let (Some(ty), Some(raw_id)) = (
            data.get(RELATED_TYPE_FIELD).and_then(|v| v.as_str()),
            data.get(RELATED_ID_FIELD),
        ) {
            if !ty.is_empty() {
                if let Some(id) = raw_id.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                    let target = self.registry.get(ty)?;
                    self.permissions
                        .check(&self.auth, &target.name, Action::Read)?;
                    let record = self
                        .repo
                        .get_tx(self.tx, &target, &self.auth, id, false)
                        .await?;
                    enforce_row_policy(&self.auth, &target, &record)?;
                }
            }
        }
        Ok(())
    }

    async fn record_side_effects(
        &mut self,
        def: &qefro_core::EntityDef,
        before: Option<&Value>,
        after: &Value,
        action: &str,
    ) -> QefroResult<()> {
        let id = record_id(after)?;
        if def.audit {
            self.audit
                .record_tx(
                    self.tx,
                    &self.auth,
                    &def.name,
                    Some(id),
                    action,
                    before,
                    Some(after),
                )
                .await?;
        }
        if def.activity {
            let atype = match action {
                "create" => crate::activity::TYPE_CREATED,
                "delete" => crate::activity::TYPE_DELETED,
                _ => crate::activity::TYPE_UPDATED,
            };
            let extra = json!({
                "operation_id": self.operation_id,
                "request_id": self.auth.request_id,
                "operation": self.def.name,
            });
            let (message, metadata) = crate::activity::mutation_activity(
                &def.label,
                atype,
                before,
                Some(after),
                Some(extra),
            );
            self.activity
                .record_tx(self.tx, &self.auth, &def.name, id, atype, &message, metadata)
                .await?;
        }
        let specific = match action {
            "create" => "created",
            "delete" => "deleted",
            _ => "updated",
        };
        let framework = match action {
            "create" => "entity.created",
            "delete" => "entity.deleted",
            _ => "entity.updated",
        };
        for mut event in mutation_events_for(
            &def.name,
            id,
            &self.auth,
            after.clone(),
            specific,
            framework,
        ) {
            correlate_event(&mut event, self.operation_id, self.auth.request_id);
            self.pending_events.push(event);
        }
        Ok(())
    }
}

pub fn crud_operation_defs(entity: &qefro_core::EntityDef) -> Vec<OperationDef> {
    vec![
        OperationDef::crud("create", &entity.name),
        OperationDef::crud("get", &entity.name),
        OperationDef::crud("find", &entity.name),
        OperationDef::crud("update", &entity.name),
        OperationDef::crud("delete", &entity.name),
    ]
}

pub fn operation_allowed(
    permissions: &PermissionRegistry,
    ctx: &OpContext,
    def: &OperationDef,
) -> bool {
    permissions.check(ctx, &def.entity, Action::Update).is_ok() && def.role_allowed(ctx)
}

pub fn available_for_record(
    operations: &OperationRegistry,
    permissions: &PermissionRegistry,
    workflows: &WorkflowRegistry,
    ctx: &OpContext,
    entity_name: &str,
    record: &Value,
) -> Vec<OperationDef> {
    let mut out = Vec::new();
    for binding in operations.for_entity(entity_name) {
        if !operation_allowed(permissions, ctx, &binding.def) {
            continue;
        }
        if let Some(wf) = workflows.for_entity(entity_name) {
            let current = record
                .get(&wf.field)
                .and_then(|v| v.as_str())
                .unwrap_or(&wf.initial);
            if let Some(tname) = &binding.def.workflow_transition {
                if workflows.apply(entity_name, current, tname, ctx).is_err() {
                    continue;
                }
            } else if binding.def.name == "cancel" {
                let any = wf
                    .allowed_from(current, ctx)
                    .iter()
                    .any(|t| t.name.starts_with("cancel"));
                if !any {
                    continue;
                }
            }
        }
        out.push(binding.def.clone());
    }
    out
}

fn dirty_patch(entity: &qefro_core::EntityDef, before: &Value, after: &Value) -> Value {
    let mut patch = Map::new();
    for field in entity.stored_fields() {
        let previous = before.get(&field.name);
        let next = after.get(&field.name);
        if previous != next {
            if let Some(v) = next {
                patch.insert(field.name.clone(), v.clone());
            }
        }
    }
    Value::Object(patch)
}

fn with_user(mut event: DomainEvent, user_id: Uuid) -> DomainEvent {
    event.user_id = Some(user_id);
    event
}

/// Runs a business operation in a single SQLx transaction. Events are returned
/// so the caller publishes them only after COMMIT.
pub async fn execute_operation(
    repo: &EntityRepository,
    registry: &EntityRegistry,
    permissions: &PermissionRegistry,
    workflows: &WorkflowRegistry,
    hooks: &HookRegistry,
    operations: &OperationRegistry,
    jobs: &JobQueue,
    audit: &crate::audit::AuditLogger,
    activity: &crate::activity::ActivityStore,
    ctx: &OpContext,
    entity_name: &str,
    id: Uuid,
    name: &str,
    input: Value,
) -> QefroResult<(Value, Vec<DomainEvent>)> {
    execute_operation_with(
        repo,
        registry,
        permissions,
        workflows,
        hooks,
        operations,
        jobs,
        audit,
        activity,
        ctx,
        entity_name,
        id,
        name,
        input,
        ExecuteOpts::default(),
    )
    .await
}

pub async fn execute_operation_with(
    repo: &EntityRepository,
    registry: &EntityRegistry,
    permissions: &PermissionRegistry,
    workflows: &WorkflowRegistry,
    hooks: &HookRegistry,
    operations: &OperationRegistry,
    jobs: &JobQueue,
    audit: &crate::audit::AuditLogger,
    activity: &crate::activity::ActivityStore,
    ctx: &OpContext,
    entity_name: &str,
    id: Uuid,
    name: &str,
    input: Value,
    opts: ExecuteOpts,
) -> QefroResult<(Value, Vec<DomainEvent>)> {
    let started = Instant::now();
    let binding = operations.get(entity_name, name)?;
    let entity = registry.get(entity_name)?;
    reject_client_tenant(&input)?;
    if binding.def.idempotent && opts.idempotency_key.as_deref().unwrap_or("").is_empty() {
        return Err(QefroError::bad_request(
            "Idempotency-Key is required for this operation",
        ));
    }
    authorize_operation(permissions, ctx, &binding.def)?;

    let mut tx = repo
        .pool()
        .begin()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

    let async_enqueue = binding.def.is_async() && !opts.force_sync;
    let mut operation_id = opts.operation_id.unwrap_or_else(Uuid::new_v4);
    let mut reused_run = opts.force_sync && opts.operation_id.is_some();

    if let Some(key) = opts.idempotency_key.as_deref() {
        if let Some(existing) = OperationRunStore::find_idempotent(&mut tx, ctx, key).await? {
            match existing.status.as_str() {
                s if s == STATUS_COMPLETED => {
                    let result = existing.result.clone().unwrap_or_else(|| json!({}));
                    let _ = tx.commit().await;
                    return Ok((result, Vec::new()));
                }
                s if s == STATUS_FAILED && !opts.force_sync => {
                    let _ = tx.commit().await;
                    return Err(QefroError::business(
                        "operation_failed",
                        existing
                            .error
                            .unwrap_or_else(|| "previous attempt failed".into()),
                    ));
                }
                s if (s == STATUS_QUEUED || s == STATUS_RUNNING) && opts.force_sync => {
                    operation_id = existing.id;
                    reused_run = true;
                }
                _ if !opts.force_sync => {
                    let mut queued = json!({});
                    attach_operation_envelope(&mut queued, &existing, &binding.def, None, None);
                    let _ = tx.commit().await;
                    return Ok((queued, Vec::new()));
                }
                _ => {
                    operation_id = existing.id;
                    reused_run = true;
                }
            }
        }
    }

    if reused_run {
        OperationRunStore::mark_running_tx(&mut tx, ctx, operation_id).await?;
    } else if let Err(err) = OperationRunStore::insert_tx(
        &mut tx,
        ctx,
        operation_id,
        entity_name,
        id,
        &binding.def.name,
        if async_enqueue {
            STATUS_QUEUED
        } else {
            STATUS_RUNNING
        },
        opts.idempotency_key.as_deref(),
    )
    .await
    {
        let _ = tx.rollback().await;
        if let Some(key) = opts.idempotency_key.as_deref() {
            let mut retry = repo
                .pool()
                .begin()
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            if let Some(existing) = OperationRunStore::find_idempotent(&mut retry, ctx, key).await? {
                let result = existing.result.clone().unwrap_or_else(|| json!({}));
                let _ = retry.commit().await;
                return Ok((result, Vec::new()));
            }
            let _ = retry.rollback().await;
        }
        return Err(err);
    }

    if async_enqueue {
        let payload = json!({
            "entity": entity_name,
            "entity_id": id,
            "operation": binding.def.name,
            "input": input,
            "operation_id": operation_id,
            "request_id": ctx.request_id,
            "user_id": ctx.user_id,
            "roles": ctx.roles,
            "actor_name": ctx.actor_name,
            "idempotency_key": opts.idempotency_key,
        });
        if let Err(err) = jobs
            .enqueue_tx(&mut tx, ctx, OPERATION_EXECUTE_JOB, payload)
            .await
        {
            let _ = tx.rollback().await;
            return Err(err);
        }
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let mut queued = json!({});
        attach_operation_envelope(
            &mut queued,
            &OperationRun {
                id: operation_id,
                tenant_id: ctx.tenant_id,
                user_id: Some(ctx.user_id),
                entity: entity_name.into(),
                entity_id: id,
                operation: binding.def.name.clone(),
                status: STATUS_QUEUED.into(),
                request_id: Some(ctx.request_id),
                idempotency_key: opts.idempotency_key.clone(),
                progress: 0,
                result: None,
                error: None,
                started_at: None,
                completed_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            &binding.def,
            None,
            None,
        );
        return Ok((queued, Vec::new()));
    }

    let call_stack = vec![format!("{entity_name}.{}", binding.def.name)];
    let outcome = execute_in_transaction(
        &mut tx,
        repo,
        registry,
        permissions,
        workflows,
        hooks,
        operations,
        &binding,
        &entity,
        ctx,
        id,
        input,
        audit,
        activity,
        jobs,
        operation_id,
        call_stack,
        true,
    )
    .await;

    match outcome {
        Ok((mut record, events, _jobs, envelope)) => {
            attach_operation_envelope(
                &mut record,
                &OperationRun {
                    id: operation_id,
                    tenant_id: ctx.tenant_id,
                    user_id: Some(ctx.user_id),
                    entity: entity_name.into(),
                    entity_id: id,
                    operation: binding.def.name.clone(),
                    status: STATUS_COMPLETED.into(),
                    request_id: Some(ctx.request_id),
                    idempotency_key: opts.idempotency_key.clone(),
                    progress: 100,
                    result: None,
                    error: None,
                    started_at: Some(chrono::Utc::now()),
                    completed_at: Some(chrono::Utc::now()),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                &binding.def,
                envelope.message.clone(),
                envelope.navigate.clone(),
            );
            if let Err(err) = OperationRunStore::complete_tx(&mut tx, ctx, operation_id, &record).await
            {
                let _ = tx.rollback().await;
                return Err(err);
            }
            tx.commit()
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            tracing::info!(
                request_id = %ctx.request_id,
                operation_id = %operation_id,
                operation = %format!("{entity_name}.{}", binding.def.name),
                tenant_id = %ctx.tenant_id,
                user_id = %ctx.user_id,
                entity = entity_name,
                entity_id = %id,
                duration_ms = started.elapsed().as_millis() as u64,
                status = "success",
                "business operation"
            );
            MeteringEvent::new(
                ctx.tenant_id,
                "workflow.executed",
                entity_name,
                ctx.request_id,
            )
            .with_resource_id(id.to_string())
            .with_user(ctx.user_id)
            .emit();
            Ok((record, events))
        }
        Err(err) => {
            let _ = tx.rollback().await;
            tracing::info!(
                operation = %format!("{entity_name}.{}", binding.def.name),
                tenant_id = %ctx.tenant_id,
                user_id = %ctx.user_id,
                entity = entity_name,
                entity_id = %id,
                duration_ms = started.elapsed().as_millis() as u64,
                status = "error",
                error = err.error_code(),
                "business operation"
            );
            if matches!(
                err,
                QefroError::Database { .. } | QefroError::Internal { .. }
            ) {
                tracing::error!(
                    operation = %format!("{entity_name}.{}", binding.def.name),
                    tenant_id = %ctx.tenant_id,
                    error = %err,
                    "internal operation failure"
                );
            }
            Err(err)
        }
    }
}

struct HandlerEnvelope {
    message: Option<String>,
    navigate: Option<Value>,
}

async fn execute_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    repo: &EntityRepository,
    registry: &EntityRegistry,
    permissions: &PermissionRegistry,
    workflows: &WorkflowRegistry,
    hooks: &HookRegistry,
    operations: &OperationRegistry,
    binding: &OperationBinding,
    entity: &Arc<qefro_core::EntityDef>,
    ctx: &OpContext,
    id: Uuid,
    input: Value,
    audit: &crate::audit::AuditLogger,
    activity: &crate::activity::ActivityStore,
    jobs: &JobQueue,
    operation_id: Uuid,
    call_stack: Vec<String>,
    enqueue_outbox: bool,
) -> QefroResult<(Value, Vec<DomainEvent>, Vec<(String, Value)>, HandlerEnvelope)> {
    let current = repo.get_tx(tx, entity, ctx, id, true).await?;
    enforce_row_policy(ctx, entity, &current)?;

    if let Some(tname) = &binding.def.workflow_transition {
        let wf = workflows
            .for_entity(&entity.name)
            .ok_or_else(|| QefroError::not_found(format!("no workflow for {}", entity.name)))?;
        let from = current
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial);
        let _ = workflows.apply(&entity.name, from, tname, ctx)?;
    }

    hooks
        .before_operation(ctx, &entity.name, &binding.def.name, &current, &input)
        .await?;

    let (mut record, mut events, job_list, envelope) = {
        let mut op_ctx = OperationCtx {
            auth: ctx.clone(),
            def: binding.def.clone(),
            entity: entity.clone(),
            record: current.clone(),
            input,
            pending_events: Vec::new(),
            pending_jobs: Vec::new(),
            operation_id,
            result_message: None,
            result_navigate: None,
            call_stack,
            registry,
            permissions,
            workflows,
            hooks,
            operations,
            repo,
            audit,
            activity,
            jobs,
            tx,
        };
        binding.handler.handle(&mut op_ctx).await?;
        (
            op_ctx.record,
            op_ctx.pending_events,
            op_ctx.pending_jobs,
            HandlerEnvelope {
                message: op_ctx.result_message,
                navigate: op_ctx.result_navigate,
            },
        )
    };

    if let Some(tname) = &binding.def.workflow_transition {
        let wf = workflows
            .for_entity(&entity.name)
            .ok_or_else(|| QefroError::not_found(format!("no workflow for {}", entity.name)))?;
        let original = current
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial);
        let now = record
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial);
        if now == original {
            let to = workflows.apply(&entity.name, original, tname, ctx)?;
            if let Some(obj) = record.as_object_mut() {
                obj.insert(wf.field.clone(), json!(to));
            }
        }
    }

    let patch = dirty_patch(entity, &current, &record);
    if patch.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        record = repo.update_tx(tx, entity, ctx, id, patch).await?;
    }

    if let Some(naming) = &entity.naming {
        if naming.assign_on == "submit" && binding.def.name != "cancel" {
            let empty = record
                .get(&naming.field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty();
            let initial = workflows
                .for_entity(&entity.name)
                .map(|wf| wf.initial)
                .unwrap_or_else(|| "Draft".into());
            let status = record
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or(initial.as_str());
            if empty && status != initial {
                let number = crate::numbering::allocate(
                    tx,
                    ctx.tenant_id,
                    &entity.name,
                    naming,
                    chrono::Utc::now(),
                )
                .await?;
                let patch = json!({ naming.field.clone(): number });
                record = repo.update_tx(tx, entity, ctx, id, patch).await?;
            }
        }
    }

    if binding.def.audit || entity.audit {
        audit
            .record_tx(
                tx,
                ctx,
                &entity.name,
                Some(id),
                &binding.def.name,
                Some(&current),
                Some(&record),
            )
            .await?;
    }

    if entity.activity {
        let (atype, extra) = if let Some(tname) = &binding.def.workflow_transition {
            let wf = workflows.for_entity(&entity.name);
            let field = wf.as_ref().map(|w| w.field.as_str()).unwrap_or("status");
            let from = current.get(field).and_then(|v| v.as_str()).unwrap_or("");
            let to = record.get(field).and_then(|v| v.as_str()).unwrap_or("");
            (
                crate::activity::TYPE_WORKFLOW,
                Some(json!({
                    "from": from,
                    "to": to,
                    "transition": tname,
                    "operation_id": operation_id,
                    "request_id": ctx.request_id,
                })),
            )
        } else {
            (
                crate::activity::TYPE_UPDATED,
                Some(json!({
                    "operation_id": operation_id,
                    "request_id": ctx.request_id,
                })),
            )
        };
        let (message, metadata) = crate::activity::mutation_activity(
            &entity.label,
            atype,
            Some(&current),
            Some(&record),
            extra,
        );
        activity
            .record_tx(tx, ctx, &entity.name, id, atype, &message, metadata)
            .await?;
        if let Some(tname) = &binding.def.workflow_transition {
            let mut evt = DomainEvent::new(
                "workflow.transitioned",
                entity.name.clone(),
                id,
                ctx.tenant_id,
                json!({
                    "status": record.get("status"),
                    "from": current.get("status"),
                    "to": record.get("status"),
                    "transition": tname,
                }),
            );
            evt.user_id = Some(ctx.user_id);
            correlate_event(&mut evt, operation_id, ctx.request_id);
            if !events.iter().any(|e| e.name == "workflow.transitioned") {
                events.push(evt);
            }
        }
    }

    if let Some(event_name) = &binding.def.event {
        if !events.iter().any(|e| &e.name == event_name) {
            let mut evt = with_user(
                DomainEvent::new(
                    event_name.clone(),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({ "status": record.get("status") }),
                ),
                ctx.user_id,
            );
            correlate_event(&mut evt, operation_id, ctx.request_id);
            events.push(evt);
        }
    }

    for event in &mut events {
        correlate_event(event, operation_id, ctx.request_id);
    }

    let pending_jobs = job_list.clone();
    for (job_name, payload) in &job_list {
        jobs.enqueue_tx(tx, ctx, job_name, payload.clone()).await?;
    }
    if let Some(job_name) = &binding.def.job {
        if !job_list.iter().any(|(n, _)| n == job_name) {
            jobs.enqueue_tx(
                tx,
                ctx,
                job_name,
                json!({
                    "entity": entity.name,
                    "entity_id": id,
                    "operation_id": operation_id,
                    "request_id": ctx.request_id,
                }),
            )
            .await?;
        }
    }

    hooks
        .after_operation(ctx, &entity.name, &binding.def.name, &record)
        .await?;

    if enqueue_outbox {
        crate::outbox::Outbox::enqueue_many_tx(tx, &events).await?;
    }

    Ok((record, events, pending_jobs, envelope))
}

fn authorize_operation(
    permissions: &PermissionRegistry,
    ctx: &OpContext,
    def: &OperationDef,
) -> QefroResult<()> {
    if ctx.is_worker() {
        if !def.worker_safe {
            return Err(QefroError::forbidden(format!(
                "operation '{}.{}' is not worker-safe",
                def.entity, def.name
            )));
        }
        return Ok(());
    }
    permissions.check(ctx, &def.entity, Action::Update)?;
    if !def.role_allowed(ctx) {
        return Err(QefroError::forbidden(format!(
            "role(s) {:?} cannot {} {}",
            ctx.roles, def.name, def.entity
        )));
    }
    Ok(())
}

fn apply_transition_to(
    workflows: &WorkflowRegistry,
    auth: &OpContext,
    entity: &str,
    record: &mut Value,
    name: &str,
) -> QefroResult<String> {
    let wf = workflows
        .for_entity(entity)
        .ok_or_else(|| QefroError::not_found(format!("no workflow for {entity}")))?;
    let from = record
        .get(&wf.field)
        .and_then(|v| v.as_str())
        .unwrap_or(&wf.initial)
        .to_string();
    let to = workflows.apply(entity, &from, name, auth)?;
    if let Some(t) = wf.find_transition(&from, name) {
        t.guard_allows(record)?;
    }
    if let Some(obj) = record.as_object_mut() {
        obj.insert(wf.field.clone(), json!(to.clone()));
    }
    Ok(to)
}

fn reject_client_tenant(data: &Value) -> QefroResult<()> {
    if data.get("tenant_id").is_some() {
        return Err(QefroError::bad_request(
            "tenant_id cannot be set by the client",
        ));
    }
    Ok(())
}

fn enforce_row_policy(
    ctx: &OpContext,
    entity: &qefro_core::EntityDef,
    record: &Value,
) -> QefroResult<()> {
    if ctx.is_admin() {
        return Ok(());
    }
    match entity.row_policy {
        Some(RowPolicy::AssignedTo) => {
            let assigned = record
                .get("assigned_to")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if assigned != ctx.user_id.to_string() {
                return Err(QefroError::not_found(format!("{} not found", entity.name)));
            }
        }
        Some(RowPolicy::CreatedBy) => {
            let created = record
                .get("created_by")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if created != ctx.user_id.to_string() {
                return Err(QefroError::not_found(format!("{} not found", entity.name)));
            }
        }
        Some(RowPolicy::AssignedToOrCreatedBy) => {
            let me = ctx.user_id.to_string();
            let assigned = record
                .get("assigned_to")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let created = record
                .get("created_by")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if assigned != me && created != me {
                return Err(QefroError::not_found(format!("{} not found", entity.name)));
            }
        }
        None => {}
    }
    Ok(())
}

fn apply_op_defaults(entity: &qefro_core::EntityDef, data: &mut Value, ctx: &OpContext) {
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
                "current_date" => json!(chrono::Utc::now().date_naive().to_string()),
                "current_datetime" => json!(chrono::Utc::now().to_rfc3339()),
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

fn correlate_event(event: &mut DomainEvent, operation_id: Uuid, request_id: Uuid) {
    match event.payload {
        Value::Object(ref mut obj) => {
            obj.entry("operation_id")
                .or_insert_with(|| json!(operation_id));
            obj.entry("request_id")
                .or_insert_with(|| json!(request_id));
        }
        _ => {
            event.payload = json!({
                "value": event.payload,
                "operation_id": operation_id,
                "request_id": request_id,
            });
        }
    }
}

fn mutation_events_for(
    entity: &str,
    id: Uuid,
    ctx: &OpContext,
    mut payload: Value,
    specific: &str,
    framework: &str,
) -> Vec<DomainEvent> {
    qefro_core::strip_secrets(None, &mut payload);
    let specific_name = if specific.contains('.') {
        specific.to_string()
    } else {
        format!("{}.{}", snake_case(entity), specific)
    };
    vec![
        with_user(
            DomainEvent::new(
                specific_name,
                entity.to_string(),
                id,
                ctx.tenant_id,
                payload.clone(),
            ),
            ctx.user_id,
        ),
        with_user(
            DomainEvent::new(
                framework.to_string(),
                entity.to_string(),
                id,
                ctx.tenant_id,
                payload,
            ),
            ctx.user_id,
        ),
    ]
}

fn attach_operation_envelope(
    record: &mut Value,
    run: &OperationRun,
    def: &OperationDef,
    message: Option<String>,
    navigate: Option<Value>,
) {
    let obj = match record.as_object_mut() {
        Some(o) => o,
        None => {
            *record = json!({});
            record.as_object_mut().unwrap()
        }
    };
    obj.insert(
        "_operation".into(),
        json!({
            "id": run.id,
            "operation": def.name,
            "name": def.name,
            "status": run.status,
            "progress": run.progress,
            "message": message,
            "navigate": navigate,
            "request_id": run.request_id,
        }),
    );
}

/// JobQueue handler for `OperationDef.execution = async`. Reconstructs the
/// original caller so permissions are not elevated to Worker.
pub struct OperationExecuteJob {
    entities: OnceLock<Arc<crate::service::EntityService>>,
}

impl OperationExecuteJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entities: OnceLock::new(),
        })
    }

    pub fn bind(&self, entities: Arc<crate::service::EntityService>) {
        let _ = self.entities.set(entities);
    }
}

#[async_trait]
impl JobHandler for OperationExecuteJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, job_ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(entities) = self.entities.get() else {
            return Err(QefroError::internal("operation execute job is not bound"));
        };
        let entity = payload
            .get("entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("entity is required"))?;
        let id = payload
            .get("entity_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request("entity_id is required"))?;
        let operation = payload
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("operation is required"))?;
        let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));
        let operation_id = payload
            .get("operation_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let user_id = payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or(job_ctx.user_id);
        let roles = payload
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| job_ctx.roles.clone());
        let mut ctx = OpContext::new(job_ctx.tenant_id, user_id, roles);
        ctx.request_id = payload
            .get("request_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or(job_ctx.request_id);
        ctx.actor_name = payload
            .get("actor_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or(job_ctx.actor_name.clone());
        ctx.enabled_apps = job_ctx.enabled_apps.clone();
        entities
            .execute_with(
                &ctx,
                entity,
                id,
                operation,
                input,
                ExecuteOpts {
                    idempotency_key: payload
                        .get("idempotency_key")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    force_sync: true,
                    operation_id,
                },
            )
            .await?;
        Ok(())
    }
}
