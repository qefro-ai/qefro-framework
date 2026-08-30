use async_trait::async_trait;
use chrono::{DateTime, Utc};
use qefro_core::QefroResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: Uuid,
    pub name: String,
    pub entity: String,
    pub entity_id: Uuid,
    pub tenant_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub payload: Value,
    #[serde(default)]
    pub user_id: Option<Uuid>,
}

impl DomainEvent {
    pub fn new(
        name: impl Into<String>,
        entity: impl Into<String>,
        entity_id: Uuid,
        tenant_id: Uuid,
        payload: Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            entity: entity.into(),
            entity_id,
            tenant_id,
            timestamp: Utc::now(),
            payload,
            user_id: None,
        }
    }

    pub fn to_public_json(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "event_id": self.id,
            "name": self.name,
            "event_type": self.name,
            "entity": self.entity,
            "entity_id": self.entity_id,
            "record_id": self.entity_id,
            "tenant_id": self.tenant_id,
            "timestamp": self.timestamp,
            "payload": self.payload,
            "user_id": self.user_id,
            "actor": self.user_id,
        })
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &DomainEvent) -> QefroResult<()>;
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> QefroResult<()>;
    fn subscribe(&self, name: &str, handler: Arc<dyn EventHandler>);
}

/// In-process bus. A Redis/queue adapter can implement `EventBus` later
/// without changing publishers.
#[derive(Default, Clone)]
pub struct InProcessEventBus {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default)]
struct Inner {
    handlers: HashMap<String, Vec<Arc<dyn EventHandler>>>,
    /// Recent events for admin/debug. Not a durable log.
    log: Vec<DomainEvent>,
}

impl InProcessEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn recent(&self, limit: usize) -> Vec<DomainEvent> {
        let inner = self.inner.read().await;
        inner.log.iter().rev().take(limit).cloned().collect()
    }

    pub async fn recent_for_tenant(&self, tenant_id: Uuid, limit: usize) -> Vec<DomainEvent> {
        let inner = self.inner.read().await;
        inner
            .log
            .iter()
            .rev()
            .filter(|e| e.tenant_id == tenant_id)
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn subscribe_async(&self, name: &str, handler: Arc<dyn EventHandler>) {
        let mut inner = self.inner.write().await;
        inner
            .handlers
            .entry(name.to_string())
            .or_default()
            .push(handler);
    }
}

#[async_trait]
impl EventBus for InProcessEventBus {
    async fn publish(&self, event: DomainEvent) -> QefroResult<()> {
        tracing::info!(
            event = %event.name,
            entity = %event.entity,
            entity_id = %event.entity_id,
            tenant_id = %event.tenant_id,
            request_id = ?event.user_id,
            "domain event"
        );
        let handlers = {
            let mut inner = self.inner.write().await;
            inner.log.push(event.clone());
            if inner.log.len() > 1000 {
                let drain = inner.log.len() - 1000;
                inner.log.drain(0..drain);
            }
            let mut hs = inner.handlers.get(&event.name).cloned().unwrap_or_default();
            if let Some(wildcard) = inner.handlers.get("*") {
                hs.extend(wildcard.iter().cloned());
            }
            hs
        };
        for handler in handlers {
            if let Err(err) = handler.handle(&event).await {
                tracing::error!(error = %err, event = %event.name, "event handler failed");
            }
        }
        Ok(())
    }

    fn subscribe(&self, name: &str, handler: Arc<dyn EventHandler>) {
        let mut inner = self.inner.blocking_write();
        inner
            .handlers
            .entry(name.to_string())
            .or_default()
            .push(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::Mutex;

    struct Capture(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl EventHandler for Capture {
        async fn handle(&self, event: &DomainEvent) -> QefroResult<()> {
            self.0.lock().await.push(event.name.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn publish_and_subscribe() {
        let bus = InProcessEventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let mut inner = bus.inner.write().await;
            inner
                .handlers
                .entry("customer.created".into())
                .or_default()
                .push(Arc::new(Capture(seen.clone())));
        }
        bus.publish(DomainEvent::new(
            "customer.created",
            "Customer",
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({}),
        ))
        .await
        .unwrap();
        assert_eq!(*seen.lock().await, vec!["customer.created"]);
    }
}
