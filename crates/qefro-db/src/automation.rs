//! Automation runtime: match DomainEvents, enqueue JobQueue work, execute
//! actions through EntityService / NotificationDef / WebhookDef.

use crate::communication::{enqueue_communication, CommunicationStore};
use crate::jobs::{JobHandler, JobQueue};
use crate::notifications::NotificationStore;
use crate::service::EntityService;
use async_trait::async_trait;
use qefro_core::{
    next_run_after, parse_cron, schedule_slot_key, strip_secrets, AutomationAction, AutomationDef,
    CommunicationDef, NotificationDef, OpContext, QefroError, QefroResult, WebhookDef, ROLE_WORKER,
};
use qefro_events::{DomainEvent, EventHandler};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
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
        }
    }

    pub fn bind(&self, entities: Arc<EntityService>) {
        let _ = self.entities.set(entities);
    }

    pub fn defs(&self) -> &[AutomationDef] {
        &self.defs
    }

    fn entities(&self) -> QefroResult<&EntityService> {
        self.entities
            .get()
            .map(|e| e.as_ref())
            .ok_or_else(|| QefroError::internal("automation engine is not bound"))
    }

    pub async fn enqueue_for_event(&self, event: &DomainEvent) -> QefroResult<()> {
        let view = event_view(event);
        for def in &self.defs {
            if !def.matches_event(&event.name) {
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
        for def in &self.defs {
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
        let def = self
            .defs
            .iter()
            .find(|d| d.id_key() == automation_id || d.name == automation_id)
            .ok_or_else(|| {
                QefroError::not_found(format!("automation '{automation_id}' not found"))
            })?
            .clone();
        if !def.enabled {
            return Ok(());
        }
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
        let event = payload
            .get("event")
            .cloned()
            .map(event_from_json)
            .unwrap_or_else(|| scheduled_event(ctx, &def, event_id));
        let mut run_ctx = self.action_context(ctx, &def, &event, execution_id).await;
        let result = self.execute_actions(&mut run_ctx, &def, &event).await;
        let status = if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        let err = result.as_ref().err().map(|e| e.to_string());
        sqlx::query(
            "UPDATE qefro_automation_executions SET status = $2, error = $3 WHERE tenant_id = $1 AND automation_id = $4 AND event_id = $5",
        )
        .bind(ctx.tenant_id)
        .bind(status)
        .bind(err.as_deref())
        .bind(def.id_key())
        .bind(event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        result.map_err(|e| QefroError::business("automation_failed", e.to_string()))
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
            Some("completed") | Some("succeeded") => Ok(false),
            _ => {
                sqlx::query(
                    "UPDATE qefro_automation_executions SET status = 'running', execution_id = $4, error = NULL WHERE tenant_id = $1 AND automation_id = $2 AND event_id = $3 AND status <> 'completed' AND status <> 'succeeded'",
                )
                .bind(tenant_id)
                .bind(automation_id)
                .bind(event_id)
                .bind(execution_id)
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
                Ok(true)
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
        let mut roles = def.as_roles.clone();
        let mut user_id = event.user_id.unwrap_or(job_ctx.user_id);
        if roles.is_empty() {
            if let Some(uid) = event.user_id {
                if let Ok(loaded) = load_roles(&self.pool, event.tenant_id, uid).await {
                    roles = loaded;
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

    async fn execute_actions(
        &self,
        ctx: &mut OpContext,
        def: &AutomationDef,
        event: &DomainEvent,
    ) -> QefroResult<()> {
        for action in &def.actions {
            self.execute_action(ctx, def, event, action).await?;
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
                self.entities()?
                    .update(ctx, entity, id, update_entity.fields.clone())
                    .await?;
                Ok(())
            }
            AutomationAction::CreateEntity { create_entity } => {
                self.entities()?
                    .create(ctx, &create_entity.entity, create_entity.fields.clone())
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
    if s == "{{record_id}}" || s == "$record_id" {
        return json!(event.entity_id);
    }
    json!(s)
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
}
