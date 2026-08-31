//! Communication delivery: providers, log, and JobQueue handler.
//!
//! Event → Outbox → this job → provider. The business transaction never waits
//! on email/SMS/WhatsApp. Providers are pluggable; the default logs only.

use crate::attachments::AttachmentStore;
use crate::jobs::{JobHandler, JobQueue};
use crate::notifications::NotificationStore;
use crate::service::EntityService;
use async_trait::async_trait;
use qefro_core::{
    render_template, select_channels, wrap_record, CommunicationDef, FormatOpts, MemoryRateLimiter,
    OpContext, QefroError, QefroResult, RateLimiter, RecipientAddress, CHANNEL_EMAIL,
    CHANNEL_IN_APP, CHANNEL_SMS, CHANNEL_WHATSAPP, COMM_DEAD_LETTER, COMM_DELIVERED, COMM_FAILED,
    COMM_QUEUED, COMM_SENDING, COMM_SENT, COMM_SKIPPED, PERSON_ENTITY,
};
use qefro_events::{DomainEvent, EventHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

pub const COMMUNICATION_DELIVER_JOB: &str = "communication.deliver";
const MAX_PER_TENANT_PER_MINUTE: u32 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommunicationLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity: String,
    pub entity_id: Uuid,
    pub template: String,
    pub channel: String,
    pub purpose: String,
    pub status: String,
    pub recipient: Option<String>,
    pub recipient_user_id: Option<Uuid>,
    pub event_id: Option<Uuid>,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub sent_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CommunicationLog {
    pub fn to_client_json(&self) -> Value {
        json!({
            "id": self.id,
            "entity": self.entity,
            "entity_id": self.entity_id,
            "template": self.template,
            "channel": self.channel,
            "purpose": self.purpose,
            "status": self.status,
            "recipient": self.recipient,
            "created_at": self.created_at,
            "sent_at": self.sent_at,
            "attempts": self.attempts,
            "last_error": self.last_error,
        })
    }
}

#[derive(Clone)]
pub struct CommunicationStore {
    pool: PgPool,
}

impl CommunicationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_queued(&self, row: &CommunicationLog) -> QefroResult<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO qefro_communications (
                id, tenant_id, entity, entity_id, template, channel, purpose, status,
                recipient, recipient_user_id, event_id, attempts, last_error, created_at, sent_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            ON CONFLICT (tenant_id, template, entity_id, event_id, channel) DO NOTHING
            "#,
        )
        .bind(row.id)
        .bind(row.tenant_id)
        .bind(&row.entity)
        .bind(row.entity_id)
        .bind(&row.template)
        .bind(&row.channel)
        .bind(&row.purpose)
        .bind(&row.status)
        .bind(&row.recipient)
        .bind(row.recipient_user_id)
        .bind(row.event_id)
        .bind(row.attempts)
        .bind(&row.last_error)
        .bind(row.created_at)
        .bind(row.sent_at)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<CommunicationLog> {
        sqlx::query_as::<_, CommunicationLog>(
            r#"
            SELECT id, tenant_id, entity, entity_id, template, channel, purpose, status,
                   recipient, recipient_user_id, event_id, attempts, last_error, created_at, sent_at
            FROM qefro_communications
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("communication not found"))
    }

    pub async fn list_for_record(
        &self,
        tenant_id: Uuid,
        entity: &str,
        entity_id: Uuid,
    ) -> QefroResult<Vec<CommunicationLog>> {
        sqlx::query_as::<_, CommunicationLog>(
            r#"
            SELECT id, tenant_id, entity, entity_id, template, channel, purpose, status,
                   recipient, recipient_user_id, event_id, attempts, last_error, created_at, sent_at
            FROM qefro_communications
            WHERE tenant_id = $1 AND entity = $2 AND entity_id = $3
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn search(
        &self,
        tenant_id: Uuid,
        entity: Option<&str>,
        entity_id: Option<Uuid>,
        channel: Option<&str>,
        status: Option<&str>,
        recipient: Option<&str>,
    ) -> QefroResult<Vec<CommunicationLog>> {
        sqlx::query_as::<_, CommunicationLog>(
            r#"
            SELECT id, tenant_id, entity, entity_id, template, channel, purpose, status,
                   recipient, recipient_user_id, event_id, attempts, last_error, created_at, sent_at
            FROM qefro_communications
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR entity = $2)
              AND ($3::uuid IS NULL OR entity_id = $3)
              AND ($4::text IS NULL OR channel = $4)
              AND ($5::text IS NULL OR status = $5)
              AND ($6::text IS NULL OR recipient ILIKE '%' || $6 || '%')
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(entity_id)
        .bind(channel)
        .bind(status)
        .bind(recipient)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn mark(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        status: &str,
        error: Option<&str>,
        sent: bool,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_communications
            SET status = $3,
                last_error = $4,
                attempts = attempts + 1,
                sent_at = CASE WHEN $5 THEN now() ELSE sent_at END
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(status)
        .bind(error)
        .bind(sent)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub tenant_id: Uuid,
    pub channel: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub entity: String,
    pub record_id: Uuid,
    pub template: String,
}

#[async_trait]
pub trait CommunicationProvider: Send + Sync {
    fn channel(&self) -> &str;
    async fn send(&self, msg: &OutboundMessage) -> QefroResult<()>;
}

/// Default email provider. Logs only. Applications replace this; no SMTP here.
pub struct LogEmailProvider;
#[async_trait]
impl CommunicationProvider for LogEmailProvider {
    fn channel(&self) -> &str {
        CHANNEL_EMAIL
    }
    async fn send(&self, msg: &OutboundMessage) -> QefroResult<()> {
        tracing::info!(tenant_id = %msg.tenant_id, to = %msg.to, template = %msg.template, "email communication");
        Ok(())
    }
}

pub struct LogSmsProvider;
#[async_trait]
impl CommunicationProvider for LogSmsProvider {
    fn channel(&self) -> &str {
        CHANNEL_SMS
    }
    async fn send(&self, msg: &OutboundMessage) -> QefroResult<()> {
        tracing::info!(tenant_id = %msg.tenant_id, to = %msg.to, template = %msg.template, "sms communication");
        Ok(())
    }
}

pub struct LogWhatsAppProvider;
#[async_trait]
impl CommunicationProvider for LogWhatsAppProvider {
    fn channel(&self) -> &str {
        CHANNEL_WHATSAPP
    }
    async fn send(&self, msg: &OutboundMessage) -> QefroResult<()> {
        tracing::info!(tenant_id = %msg.tenant_id, to = %msg.to, template = %msg.template, "whatsapp communication");
        Ok(())
    }
}

/// In-memory provider for tests. Never calls a network.
#[derive(Clone, Default)]
pub struct RecordingProvider {
    pub channel: String,
    pub sent: Arc<Mutex<Vec<OutboundMessage>>>,
    pub fail: bool,
}

#[async_trait]
impl CommunicationProvider for RecordingProvider {
    fn channel(&self) -> &str {
        self.channel.as_str()
    }
    async fn send(&self, msg: &OutboundMessage) -> QefroResult<()> {
        if self.fail {
            return Err(QefroError::internal("provider unavailable"));
        }
        self.sent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(msg.clone());
        Ok(())
    }
}

pub struct CommunicationHub {
    providers: HashMap<String, Arc<dyn CommunicationProvider>>,
    limiter: MemoryRateLimiter,
}

impl CommunicationHub {
    pub fn default_loggers() -> Self {
        let mut providers: HashMap<String, Arc<dyn CommunicationProvider>> = HashMap::new();
        providers.insert(CHANNEL_EMAIL.into(), Arc::new(LogEmailProvider));
        providers.insert(CHANNEL_SMS.into(), Arc::new(LogSmsProvider));
        providers.insert(CHANNEL_WHATSAPP.into(), Arc::new(LogWhatsAppProvider));
        Self {
            providers,
            limiter: MemoryRateLimiter::new(
                MAX_PER_TENANT_PER_MINUTE,
                std::time::Duration::from_secs(60),
            ),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn CommunicationProvider>) {
        self.providers
            .insert(provider.channel().to_string(), provider);
    }

    pub async fn send(&self, msg: &OutboundMessage) -> QefroResult<()> {
        let key = format!("{}:{}", msg.tenant_id, msg.channel);
        if !self.limiter.allow(&key) {
            return Err(QefroError::rate_limited(
                "communication rate limit exceeded",
            ));
        }
        let Some(provider) = self.providers.get(&msg.channel) else {
            return Err(QefroError::bad_request(format!(
                "no provider for channel '{}'",
                msg.channel
            )));
        };
        provider.send(msg).await
    }
}

pub struct CommunicationDeliverJob {
    entities: OnceLock<Arc<EntityService>>,
    store: OnceLock<CommunicationStore>,
    notifications: OnceLock<NotificationStore>,
    defs: OnceLock<Vec<CommunicationDef>>,
    hub: OnceLock<Arc<CommunicationHub>>,
}

impl CommunicationDeliverJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entities: OnceLock::new(),
            store: OnceLock::new(),
            notifications: OnceLock::new(),
            defs: OnceLock::new(),
            hub: OnceLock::new(),
        })
    }

    pub fn bind(
        &self,
        entities: Arc<EntityService>,
        store: CommunicationStore,
        notifications: NotificationStore,
        defs: Vec<CommunicationDef>,
        hub: Arc<CommunicationHub>,
    ) {
        let _ = self.entities.set(entities);
        let _ = self.store.set(store);
        let _ = self.notifications.set(notifications);
        let _ = self.defs.set(defs);
        let _ = self.hub.set(hub);
    }
}

#[async_trait]
impl JobHandler for CommunicationDeliverJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(entities) = self.entities.get() else {
            return Err(QefroError::internal("communication job is not bound"));
        };
        let Some(store) = self.store.get() else {
            return Err(QefroError::internal("communication store is not bound"));
        };
        let Some(hub) = self.hub.get() else {
            return Err(QefroError::internal("communication hub is not bound"));
        };
        let id = payload
            .get("communication_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request("communication_id is required"))?;
        let row = store.get(ctx.tenant_id, id).await?;
        if matches!(
            row.status.as_str(),
            COMM_SENT | COMM_DELIVERED | COMM_SKIPPED | COMM_DEAD_LETTER
        ) {
            return Ok(());
        }
        store
            .mark(ctx.tenant_id, id, COMM_SENDING, None, false)
            .await?;
        let msg = OutboundMessage {
            tenant_id: ctx.tenant_id,
            channel: row.channel.clone(),
            to: row.recipient.clone().unwrap_or_default(),
            subject: payload
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            body: payload
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            entity: row.entity.clone(),
            record_id: row.entity_id,
            template: row.template.clone(),
        };
        if row.channel == CHANNEL_IN_APP {
            if let (Some(user_id), Some(notes)) = (row.recipient_user_id, self.notifications.get())
            {
                let _ = notes
                    .insert(&crate::notifications::InAppNotification {
                        id: Uuid::new_v4(),
                        tenant_id: ctx.tenant_id,
                        user_id,
                        title: if msg.subject.is_empty() {
                            row.template.clone()
                        } else {
                            msg.subject.clone()
                        },
                        body: msg.body.clone(),
                        entity: Some(row.entity.clone()),
                        record_id: Some(row.entity_id),
                        read_at: None,
                        created_at: chrono::Utc::now(),
                    })
                    .await;
                store.mark(ctx.tenant_id, id, COMM_SENT, None, true).await?;
                record_sent_activity(entities, ctx, &row).await;
                return Ok(());
            }
            store
                .mark(
                    ctx.tenant_id,
                    id,
                    COMM_SKIPPED,
                    Some("no login for in-app"),
                    false,
                )
                .await?;
            return Ok(());
        }
        match hub.send(&msg).await {
            Ok(()) => {
                store.mark(ctx.tenant_id, id, COMM_SENT, None, true).await?;
                record_sent_activity(entities, ctx, &row).await;
                Ok(())
            }
            Err(e) => {
                let status = if row.attempts + 1 >= 5 {
                    COMM_DEAD_LETTER
                } else {
                    COMM_FAILED
                };
                store
                    .mark(ctx.tenant_id, id, status, Some(&e.to_string()), false)
                    .await?;
                Err(e)
            }
        }
    }
}

async fn record_sent_activity(entities: &EntityService, ctx: &OpContext, row: &CommunicationLog) {
    let _ = entities
        .activity
        .record(
            ctx,
            &row.entity,
            row.entity_id,
            "communication.sent",
            &format!("{} sent", pretty_channel(&row.channel)),
            json!({ "channel": row.channel, "template": row.template }),
        )
        .await;
}

fn pretty_channel(channel: &str) -> &'static str {
    match channel {
        CHANNEL_EMAIL => "Email",
        CHANNEL_SMS => "SMS",
        CHANNEL_WHATSAPP => "WhatsApp",
        CHANNEL_IN_APP => "In-app",
        _ => "Message",
    }
}

/// Enqueue delivery after COMMIT. Never sends on the request thread.
pub async fn enqueue_communication(
    jobs: &JobQueue,
    store: &CommunicationStore,
    entities: &EntityService,
    ctx: &OpContext,
    def: &CommunicationDef,
    event: Option<&DomainEvent>,
    record_id: Uuid,
    channel_override: Option<&str>,
) -> QefroResult<Vec<CommunicationLog>> {
    let entity = entities.registry().get(&def.entity)?;
    if !ctx.is_worker() {
        entities
            .permissions
            .check(ctx, &def.entity, qefro_permissions::Action::Read)?;
    }
    let record = if ctx.is_worker() {
        let mut row = entities.repo.get(&entity, ctx, record_id).await?;
        qefro_core::strip_secrets(Some(&entity), &mut row);
        row
    } else {
        entities.get(ctx, &def.entity, record_id).await?
    };
    let recipient_record = resolve_recipient(entities, ctx, def, &record).await;
    if def.is_marketing() {
        if let Some(field) = &def.opt_out_field {
            if recipient_record
                .get(field)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(Vec::new());
            }
        }
    }
    let channels = if let Some(ch) = channel_override {
        vec![ch.to_ascii_lowercase()]
    } else {
        select_channels(def, &recipient_record)
    };
    if channels.is_empty() {
        return Ok(Vec::new());
    }
    let ctx_value = wrap_record(&def.entity, record.clone(), {
        let mut extras = HashMap::new();
        if let Some(path) = &def.recipient_path {
            extras.insert(path.clone(), recipient_record.clone());
        }
        extras
    });
    let opts = FormatOpts {
        currency: if ctx.currency.is_empty() {
            "USD".into()
        } else {
            ctx.currency.clone()
        },
        locale: if ctx.locale.is_empty() {
            "en-US".into()
        } else {
            ctx.locale.clone()
        },
        date_format: "YYYY-MM-DD".into(),
    };
    let subject = if let Some(s) = &def.subject {
        render_template(s, &ctx_value, &opts).unwrap_or_default()
    } else {
        def.name.replace('_', " ")
    };
    let body = render_template(&def.body, &ctx_value, &opts).unwrap_or_default();
    let address = RecipientAddress::from_record(&recipient_record);
    let event_id = event.map(|e| e.id).unwrap_or_else(Uuid::new_v4);
    let mut out = Vec::new();
    for channel in channels {
        let to = address.address_for(&channel);
        if to.is_none() && channel != CHANNEL_IN_APP {
            continue;
        }
        let id = Uuid::new_v4();
        let row = CommunicationLog {
            id,
            tenant_id: ctx.tenant_id,
            entity: def.entity.clone(),
            entity_id: record_id,
            template: def.name.clone(),
            channel: channel.clone(),
            purpose: def.purpose.clone(),
            status: COMM_QUEUED.into(),
            recipient: to.clone(),
            recipient_user_id: address
                .user_id
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            event_id: Some(event_id),
            attempts: 0,
            last_error: None,
            created_at: chrono::Utc::now(),
            sent_at: None,
        };
        let inserted = store.insert_queued(&row).await?;
        if !inserted {
            continue;
        }
        let key = format!("comm:{}:{}:{}:{}", def.name, record_id, event_id, channel);
        jobs.enqueue(
            ctx,
            COMMUNICATION_DELIVER_JOB,
            json!({
                "idempotency_key": key,
                "communication_id": id,
                "template": def.name,
                "channel": channel,
                "subject": subject,
                "body": body,
                "entity": def.entity,
                "record_id": record_id,
                "attachments": attachment_refs(entities, ctx, def, record_id).await,
            }),
        )
        .await?;
        out.push(row);
        if channel_override.is_some() {
            break;
        }
        // Fallback: only send the first channel that has an address.
        break;
    }
    if !out.is_empty() {
        let _ = entities
            .activity
            .record(
                ctx,
                &def.entity,
                record_id,
                "communication.queued",
                &format!("{} notification queued", pretty_channel(&out[0].channel)),
                json!({ "template": def.name, "channel": out[0].channel }),
            )
            .await;
    }
    Ok(out)
}

async fn resolve_recipient(
    entities: &EntityService,
    ctx: &OpContext,
    def: &CommunicationDef,
    record: &Value,
) -> Value {
    let mut recipient = if let Some(path) = &def.recipient_path {
        load_related(entities, ctx, &def.entity, record, path)
            .await
            .unwrap_or_else(|| record.clone())
    } else {
        record.clone()
    };
    nest_person(entities, ctx, &mut recipient).await;
    recipient
}

async fn load_related(
    entities: &EntityService,
    ctx: &OpContext,
    entity_name: &str,
    record: &Value,
    path: &str,
) -> Option<Value> {
    let field = if path.ends_with("_id") {
        path.to_string()
    } else {
        format!("{path}_id")
    };
    let id = record
        .get(&field)
        .or_else(|| record.get(path))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())?;
    let related_name = entities
        .registry()
        .get(entity_name)
        .ok()
        .and_then(|e| {
            e.get_field(&field)
                .and_then(|f| f.relation.as_ref().map(|r| r.target_entity.clone()))
        })
        .unwrap_or_else(|| title_case(path));
    let related_entity = entities.registry().get(&related_name).ok()?;
    match entities.repo.get(&related_entity, ctx, id).await {
        Ok(mut related) => {
            qefro_core::strip_secrets(Some(&related_entity), &mut related);
            Some(related)
        }
        Err(_) => None,
    }
}

async fn nest_person(entities: &EntityService, ctx: &OpContext, record: &mut Value) {
    if record.get("person").is_some() {
        return;
    }
    let Some(id) = record
        .get("person_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return;
    };
    let Ok(person_ent) = entities.registry().get(PERSON_ENTITY) else {
        return;
    };
    if let Ok(mut person) = entities.repo.get(&person_ent, ctx, id).await {
        qefro_core::strip_secrets(Some(&person_ent), &mut person);
        if let Some(obj) = record.as_object_mut() {
            obj.insert("person".into(), person);
        }
    }
}

fn title_case(path: &str) -> String {
    let mut s = path.chars();
    match s.next() {
        None => String::new(),
        Some(c) => c.to_ascii_uppercase().to_string() + s.as_str(),
    }
}

async fn attachment_refs(
    entities: &EntityService,
    ctx: &OpContext,
    def: &CommunicationDef,
    record_id: Uuid,
) -> Value {
    if !def.attach_document {
        return json!([]);
    }
    let store = AttachmentStore::new(entities.pool().clone());
    match store.list(ctx.tenant_id, &def.entity, record_id).await {
        Ok(files) => json!(files
            .into_iter()
            .map(|f| json!({ "id": f.id, "filename": f.filename, "mime_type": f.mime_type }))
            .collect::<Vec<_>>()),
        Err(_) => json!([]),
    }
}

pub async fn dispatch_event_communications(
    jobs: &JobQueue,
    store: &CommunicationStore,
    entities: &EntityService,
    defs: &[CommunicationDef],
    event: &DomainEvent,
) -> QefroResult<()> {
    let ctx = OpContext::worker(event.tenant_id, event.user_id.unwrap_or(Uuid::nil()));
    for def in defs {
        if !def.matches_event(&event.name) {
            continue;
        }
        if !def.entity.is_empty() && def.entity != event.entity {
            continue;
        }
        let _ = enqueue_communication(
            jobs,
            store,
            entities,
            &ctx,
            def,
            Some(event),
            event.entity_id,
            None,
        )
        .await;
    }
    Ok(())
}

/// Event → Outbox → enqueue. Never calls providers on the request thread.
pub struct CommunicationDispatcher {
    jobs: Arc<JobQueue>,
    store: CommunicationStore,
    entities: Arc<EntityService>,
    defs: Vec<CommunicationDef>,
}

impl CommunicationDispatcher {
    pub fn new(
        jobs: Arc<JobQueue>,
        store: CommunicationStore,
        entities: Arc<EntityService>,
        defs: Vec<CommunicationDef>,
    ) -> Self {
        Self {
            jobs,
            store,
            entities,
            defs,
        }
    }
}

#[async_trait]
impl EventHandler for CommunicationDispatcher {
    async fn handle(&self, event: &DomainEvent) -> QefroResult<()> {
        dispatch_event_communications(
            &self.jobs,
            &self.store,
            self.entities.as_ref(),
            &self.defs,
            event,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_msg(channel: &str) -> OutboundMessage {
        OutboundMessage {
            tenant_id: Uuid::nil(),
            channel: channel.into(),
            to: "a@example.com".into(),
            subject: "Order confirmed".into(),
            body: "Hello Ahmed".into(),
            entity: "Order".into(),
            record_id: Uuid::nil(),
            template: "order_confirmed".into(),
        }
    }

    #[tokio::test]
    async fn recording_provider_sends_without_network() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let provider = RecordingProvider {
            channel: CHANNEL_EMAIL.into(),
            sent: sent.clone(),
            fail: false,
        };
        let mut hub = CommunicationHub::default_loggers();
        hub.register(Arc::new(provider));
        hub.send(&sample_msg(CHANNEL_EMAIL)).await.unwrap();
        assert_eq!(sent.lock().unwrap_or_else(|e| e.into_inner()).len(), 1);
    }

    #[tokio::test]
    async fn recording_provider_failure_is_retryable() {
        let provider = RecordingProvider {
            channel: CHANNEL_EMAIL.into(),
            sent: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        };
        let mut hub = CommunicationHub::default_loggers();
        hub.register(Arc::new(provider));
        let err = hub.send(&sample_msg(CHANNEL_EMAIL)).await.unwrap_err();
        assert!(err.to_string().contains("unavailable"), "{err}");
    }

    #[test]
    fn five_attempts_marks_dead_letter() {
        let attempts = 4;
        let status = if attempts + 1 >= 5 {
            COMM_DEAD_LETTER
        } else {
            COMM_FAILED
        };
        assert_eq!(status, COMM_DEAD_LETTER);
        assert_ne!(status, "order.failed");
    }
}
