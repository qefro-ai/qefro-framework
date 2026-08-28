use qefro_core::{webhook_secret, webhook_signature, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub webhook: String,
    pub event: String,
    pub event_id: Uuid,
    pub target: String,
    pub status_code: Option<i32>,
    pub success: bool,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct WebhookLog {
    pool: PgPool,
}

impl WebhookLog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        tenant_id: Uuid,
        webhook: &str,
        event: &str,
        event_id: Uuid,
        target: &str,
        status_code: Option<i32>,
        success: bool,
        attempts: i32,
        last_error: Option<&str>,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO qefro_webhook_deliveries (
                id, tenant_id, webhook, event, event_id, target, status_code,
                success, attempts, last_error, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, now())
            ON CONFLICT (tenant_id, webhook, event_id) DO UPDATE SET
                status_code = EXCLUDED.status_code,
                success = EXCLUDED.success,
                attempts = EXCLUDED.attempts,
                last_error = EXCLUDED.last_error
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(webhook)
        .bind(event)
        .bind(event_id)
        .bind(target)
        .bind(status_code)
        .bind(success)
        .bind(attempts)
        .bind(last_error)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        webhook: Option<&str>,
    ) -> QefroResult<Vec<WebhookDelivery>> {
        if let Some(name) = webhook {
            sqlx::query_as::<_, WebhookDelivery>(
                r#"
                SELECT id, tenant_id, webhook, event, event_id, target, status_code,
                       success, attempts, last_error, created_at
                FROM qefro_webhook_deliveries
                WHERE tenant_id = $1 AND webhook = $2
                ORDER BY created_at DESC LIMIT 100
                "#,
            )
            .bind(tenant_id)
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
        } else {
            sqlx::query_as::<_, WebhookDelivery>(
                r#"
                SELECT id, tenant_id, webhook, event, event_id, target, status_code,
                       success, attempts, last_error, created_at
                FROM qefro_webhook_deliveries
                WHERE tenant_id = $1
                ORDER BY created_at DESC LIMIT 100
                "#,
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
        }
    }
}

pub fn signed_headers(
    def_secret_env: Option<&str>,
    event: &str,
    event_id: Uuid,
    timestamp: i64,
    body: &[u8],
) -> Vec<(String, String)> {
    let secret = webhook_secret(&qefro_core::WebhookDef {
        name: String::new(),
        event: String::new(),
        target: String::new(),
        enabled: true,
        secret_env: def_secret_env.map(|s| s.to_string()),
        module: None,
    });
    let sig = webhook_signature(&secret, timestamp, &event_id.to_string(), body);
    vec![
        ("X-Qefro-Event".into(), event.to_string()),
        ("X-Qefro-Event-ID".into(), event_id.to_string()),
        ("X-Qefro-Timestamp".into(), timestamp.to_string()),
        ("X-Qefro-Signature".into(), sig),
    ]
}

pub fn payload_bytes(payload: &Value) -> Vec<u8> {
    serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec())
}
