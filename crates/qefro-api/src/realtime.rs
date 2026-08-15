use qefro_events::DomainEvent;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RealtimeMessage {
    pub tenant_id: Uuid,
    pub entity: String,
    pub record_id: Uuid,
    pub event: String,
    pub payload: Value,
}

#[derive(Clone)]
pub struct RealtimeHub {
    tx: broadcast::Sender<RealtimeMessage>,
}

impl RealtimeHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeMessage> {
        self.tx.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    pub fn publish(&self, event: &DomainEvent) {
        let changed: Vec<String> = event
            .payload
            .as_object()
            .map(|o| o.keys().cloned().filter(|k| !k.starts_with('_')).collect())
            .unwrap_or_default();
        let msg = RealtimeMessage {
            tenant_id: event.tenant_id,
            entity: event.entity.clone(),
            record_id: event.entity_id,
            event: event.name.clone(),
            payload: json!({
                "event": event.name,
                "entity": event.entity,
                "record_id": event.entity_id,
                "changed_fields": changed,
            }),
        };
        let _ = self.tx.send(msg);
    }
}

impl Default for RealtimeHub {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RealtimeFanout(pub Arc<RealtimeHub>);

#[async_trait::async_trait]
impl qefro_events::EventHandler for RealtimeFanout {
    async fn handle(&self, event: &DomainEvent) -> qefro_core::QefroResult<()> {
        self.0.publish(event);
        Ok(())
    }
}
