//! Durable execution records for business operations.
//!
//! Sync CRUD does not write here. Named operations use this table for
//! idempotency replay and for asynchronous JobQueue runs. Status values match
//! JobQueue concepts: queued, running, completed, failed, cancelled.

use chrono::{DateTime, Utc};
use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const STATUS_QUEUED: &str = "queued";
pub const STATUS_RUNNING: &str = "running";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_CANCELLED: &str = "cancelled";

pub const OPERATION_EXECUTE_JOB: &str = "qefro.operation.execute";

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OperationRun {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub entity: String,
    pub entity_id: Uuid,
    pub operation: String,
    pub status: String,
    pub request_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
    pub progress: i32,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OperationRun {
    pub fn to_client_json(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "operation": self.operation,
            "entity": self.entity,
            "entity_id": self.entity_id,
            "status": self.status,
            "progress": self.progress,
            "request_id": self.request_id,
            "error": self.error,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
        })
    }
}

#[derive(Clone)]
pub struct OperationRunStore {
    pool: PgPool,
}

impl OperationRunStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, ctx: &OpContext, id: Uuid) -> QefroResult<OperationRun> {
        sqlx::query_as::<_, OperationRun>(
            r#"
            SELECT id, tenant_id, user_id, entity, entity_id, operation, status,
                   request_id, idempotency_key, progress, result, error,
                   started_at, completed_at, created_at, updated_at
            FROM qefro_operation_runs
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("operation run not found"))
    }

    pub async fn find_idempotent(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        key: &str,
    ) -> QefroResult<Option<OperationRun>> {
        sqlx::query_as::<_, OperationRun>(
            r#"
            SELECT id, tenant_id, user_id, entity, entity_id, operation, status,
                   request_id, idempotency_key, progress, result, error,
                   started_at, completed_at, created_at, updated_at
            FROM qefro_operation_runs
            WHERE tenant_id = $1 AND idempotency_key = $2
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn insert_tx(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        id: Uuid,
        entity: &str,
        entity_id: Uuid,
        operation: &str,
        status: &str,
        idempotency_key: Option<&str>,
    ) -> QefroResult<OperationRun> {
        let now = Utc::now();
        let started = if status == STATUS_RUNNING {
            Some(now)
        } else {
            None
        };
        sqlx::query(
            r#"
            INSERT INTO qefro_operation_runs (
                id, tenant_id, user_id, entity, entity_id, operation, status,
                request_id, idempotency_key, progress, started_at, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,0,$10, now(), now())
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(ctx.user_id)
        .bind(entity)
        .bind(entity_id)
        .bind(operation)
        .bind(status)
        .bind(ctx.request_id)
        .bind(idempotency_key)
        .bind(started)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(OperationRun {
            id,
            tenant_id: ctx.tenant_id,
            user_id: Some(ctx.user_id),
            entity: entity.into(),
            entity_id,
            operation: operation.into(),
            status: status.into(),
            request_id: Some(ctx.request_id),
            idempotency_key: idempotency_key.map(|s| s.to_string()),
            progress: 0,
            result: None,
            error: None,
            started_at: started,
            completed_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn mark_running_tx(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_operation_runs
            SET status = $3, started_at = COALESCE(started_at, now()), updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(STATUS_RUNNING)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn set_progress_tx(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        id: Uuid,
        progress: i32,
    ) -> QefroResult<()> {
        let progress = progress.clamp(0, 100);
        sqlx::query(
            r#"
            UPDATE qefro_operation_runs
            SET progress = $3, updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(progress)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn complete_tx(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        id: Uuid,
        result: &Value,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_operation_runs
            SET status = $3, result = $4, progress = 100, error = NULL,
                completed_at = now(), updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(STATUS_COMPLETED)
        .bind(result)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn fail_tx(
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        id: Uuid,
        error: &str,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_operation_runs
            SET status = $3, error = $4, completed_at = now(), updated_at = now()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(STATUS_FAILED)
        .bind(error)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }
}
