//! Due-date reminders via the existing JobQueue. Generic for any entity with `due_at`.

use crate::jobs::JobHandler;
use crate::outbox::Outbox;
use crate::repository::record_id;
use crate::service::EntityService;
use async_trait::async_trait;
use qefro_core::{ident::snake_case, OpContext, QefroError, QefroResult, STATUS_CANCELLED, STATUS_COMPLETED};
use qefro_events::DomainEvent;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

fn stable_id(kind: &str, entity: &str, record: Uuid, due: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"qefro:due:");
    hasher.update(kind.as_bytes());
    hasher.update(b":");
    hasher.update(entity.as_bytes());
    hasher.update(b":");
    hasher.update(record.as_bytes());
    hasher.update(b":");
    hasher.update(due.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

pub const DUE_REMINDER_JOB: &str = "due.reminder";

pub struct DueReminderJob {
    entities: OnceLock<Arc<EntityService>>,
}

impl DueReminderJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entities: OnceLock::new(),
        })
    }

    pub fn bind(&self, entities: Arc<EntityService>) {
        let _ = self.entities.set(entities);
    }
}

#[async_trait]
impl JobHandler for DueReminderJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(entities) = self.entities.get() else {
            return Err(QefroError::internal("due reminder job is not bound"));
        };
        let entity_name = payload
            .get("entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("entity is required"))?;
        let id = payload
            .get("record_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request("record_id is required"))?;
        let expected_due = payload.get("due_at").cloned().unwrap_or(Value::Null);
        let entity = entities.registry().get(entity_name)?;
        let record = match entities.repo.get(&entity, ctx, id).await {
            Ok(row) => row,
            Err(QefroError::NotFound { .. }) => return Ok(()),
            Err(e) => return Err(e),
        };
        if record.get("deleted_at").and_then(|v| v.as_str()).is_some() {
            return Ok(());
        }
        let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status.eq_ignore_ascii_case(STATUS_COMPLETED)
            || status.eq_ignore_ascii_case(STATUS_CANCELLED)
        {
            return Ok(());
        }
        let actual_due = record.get("due_at").cloned().unwrap_or(Value::Null);
        if actual_due != expected_due {
            return Ok(());
        }
        let mut event_payload = record.clone();
        qefro_core::strip_secrets(Some(&entity), &mut event_payload);
        let record_uuid = record_id(&record)?;
        let due_key = expected_due.as_str().unwrap_or("");
        let mut specific = DomainEvent::new(
            format!("{}.due", snake_case(&entity.name)),
            entity.name.clone(),
            record_uuid,
            ctx.tenant_id,
            event_payload.clone(),
        );
        specific.id = stable_id("specific", &entity.name, record_uuid, due_key);
        let mut generic = DomainEvent::new(
            "entity.due",
            entity.name.clone(),
            record_uuid,
            ctx.tenant_id,
            event_payload,
        );
        generic.id = stable_id("generic", &entity.name, record_uuid, due_key);
        let events = vec![specific, generic];
        let mut tx = entities
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Outbox::enqueue_many_tx(&mut tx, &events).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let _ = entities.dispatch_outbox().await;
        Ok(())
    }
}
