//! Automation runtime: match DomainEvents, enqueue JobQueue work, execute
//! actions through EntityService / NotificationDef / WebhookDef.

use crate::communication::{enqueue_communication, CommunicationStore};
use crate::jobs::{JobHandler, JobQueue};
use crate::notifications::NotificationStore;
use crate::service::EntityService;
use async_trait::async_trait;
use qefro_core::{
    next_run_after, parse_cron, parse_wait_duration, schedule_slot_key, strip_secrets,
    AutomationAction, AutomationDef, AutomationStep, CommunicationDef, NotificationDef, OpContext,
    QefroError, QefroResult, WaitSpec, WebhookDef, ROLE_WORKER,
};
use qefro_events::{DomainEvent, EventHandler};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use uuid::Uuid;

const JOB_RUN: &str = "automation.run";
const JOB_SCHEDULE: &str = "automation.schedule";

pub struct AutomationEngine {
    pool: PgPool,
    jobs: Arc<JobQueue>,
    defs: Vec<AutomationDef>,
    notifications: Vec<NotificationDef>,
    webhooks: Vec<WebhookDef>,
    communications: Vec<CommunicationDef>,
    comm_store: CommunicationStore,
    store: NotificationStore,
    entities: OnceLock<Arc<EntityService>>,
    overlays: RwLock<HashMap<String, AutomationDef>>,
}

impl AutomationEngine {
    pub fn new(
        pool: PgPool,
        jobs: Arc<JobQueue>,
        defs: Vec<AutomationDef>,
        notifications: Vec<NotificationDef>,
        webhooks: Vec<WebhookDef>,
        communications: Vec<CommunicationDef>,
    ) -> Self {
        Self {
            store: NotificationStore::new(pool.clone()),
            comm_store: CommunicationStore::new(pool.clone()),
            pool,
            jobs,
            defs,
            notifications,
            webhooks,
            communications,
            entities: OnceLock::new(),
            overlays: RwLock::new(HashMap::new()),
        }
    }

    pub fn bind(&self, entities: Arc<EntityService>) {
        let _ = self.entities.set(entities);
    }

    pub fn overlay_put(&self, def: AutomationDef) {
        if let Ok(mut g) = self.overlays.write() {
            g.insert(def.name.clone(), def);
        }
    }

    pub fn overlay_disable(&self, name: &str, enabled: bool) -> Option<AutomationDef> {
        let mut def = self.def_by_id(name)?;
        def.enabled = enabled;
        self.overlay_put(def.clone());
        Some(def)
    }

    pub fn defs(&self) -> Vec<AutomationDef> {
        let overlay = self.overlays.read().ok();
        let mut map: HashMap<String, AutomationDef> = self
            .defs
            .iter()
            .cloned()
            .map(|d| (d.name.clone(), d))
            .collect();
        if let Some(overlay) = overlay.as_ref() {
            for (k, v) in overlay.iter() {
                map.insert(k.clone(), v.clone());
            }
        }
        let mut out: Vec<_> = map.into_values().collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn def_by_id(&self, automation_id: &str) -> Option<AutomationDef> {
        self.defs()
            .into_iter()
            .find(|d| d.id_key() == automation_id || d.name == automation_id)
    }

    fn entities(&self) -> QefroResult<&EntityService> {
        self.entities
            .get()
            .map(|e| e.as_ref())
            .ok_or_else(|| QefroError::internal("automation engine is not bound"))
    }

    pub async fn enqueue_for_event(&self, event: &DomainEvent) -> QefroResult<()> {
        let view = event_view(event);
        let depth = view
            .get("_automation_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        for def in self.defs() {
            if !def.matches_event(&event.name) {
                continue;
            }
            if depth >= def.depth_limit() {
                tracing::warn!(
                    automation = %def.id_key(),
                    depth,
                    "automation depth limit reached; skipping to prevent a loop"
                );
                continue;
            }
            if let Some(cond) = &def.conditions {
                if !cond.matches(&view) {
                    continue;
                }
            }
            let execution_id = Uuid::new_v4();
            let key = format!("{}:{}:{}", event.tenant_id, def.id_key(), event.id);
            let mut ctx = OpContext::worker(event.tenant_id, event.user_id.unwrap_or(Uuid::nil()));
            ctx.source = "automation".into();
            ctx.request_id = execution_id;
            let payload = json!({
                "idempotency_key": key,
                "automation_id": def.id_key(),
                "execution_id": execution_id,
                "event_id": event.id,
                "event": event.to_public_json(),
                "max_attempts": def.attempt_limit(),
            });
            // After COMMIT (outbox). In-app notify/activity run here like NotificationDef.
            // Failures enqueue automation.run onto JobQueue for retry; success is a no-op for the job.
            if let Err(err) = self.run_payload(&ctx, &payload).await {
                tracing::error!(error = %err, automation = %def.name, "automation run failed; enqueueing retry");
                self.jobs.enqueue(&ctx, JOB_RUN, payload).await?;
            }
        }
        Ok(())
    }

    pub async fn enqueue_scheduled(&self) -> QefroResult<usize> {
        let tenants: Vec<(Uuid, Option<Value>)> = sqlx::query_as(
            "SELECT t.id, s.business_config FROM tenants t LEFT JOIN tenant_settings s ON s.tenant_id = t.id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let now = chrono::Utc::now();
        let mut n = 0;
        for def in self.defs() {
            if !def.enabled || !def.trigger.is_scheduled() {
                continue;
            }
            let Some(cron) = def.trigger.schedule.as_deref() else {
                continue;
            };
            let expr = match parse_cron(cron) {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(automation = %def.name, error = %err, "invalid schedule");
                    continue;
                }
            };
            for (tenant_id, business) in &tenants {
                let tz = def
                    .timezone
                    .clone()
                    .or_else(|| {
                        business
                            .as_ref()
                            .and_then(|v| v.get("timezone"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "UTC".into());
                let next = next_run_after(&expr, now - chrono::Duration::minutes(1), &tz);
                if next > now + chrono::Duration::seconds(30) {
                    continue;
                }
                let slot = schedule_slot_key(next);
                let key = format!("{}:{}:{}", tenant_id, def.id_key(), slot);
                let execution_id = Uuid::new_v4();
                let ctx = OpContext::worker(*tenant_id, Uuid::nil());
                self.jobs
                    .enqueue(
                        &ctx,
                        JOB_SCHEDULE,
                        json!({
                            "idempotency_key": key,
                            "automation_id": def.id_key(),
                            "execution_id": execution_id,
                            "event_id": uuid_from_key(&key),
                            "run_at": next.to_rfc3339(),
                            "kind": "scheduled",
                            "timezone": tz,
                            "schedule": cron,
                            "next_run": next.to_rfc3339(),
                            "max_attempts": def.attempt_limit(),
                        }),
                    )
                    .await?;
                n += 1;
            }
        }
        Ok(n)
    }

    async fn run_payload(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let automation_id = payload
            .get("automation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("automation_id required"))?;
        let live = self.def_by_id(automation_id).ok_or_else(|| {
            QefroError::not_found(format!("automation '{automation_id}' not found"))
        })?;
        let execution_id = payload
            .get("execution_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);
        let event_id = payload
            .get("event_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| {
                payload
                    .get("idempotency_key")
                    .and_then(|v| v.as_str())
                    .map(uuid_from_key)
                    .unwrap_or_else(Uuid::new_v4)
            });
        let prior = self
            .peek_status(ctx.tenant_id, &live.id_key(), event_id)
            .await?;
        let resuming = matches!(
            prior.as_deref(),
            Some("waiting" | "failed" | "pending" | "retrying")
        );
        if !live.enabled && !resuming {
            return Ok(());
        }
        let def = live;
        if !self
            .claim_execution(ctx.tenant_id, &def.id_key(), event_id, execution_id)
            .await?
        {
            tracing::info!(
                tenant_id = %ctx.tenant_id,
                automation = %def.id_key(),
                event_id = %event_id,
                execution_id = %execution_id,
                "automation already executed"
            );
            return Ok(());
        }
        let mut event = payload
            .get("event")
            .cloned()
            .map(event_from_json)
            .unwrap_or_else(|| scheduled_event(ctx, &def, event_id));
        event.tenant_id = ctx.tenant_id;
        event.user_id = if ctx.user_id.is_nil() {
            None
        } else {
            Some(ctx.user_id)
        };
        self.remember_execution(ctx.tenant_id, &def, event_id, &event)
            .await?;
        let stored = self
            .load_execution(ctx.tenant_id, &def.id_key(), event_id)
            .await?;
        let def = stored
            .as_ref()
            .and_then(|s| s.snapshot.clone())
            .unwrap_or(def);
        let start_cursor = stored
            .as_ref()
            .map(|s| s.cursor.clone())
            .unwrap_or_default();
        let mut log = stored
            .as_ref()
            .map(|s| s.log.clone())
            .unwrap_or_else(|| json!([]));
        let mut run_ctx = self.action_context(ctx, &def, &event, execution_id).await;
        run_ctx.automation_depth = event
            .payload
            .get("_automation_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let mut cursor = start_cursor.clone();
        let result = self
            .execute_steps(
                &mut run_ctx,
                &def,
                &mut event,
                start_cursor.clone(),
                &mut log,
                &mut cursor,
            )
            .await;
        match result {
            Ok(StepResult::Waiting { run_at, cursor }) => {
                self.persist_execution(
                    ctx.tenant_id,
                    &def.id_key(),
                    event_id,
                    "waiting",
                    None,
                    &cursor,
                    &log,
                )
                .await?;
                let mut resume = payload.clone();
                if let Some(obj) = resume.as_object_mut() {
                    obj.insert("run_at".into(), json!(run_at.to_rfc3339()));
                    obj.insert("max_attempts".into(), json!(def.attempt_limit()));
                    obj.insert(
                        "idempotency_key".into(),
                        json!(format!(
                            "{}:{}:{}:step:{}",
                            ctx.tenant_id,
                            def.id_key(),
                            event_id,
                            cursor
                                .iter()
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join(".")
                        )),
                    );
                }
                self.jobs.enqueue(ctx, JOB_RUN, resume).await?;
                Ok(())
            }
            Ok(StepResult::Done) => {
                self.persist_execution(
                    ctx.tenant_id,
                    &def.id_key(),
                    event_id,
                    "completed",
                    None,
                    &[],
                    &log,
                )
                .await?;
                Ok(())
            }
            Err(e) => {
                let msg = e.public_message();
                self.persist_execution(
                    ctx.tenant_id,
                    &def.id_key(),
                    event_id,
                    "retrying",
                    Some(&msg),
                    &cursor,
                    &log,
                )
                .await?;
                Err(QefroError::business("automation_failed", msg))
            }
        }
    }

    async fn peek_status(
        &self,
        tenant_id: Uuid,
        automation_id: &str,
        event_id: Uuid,
    ) -> QefroResult<Option<String>> {
        sqlx::query_scalar(
            "SELECT status FROM qefro_automation_executions WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3",
        )
        .bind(tenant_id)
        .bind(automation_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    async fn claim_execution(
        &self,
        tenant_id: Uuid,
        automation_id: &str,
        event_id: Uuid,
        execution_id: Uuid,
    ) -> QefroResult<bool> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO qefro_automation_executions (
                id, tenant_id, automation_id, event_id, execution_id, status, created_at
            ) VALUES ($1,$2,$3,$4,$5,'running', now())
            ON CONFLICT (tenant_id, automation_id, event_id) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(automation_id)
        .bind(event_id)
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        if row.is_some() {
            return Ok(true);
        }
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM qefro_automation_executions WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3",
        )
        .bind(tenant_id)
        .bind(automation_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        match status.as_deref() {
            Some("completed") | Some("succeeded") | Some("cancelled") => Ok(false),
            Some("running") => Ok(false),
            _ => {
                let updated = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    UPDATE qefro_automation_executions
                    SET status = 'running', execution_id = $4, error = NULL, updated_at = now()
                    WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3
                      AND status IN ('waiting', 'failed', 'pending', 'retrying')
                    RETURNING id
                    "#,
                )
                .bind(tenant_id)
                .bind(automation_id)
                .bind(event_id)
                .bind(execution_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
                Ok(updated.is_some())
            }
        }
    }

    async fn action_context(
        &self,
        job_ctx: &OpContext,
        def: &AutomationDef,
        event: &DomainEvent,
        execution_id: Uuid,
    ) -> OpContext {
        let mut roles = qefro_core::sanitize_automation_roles(def.as_roles.clone());
        let mut user_id = event.user_id.unwrap_or(job_ctx.user_id);
        if roles.is_empty() {
            if let Some(uid) = event.user_id {
                if let Ok(loaded) = load_roles(&self.pool, event.tenant_id, uid).await {
                    roles = qefro_core::sanitize_automation_roles(loaded);
                    user_id = uid;
                }
            }
        }
        if roles.is_empty() {
            roles = vec![ROLE_WORKER.into()];
        }
        let mut ctx = OpContext::new(event.tenant_id, user_id, roles);
        ctx.source = "automation".into();
        ctx.request_id = execution_id;
        ctx.timezone = def
            .timezone
            .clone()
            .unwrap_or_else(|| job_ctx.timezone.clone());
        if ctx.timezone.is_empty() {
            ctx.timezone = "UTC".into();
        }
        ctx.enabled_apps = job_ctx.enabled_apps.clone();
        ctx
    }

    async fn remember_execution(
        &self,
        tenant_id: Uuid,
        def: &AutomationDef,
        event_id: Uuid,
        event: &DomainEvent,
    ) -> QefroResult<()> {
        let snapshot = serde_json::to_value(def).unwrap_or(json!({}));
        sqlx::query(
            r#"
            UPDATE qefro_automation_executions
            SET def_snapshot = COALESCE(def_snapshot, $4),
                entity = COALESCE(entity, $5),
                record_id = COALESCE(record_id, $6),
                updated_at = now()
            WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(def.id_key())
        .bind(event_id)
        .bind(snapshot)
        .bind(&event.entity)
        .bind(event.entity_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    async fn load_execution(
        &self,
        tenant_id: Uuid,
        automation_id: &str,
        event_id: Uuid,
    ) -> QefroResult<Option<StoredExecution>> {
        let row = sqlx::query_as::<_, (Value, Value, Option<Value>)>(
            "SELECT cursor, steps_log, def_snapshot FROM qefro_automation_executions WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3",
        )
        .bind(tenant_id)
        .bind(automation_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(row.map(|(cursor, log, snap)| StoredExecution {
            cursor: cursor_from_value(&cursor),
            log,
            snapshot: snap.and_then(|v| serde_json::from_value(v).ok()),
        }))
    }

    async fn persist_execution(
        &self,
        tenant_id: Uuid,
        automation_id: &str,
        event_id: Uuid,
        status: &str,
        error: Option<&str>,
        cursor: &[usize],
        log: &Value,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_automation_executions
            SET status = $2, error = $3, cursor = $4, steps_log = $5, updated_at = now()
            WHERE tenant_id = $1 AND automation_id = $6 AND event_id = $7
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(error)
        .bind(json!(cursor))
        .bind(log)
        .bind(automation_id)
        .bind(event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn list_runs(
        &self,
        ctx: &OpContext,
        automation_id: Option<&str>,
        entity: Option<&str>,
        record_id: Option<Uuid>,
        limit: i64,
    ) -> QefroResult<Vec<Value>> {
        let resolved = automation_id.map(|name| {
            self.def_by_id(name)
                .map(|d| d.id_key())
                .unwrap_or_else(|| name.to_string())
        });
        let rows = sqlx::query_as::<_, (Uuid, String, Uuid, String, Option<String>, Option<String>, Option<Uuid>, Value, chrono::DateTime<chrono::Utc>)>(
            r#"
            SELECT execution_id, automation_id, event_id, status, error, entity, record_id, steps_log, created_at
            FROM qefro_automation_executions
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR automation_id = $2)
              AND ($3::text IS NULL OR entity = $3)
              AND ($4::uuid IS NULL OR record_id = $4)
            ORDER BY created_at DESC
            LIMIT $5
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(resolved.as_deref())
        .bind(entity)
        .bind(record_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(
                |(eid, aid, ev, status, error, entity, record_id, log, at)| {
                    json!({
                        "execution_id": eid,
                        "automation_id": aid,
                        "event_id": ev,
                        "status": status,
                        "error": error,
                        "entity": entity,
                        "record_id": record_id,
                        "steps": log,
                        "created_at": at.to_rfc3339(),
                    })
                },
            )
            .collect())
    }

    pub async fn preview(
        &self,
        ctx: &OpContext,
        name: &str,
        event: &DomainEvent,
    ) -> QefroResult<Value> {
        let def = self
            .def_by_id(name)
            .ok_or_else(|| QefroError::not_found(format!("automation '{name}' not found")))?;
        let steps = self.plan_steps(&def, event);
        let _ = ctx;
        Ok(json!({
            "automation": def.name,
            "dry_run": true,
            "would_execute": steps,
            "side_effects": false,
        }))
    }

    fn plan_steps(&self, def: &AutomationDef, event: &DomainEvent) -> Vec<Value> {
        let view = event_view(event);
        plan_walk(&def.effective_steps(), &view)
    }

    async fn execute_steps(
        &self,
        ctx: &mut OpContext,
        def: &AutomationDef,
        event: &mut DomainEvent,
        start: Vec<usize>,
        log: &mut Value,
        out_cursor: &mut Vec<usize>,
    ) -> QefroResult<StepResult> {
        let steps = def.effective_steps();
        self.walk_steps(ctx, def, event, &steps, &[], start, log, out_cursor)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn walk_steps(
        &self,
        ctx: &mut OpContext,
        def: &AutomationDef,
        event: &mut DomainEvent,
        steps: &[AutomationStep],
        prefix: &[usize],
        start: Vec<usize>,
        log: &mut Value,
        out_cursor: &mut Vec<usize>,
    ) -> QefroResult<StepResult> {
        let mut index = start.first().copied().unwrap_or(0);
        while index < steps.len() {
            let mut here = prefix.to_vec();
            here.push(index);
            *out_cursor = here.clone();
            let rest = if start.len() > 1 && index == start.first().copied().unwrap_or(0) {
                start[1..].to_vec()
            } else {
                Vec::new()
            };
            match &steps[index] {
                AutomationStep::End { .. } => {
                    push_log(log, "end", "ok", "End");
                    return Ok(StepResult::Done);
                }
                AutomationStep::Wait { wait } => {
                    self.refresh_event(ctx, event).await?;
                    let run_at = self.resolve_wait(wait, event, &ctx.timezone)?;
                    if run_at <= chrono::Utc::now() + chrono::Duration::seconds(1) {
                        push_log(log, "wait", "ok", &wait.label());
                        index += 1;
                        continue;
                    }
                    push_log(log, "wait", "waiting", &wait.label());
                    let mut next = prefix.to_vec();
                    next.push(index + 1);
                    return Ok(StepResult::Waiting {
                        run_at,
                        cursor: next,
                    });
                }
                AutomationStep::Branch {
                    condition,
                    then,
                    otherwise,
                } => {
                    let (which, inner_start) = if rest.len() >= 2 {
                        (rest[0], rest[1..].to_vec())
                    } else if rest.len() == 1 {
                        (rest[0], Vec::new())
                    } else {
                        self.refresh_event(ctx, event).await?;
                        let view = event_view(event);
                        let ok = condition.matches(&view);
                        push_log(log, "condition", "ok", if ok { "then" } else { "else" });
                        (if ok { 0 } else { 1 }, Vec::new())
                    };
                    let branch = if which == 0 {
                        then.as_slice()
                    } else {
                        otherwise.as_slice()
                    };
                    let mut nested_prefix = here.clone();
                    nested_prefix.push(which);
                    match Box::pin(self.walk_steps(
                        ctx,
                        def,
                        event,
                        branch,
                        &nested_prefix,
                        inner_start,
                        log,
                        out_cursor,
                    ))
                    .await
                    {
                        Ok(StepResult::Done) => {
                            index += 1;
                        }
                        Ok(StepResult::Waiting { run_at, cursor }) => {
                            return Ok(StepResult::Waiting { run_at, cursor });
                        }
                        Err(e) => return Err(e),
                    }
                }
                AutomationStep::Action(action) => {
                    if let Err(e) = self.execute_action(ctx, def, event, action).await {
                        push_log(log, action.kind(), "failed", &e.public_message());
                        return Err(e);
                    }
                    push_log(log, action.kind(), "ok", action.kind());
                    index += 1;
                }
            }
        }
        Ok(StepResult::Done)
    }

    fn resolve_wait(
        &self,
        wait: &WaitSpec,
        event: &DomainEvent,
        tz: &str,
    ) -> QefroResult<chrono::DateTime<chrono::Utc>> {
        match wait {
            WaitSpec::Duration(raw) => {
                let d = parse_wait_duration(raw).map_err(QefroError::bad_request)?;
                Ok(chrono::Utc::now() + d)
            }
            WaitSpec::UntilField { until_field } => {
                let raw = event
                    .payload
                    .get(until_field)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        QefroError::bad_request(format!("wait until '{until_field}' is missing"))
                    })?;
                if let Some(dt) = qefro_core::canonicalize_datetime(raw, tz) {
                    return Ok(dt);
                }
                if let Some(date) = qefro_core::parse_date(raw) {
                    let naive = date.and_hms_opt(0, 0, 0).unwrap();
                    return Ok(qefro_core::local_to_utc(naive, tz));
                }
                Err(QefroError::bad_request(format!(
                    "wait until '{until_field}' is not a date"
                )))
            }
        }
    }

    async fn refresh_event(&self, ctx: &OpContext, event: &mut DomainEvent) -> QefroResult<()> {
        if event.entity.is_empty() || event.entity_id.is_nil() {
            return Ok(());
        }
        let Ok(entities) = self.entities() else {
            return Ok(());
        };
        if let Ok(record) = entities.get(ctx, &event.entity, event.entity_id).await {
            if let Some(obj) = record.as_object() {
                if let Some(payload) = event.payload.as_object_mut() {
                    for (k, v) in obj {
                        payload.insert(k.clone(), v.clone());
                    }
                } else {
                    event.payload = record;
                }
            }
        }
        Ok(())
    }

    async fn execute_action(
        &self,
        ctx: &mut OpContext,
        def: &AutomationDef,
        event: &DomainEvent,
        action: &AutomationAction,
    ) -> QefroResult<()> {
        match action {
            AutomationAction::Notify { notify } => self.action_notify(ctx, event, notify).await,
            AutomationAction::SendCommunication { send_communication } => {
                self.action_send_communication(ctx, event, send_communication)
                    .await
            }
            AutomationAction::SendWebhook { send_webhook } => {
                self.action_webhook(ctx, event, send_webhook).await
            }
            AutomationAction::CreateActivity { create_activity } => {
                self.action_activity(ctx, event, create_activity).await
            }
            AutomationAction::CreateComment { create_comment } => {
                self.entities()?
                    .add_comment(ctx, &event.entity, event.entity_id, &create_comment.message)
                    .await?;
                Ok(())
            }
            AutomationAction::UpdateEntity { update_entity } => {
                let entity = update_entity
                    .entity
                    .as_deref()
                    .unwrap_or(event.entity.as_str());
                let id = resolve_id(update_entity.record_id.as_deref(), event)?;
                let fields = interpolate_value(&update_entity.fields, event);
                self.entities()?.update(ctx, entity, id, fields).await?;
                Ok(())
            }
            AutomationAction::CreateEntity { create_entity } => {
                let fields = interpolate_value(&create_entity.fields, event);
                self.entities()?
                    .create(ctx, &create_entity.entity, fields)
                    .await?;
                Ok(())
            }
            AutomationAction::Transition { transition } => {
                let entity = transition
                    .entity
                    .as_deref()
                    .unwrap_or(event.entity.as_str());
                let id = resolve_id(transition.record_id.as_deref(), event)?;
                self.entities()?
                    .transition(ctx, entity, id, &transition.name)
                    .await?;
                Ok(())
            }
            AutomationAction::Assign { assign } => {
                let entity = assign.entity.as_deref().unwrap_or(event.entity.as_str());
                let id = resolve_id(assign.record_id.as_deref(), event)?;
                let user = assign
                    .user_id
                    .as_deref()
                    .map(|s| interpolate(s, event))
                    .unwrap_or(Value::Null);
                let mut patch = serde_json::Map::new();
                patch.insert(assign.field.clone(), user);
                self.entities()?
                    .update(ctx, entity, id, Value::Object(patch))
                    .await?;
                Ok(())
            }
            AutomationAction::PrintDocument { print_document } => {
                let entity = print_document
                    .entity
                    .as_deref()
                    .unwrap_or(event.entity.as_str());
                let id = resolve_id(print_document.record_id.as_deref(), event)?;
                let _ = self
                    .entities()?
                    .print_document(ctx, entity, id, print_document.format.as_deref(), &[])
                    .await?;
                Ok(())
            }
            AutomationAction::Named { kind, params } => match kind.as_str() {
                "notify" => {
                    let notify: qefro_core::NotifyAction =
                        serde_json::from_value(params.clone()).unwrap_or_default();
                    self.action_notify(ctx, event, &notify).await
                }
                "send_communication" => {
                    let spec: qefro_core::CommunicationAction =
                        serde_json::from_value(params.clone()).unwrap_or_default();
                    self.action_send_communication(ctx, event, &spec).await
                }
                other => Err(QefroError::bad_request(format!(
                    "unknown automation action '{other}'"
                ))),
            },
        }
        .map_err(|e| {
            tracing::error!(
                tenant_id = %ctx.tenant_id,
                execution_id = %ctx.request_id,
                automation = %def.id_key(),
                action = action.kind(),
                error = %e,
                "automation action failed"
            );
            e
        })
    }

    async fn action_notify(
        &self,
        ctx: &OpContext,
        event: &DomainEvent,
        notify: &qefro_core::NotifyAction,
    ) -> QefroResult<()> {
        if let Some(name) = &notify.notification {
            if let Some(def) = self.notifications.iter().find(|n| n.name == *name) {
                return deliver_notification(
                    &self.pool,
                    &self.store,
                    &self.jobs,
                    event,
                    def,
                    ctx.request_id,
                )
                .await;
            }
        }
        let mut recipients = notify.recipients.clone();
        if let Some(role) = &notify.role {
            recipients.push(role.clone());
        }
        if recipients.is_empty() {
            recipients.push("Staff".into());
        }
        let def = NotificationDef {
            name: "automation.notify".into(),
            event: event.name.clone(),
            channels: vec!["in_app".into()],
            recipients,
            title: notify.title.clone(),
            body: notify.body.clone(),
            module: None,
        };
        deliver_notification(
            &self.pool,
            &self.store,
            &self.jobs,
            event,
            &def,
            ctx.request_id,
        )
        .await
    }

    async fn action_send_communication(
        &self,
        ctx: &OpContext,
        event: &DomainEvent,
        spec: &qefro_core::CommunicationAction,
    ) -> QefroResult<()> {
        let template = spec
            .template
            .as_deref()
            .ok_or_else(|| QefroError::bad_request("send_communication requires template"))?;
        let def = self
            .communications
            .iter()
            .find(|d| d.name == template)
            .ok_or_else(|| {
                QefroError::not_found(format!("communication '{template}' not found"))
            })?;
        let _ = enqueue_communication(
            &self.jobs,
            &self.comm_store,
            self.entities()?,
            ctx,
            def,
            Some(event),
            event.entity_id,
            spec.channel.as_deref(),
        )
        .await?;
        Ok(())
    }

    async fn action_webhook(
        &self,
        ctx: &OpContext,
        event: &DomainEvent,
        spec: &qefro_core::WebhookAction,
    ) -> QefroResult<()> {
        let name = spec
            .webhook
            .as_deref()
            .or(spec.name.as_deref())
            .ok_or_else(|| QefroError::bad_request("send_webhook requires webhook name"))?;
        let hook = self
            .webhooks
            .iter()
            .find(|w| w.name == name)
            .ok_or_else(|| QefroError::not_found(format!("webhook '{name}' not found")))?;
        if !hook.enabled {
            return Ok(());
        }
        let key = format!("{}:{}:{}", hook.name, event.id, ctx.request_id);
        self.jobs
            .enqueue(
                ctx,
                "webhook.deliver",
                json!({
                    "idempotency_key": key,
                    "webhook": hook.name,
                    "event": event.name,
                    "event_id": event.id,
                    "entity": event.entity,
                    "record_id": event.entity_id,
                    "target": hook.target,
                    "secret_env": hook.secret_env,
                    "timestamp": event.timestamp.timestamp(),
                    "payload": event.payload,
                    "execution_id": ctx.request_id,
                }),
            )
            .await?;
        Ok(())
    }

    async fn action_activity(
        &self,
        ctx: &OpContext,
        event: &DomainEvent,
        spec: &qefro_core::ActivityAction,
    ) -> QefroResult<()> {
        let message = spec
            .message
            .clone()
            .unwrap_or_else(|| format!("{} automation", event.entity));
        let atype = spec
            .activity_type
            .clone()
            .unwrap_or_else(|| crate::activity::TYPE_SYSTEM.to_string());
        self.entities()?
            .activity
            .record(
                ctx,
                &event.entity,
                event.entity_id,
                &atype,
                &message,
                json!({ "source": "automation" }),
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl EventHandler for AutomationEngine {
    async fn handle(&self, event: &DomainEvent) -> QefroResult<()> {
        if let Err(err) = self.enqueue_for_event(event).await {
            tracing::error!(error = %err, event = %event.name, "automation enqueue failed");
        }
        Ok(())
    }
}

#[async_trait]
impl JobHandler for AutomationEngine {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        self.run_payload(ctx, payload).await
    }
}

pub fn event_view(event: &DomainEvent) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("entity".into(), json!(event.entity));
    map.insert("record_id".into(), json!(event.entity_id));
    map.insert("entity_id".into(), json!(event.entity_id));
    map.insert("event".into(), json!(event.name));
    map.insert("event_type".into(), json!(event.name));
    map.insert("tenant_id".into(), json!(event.tenant_id));
    map.insert("actor".into(), json!(event.user_id));
    if let Some(obj) = event.payload.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
        if let Some(to) = obj.get("to").cloned() {
            map.insert("to_state".into(), to);
        }
        if let Some(from) = obj.get("from").cloned() {
            map.insert("from_state".into(), from);
        }
        if let Some(status) = obj.get("status").cloned() {
            if !map.contains_key("to_state") {
                map.insert("to_state".into(), status.clone());
            }
            map.entry("to".to_string()).or_insert(status);
        }
    }
    Value::Object(map)
}

fn event_from_json(value: Value) -> DomainEvent {
    let id = value
        .get("id")
        .or_else(|| value.get("event_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);
    let name = value
        .get("name")
        .or_else(|| value.get("event_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("automation.scheduled")
        .to_string();
    let entity = value
        .get("entity")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let entity_id = value
        .get("entity_id")
        .or_else(|| value.get("record_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(Uuid::nil());
    let tenant_id = value
        .get("tenant_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(Uuid::nil());
    let user_id = value
        .get("user_id")
        .or_else(|| value.get("actor"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let payload = value.get("payload").cloned().unwrap_or(json!({}));
    DomainEvent {
        id,
        name,
        entity,
        entity_id,
        tenant_id,
        timestamp: chrono::Utc::now(),
        payload,
        user_id,
    }
}

fn scheduled_event(ctx: &OpContext, def: &AutomationDef, event_id: Uuid) -> DomainEvent {
    DomainEvent {
        id: event_id,
        name: "automation.scheduled".into(),
        entity: String::new(),
        entity_id: Uuid::nil(),
        tenant_id: ctx.tenant_id,
        timestamp: chrono::Utc::now(),
        payload: json!({ "automation": def.name, "schedule": def.trigger.schedule }),
        user_id: Some(ctx.user_id),
    }
}

fn resolve_id(raw: Option<&str>, event: &DomainEvent) -> QefroResult<Uuid> {
    match raw {
        None | Some("record_id") | Some("{{record_id}}") | Some("$record_id") => {
            Ok(event.entity_id)
        }
        Some(s) => Uuid::parse_str(s)
            .or_else(|_| Uuid::parse_str(&interpolate(s, event).as_str().unwrap_or("")))
            .map_err(|_| QefroError::bad_request("invalid record_id")),
    }
}

fn interpolate(s: &str, event: &DomainEvent) -> Value {
    match s {
        "{{record_id}}" | "$record_id" => json!(event.entity_id),
        "{{entity}}" | "$entity" => json!(event.entity),
        "{{event}}" | "$event" => json!(event.name),
        other => json!(other),
    }
}

fn interpolate_value(value: &Value, event: &DomainEvent) -> Value {
    match value {
        Value::String(s) => interpolate(s, event),
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| interpolate_value(v, event)).collect())
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), interpolate_value(v, event));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

enum StepResult {
    Done,
    Waiting {
        run_at: chrono::DateTime<chrono::Utc>,
        cursor: Vec<usize>,
    },
}

struct StoredExecution {
    cursor: Vec<usize>,
    log: Value,
    snapshot: Option<AutomationDef>,
}

fn cursor_from_value(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn push_log(log: &mut Value, kind: &str, status: &str, message: &str) {
    if !log.is_array() {
        *log = json!([]);
    }
    if let Some(arr) = log.as_array_mut() {
        arr.push(json!({
            "kind": kind,
            "status": status,
            "message": message,
            "at": chrono::Utc::now().to_rfc3339(),
        }));
    }
}

fn plan_walk(steps: &[AutomationStep], view: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for step in steps {
        match step {
            AutomationStep::Wait { wait } => {
                out.push(json!({ "kind": "wait", "label": wait.label() }))
            }
            AutomationStep::End { .. } => out.push(json!({ "kind": "end" })),
            AutomationStep::Action(a) => out.push(json!({ "kind": a.kind() })),
            AutomationStep::Branch {
                condition,
                then,
                otherwise,
            } => {
                let ok = condition.matches(view);
                out.push(json!({ "kind": "condition", "result": ok }));
                let branch = if ok {
                    then.as_slice()
                } else {
                    otherwise.as_slice()
                };
                out.extend(plan_walk(branch, view));
            }
        }
    }
    out
}

fn uuid_from_key(key: &str) -> Uuid {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(key.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

async fn load_roles(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) -> QefroResult<Vec<String>> {
    let roles: Option<Vec<String>> =
        sqlx::query_scalar("SELECT roles FROM user_tenants WHERE tenant_id = $1 AND user_id = $2")
            .bind(tenant_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(roles.unwrap_or_default())
}

pub async fn deliver_notification(
    pool: &PgPool,
    store: &NotificationStore,
    jobs: &JobQueue,
    event: &DomainEvent,
    def: &NotificationDef,
    execution_id: Uuid,
) -> QefroResult<()> {
    let users = crate::notifications::recipient_users(pool, event, def).await?;
    let title = def
        .title
        .clone()
        .unwrap_or_else(|| event.name.replace('.', " "));
    let mut body = def.body.clone().unwrap_or_default();
    let mut payload = event.payload.clone();
    strip_secrets(None, &mut payload);
    if body.is_empty() {
        body = event.entity.clone();
    }
    for user_id in users {
        if def.channels.iter().any(|c| c == "in_app") {
            store
                .insert(&crate::notifications::InAppNotification {
                    id: Uuid::new_v4(),
                    tenant_id: event.tenant_id,
                    user_id,
                    title: title.clone(),
                    body: body.clone(),
                    entity: Some(event.entity.clone()),
                    record_id: Some(event.entity_id),
                    read_at: None,
                    created_at: chrono::Utc::now(),
                })
                .await?;
        }
        if def.channels.iter().any(|c| c == "email") {
            let ctx = OpContext::worker(event.tenant_id, user_id);
            jobs.enqueue(
                &ctx,
                "notify.email",
                json!({
                    "idempotency_key": format!("email:{}:{}:{}", def.name, event.id, user_id),
                    "user_id": user_id,
                    "title": title,
                    "body": body,
                    "event": event.name,
                    "entity": event.entity,
                    "record_id": event.entity_id,
                    "execution_id": execution_id,
                }),
            )
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::Condition;

    #[test]
    fn view_exposes_to_state() {
        let event = DomainEvent::new(
            "workflow.transitioned",
            "Order",
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({ "from": "Preparing", "to": "Ready" }),
        );
        let view = event_view(&event);
        let cond = Condition::all(vec![
            Condition::field_equals("entity", "Order"),
            Condition::field_equals("to_state", "ready"),
        ]);
        assert!(cond.matches(&view));
    }

    #[test]
    fn plan_walk_branches() {
        use qefro_core::{AutomationAction, AutomationStep};
        let steps = vec![
            AutomationStep::wait("0s"),
            AutomationStep::branch(
                Condition::field_equals("status", "Draft"),
                vec![AutomationStep::action(AutomationAction::notify("Manager"))],
                vec![AutomationStep::End { end: true }],
            ),
        ];
        let plan = plan_walk(&steps, &json!({ "status": "Draft" }));
        assert!(plan.iter().any(|s| s["kind"] == "wait"));
        assert!(plan.iter().any(|s| s["kind"] == "notify"));
        let other = plan_walk(&steps, &json!({ "status": "Confirmed" }));
        assert!(other.iter().any(|s| s["kind"] == "end"));
    }
}
