use crate::jobs::JobQueue;
use crate::repository::{record_id, EntityRepository};
use async_trait::async_trait;
use qefro_core::{
    EntityRegistry, HookRegistry, MeteringEvent, OpContext, OperationDef, QefroError, QefroResult,
};
use qefro_events::DomainEvent;
use qefro_permissions::{Action, PermissionRegistry};
use qefro_search::{Filter, Query};
use qefro_workflow::WorkflowRegistry;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

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

/// Handler context. Auth, tenant, and RBAC are already enforced.
/// Mutate through the provided methods so work stays on the same transaction.
pub struct OperationCtx<'a, 'conn: 'a> {
    pub auth: OpContext,
    pub def: OperationDef,
    pub entity: Arc<qefro_core::EntityDef>,
    pub record: Value,
    pub input: Value,
    pub pending_events: Vec<DomainEvent>,
    pub pending_jobs: Vec<(String, Value)>,
    registry: &'a EntityRegistry,
    workflows: &'a WorkflowRegistry,
    repo: &'a EntityRepository,
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
            self.pending_events.push(event);
        }
    }

    pub fn enqueue_job(&mut self, name: impl Into<String>, payload: Value) {
        self.pending_jobs.push((name.into(), payload));
    }

    pub fn apply_transition(&mut self, name: &str) -> QefroResult<String> {
        let wf = self
            .workflows
            .for_entity(&self.entity.name)
            .ok_or_else(|| {
                QefroError::not_found(format!("no workflow for {}", self.entity.name))
            })?;
        let from = self
            .record
            .get(&wf.field)
            .and_then(|v| v.as_str())
            .unwrap_or(&wf.initial)
            .to_string();
        let to = self
            .workflows
            .apply(&self.entity.name, &from, name, &self.auth)?;
        self.set_field(&wf.field, json!(to));
        Ok(to)
    }

    pub async fn get(&mut self, entity: &str, id: Uuid) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.repo.get_tx(self.tx, &def, &self.auth, id, true).await
    }

    pub async fn update(&mut self, entity: &str, id: Uuid, patch: Value) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.repo
            .update_tx(self.tx, &def, &self.auth, id, patch)
            .await
    }

    pub async fn create(&mut self, entity: &str, data: Value) -> QefroResult<Value> {
        let def = self.registry.get(entity)?;
        self.repo.insert_tx(self.tx, &def, &self.auth, data).await
    }

    pub async fn list(
        &mut self,
        entity: &str,
        field: &str,
        value: Value,
    ) -> QefroResult<Vec<Value>> {
        let def = self.registry.get(entity)?;
        let mut query = Query::default();
        query.page_size = 100;
        query.filters.push(Filter::Eq {
            field: field.into(),
            value,
        });
        let page = self.repo.list_tx(self.tx, &def, &self.auth, &query).await?;
        Ok(page.items)
    }

    pub fn entity_def(&self, name: &str) -> QefroResult<std::sync::Arc<qefro_core::EntityDef>> {
        self.registry.get(name)
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
    let started = Instant::now();
    let binding = operations.get(entity_name, name)?;
    let entity = registry.get(entity_name)?;
    if ctx.is_worker() {
        if !binding.def.worker_safe {
            return Err(QefroError::forbidden(format!(
                "operation '{entity_name}.{}' is not worker-safe",
                binding.def.name
            )));
        }
    } else {
        permissions.check(ctx, &entity.name, Action::Update)?;
        if !binding.def.role_allowed(ctx) {
            return Err(QefroError::forbidden(format!(
                "role(s) {:?} cannot {} {}",
                ctx.roles, binding.def.name, entity.name
            )));
        }
    }

    let mut tx = repo
        .pool()
        .begin()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;

    let outcome = execute_in_transaction(
        &mut tx, repo, registry, workflows, hooks, &binding, &entity, ctx, id, input, audit,
        activity, jobs,
    )
    .await;

    match outcome {
        Ok((record, events)) => {
            tx.commit()
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            tracing::info!(
                request_id = %ctx.request_id,
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

async fn execute_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    repo: &EntityRepository,
    registry: &EntityRegistry,
    workflows: &WorkflowRegistry,
    hooks: &HookRegistry,
    binding: &OperationBinding,
    entity: &Arc<qefro_core::EntityDef>,
    ctx: &OpContext,
    id: Uuid,
    input: Value,
    audit: &crate::audit::AuditLogger,
    activity: &crate::activity::ActivityStore,
    jobs: &JobQueue,
) -> QefroResult<(Value, Vec<DomainEvent>)> {
    let current = repo.get_tx(tx, entity, ctx, id, true).await?;

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

    let (mut record, mut events, job_list) = {
        let mut op_ctx = OperationCtx {
            auth: ctx.clone(),
            def: binding.def.clone(),
            entity: entity.clone(),
            record: current.clone(),
            input,
            pending_events: Vec::new(),
            pending_jobs: Vec::new(),
            registry,
            workflows,
            repo,
            tx,
        };
        binding.handler.handle(&mut op_ctx).await?;
        (op_ctx.record, op_ctx.pending_events, op_ctx.pending_jobs)
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
                Some(json!({ "from": from, "to": to, "transition": tname })),
            )
        } else {
            (crate::activity::TYPE_UPDATED, None)
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
            if !events.iter().any(|e| e.name == "workflow.transitioned") {
                events.push(evt);
            }
        }
    }

    if let Some(event_name) = &binding.def.event {
        if !events.iter().any(|e| &e.name == event_name) {
            events.push(with_user(
                DomainEvent::new(
                    event_name.clone(),
                    entity.name.clone(),
                    id,
                    ctx.tenant_id,
                    json!({ "status": record.get("status") }),
                ),
                ctx.user_id,
            ));
        }
    }

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
                }),
            )
            .await?;
        }
    }

    hooks
        .after_operation(ctx, &entity.name, &binding.def.name, &record)
        .await?;

    crate::outbox::Outbox::enqueue_many_tx(tx, &events).await?;

    Ok((record, events))
}
