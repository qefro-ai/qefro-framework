use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Future metering hook. V0.4 records the event; it does not bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteringEvent {
    pub tenant_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub request_id: Uuid,
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub metadata: Value,
}

impl MeteringEvent {
    pub fn new(
        tenant_id: Uuid,
        event_type: impl Into<String>,
        resource: impl Into<String>,
        request_id: Uuid,
    ) -> Self {
        Self {
            tenant_id,
            timestamp: Utc::now(),
            event_type: event_type.into(),
            resource: resource.into(),
            resource_id: None,
            request_id,
            user_id: None,
            metadata: Value::Null,
        }
    }

    pub fn with_resource_id(mut self, id: impl Into<String>) -> Self {
        self.resource_id = Some(id.into());
        self
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Structured hook for future billing. Does not calculate charges.
    pub fn emit(self) {
        tracing::info!(
            tenant_id = %self.tenant_id,
            request_id = %self.request_id,
            event_type = %self.event_type,
            resource = %self.resource,
            resource_id = self.resource_id.as_deref().unwrap_or(""),
            "metering"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metering_event_carries_tenant() {
        let tenant = Uuid::new_v4();
        let ev = MeteringEvent::new(tenant, "api.request", "Reservation", Uuid::new_v4())
            .with_resource_id("abc");
        assert_eq!(ev.tenant_id, tenant);
        assert_eq!(ev.event_type, "api.request");
        assert_eq!(ev.resource_id.as_deref(), Some("abc"));
    }
}
