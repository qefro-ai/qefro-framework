use chrono::{DateTime, Utc};
use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub entity: String,
    pub entity_id: Option<Uuid>,
    pub action: String,
    pub old_values: Option<Value>,
    pub new_values: Option<Value>,
    pub request_id: Option<Uuid>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        ctx: &OpContext,
        entity: &str,
        entity_id: Option<Uuid>,
        action: &str,
        old_values: Option<&Value>,
        new_values: Option<&Value>,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, tenant_id, user_id, entity, entity_id, action,
                old_values, new_values, request_id, ip, user_agent, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctx.tenant_id)
        .bind(ctx.user_id)
        .bind(entity)
        .bind(entity_id)
        .bind(action)
        .bind(old_values.cloned())
        .bind(new_values.cloned())
        .bind(ctx.request_id)
        .bind(&ctx.ip)
        .bind(&ctx.user_agent)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn record_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity: &str,
        entity_id: Option<Uuid>,
        action: &str,
        old_values: Option<&Value>,
        new_values: Option<&Value>,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, tenant_id, user_id, entity, entity_id, action,
                old_values, new_values, request_id, ip, user_agent, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctx.tenant_id)
        .bind(ctx.user_id)
        .bind(entity)
        .bind(entity_id)
        .bind(action)
        .bind(old_values.cloned())
        .bind(new_values.cloned())
        .bind(ctx.request_id)
        .bind(&ctx.ip)
        .bind(&ctx.user_agent)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn list(
        &self,
        ctx: &OpContext,
        entity: Option<&str>,
        entity_id: Option<Uuid>,
        limit: i64,
    ) -> QefroResult<Vec<AuditRecord>> {
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query_as::<_, AuditRecord>(
            r#"
            SELECT id, tenant_id, user_id, entity, entity_id, action,
                   old_values, new_values, request_id, ip, user_agent, created_at
            FROM audit_logs
            WHERE tenant_id = $1
              AND ($2::text IS NULL OR entity = $2)
              AND ($3::uuid IS NULL OR entity_id = $3)
            ORDER BY created_at DESC
            LIMIT $4
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(entity)
        .bind(entity_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rows)
    }
}
