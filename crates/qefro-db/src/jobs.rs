use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub payload: Value,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub idempotency_key: Option<String>,
}

#[async_trait]
pub trait JobHandler: Send + Sync {
    /// Jobs are denied unless they opt in. Notifications opt in; mutations do not.
    fn worker_safe(&self) -> bool {
        false
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()>;
}

#[derive(Clone, Default)]
pub struct JobRegistry {
    handlers: HashMap<String, Arc<dyn JobHandler>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn JobHandler>) {
        self.handlers.insert(name.into(), handler);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn JobHandler>> {
        self.handlers.get(name).cloned()
    }

    pub fn is_worker_safe(&self, name: &str) -> bool {
        self.handlers
            .get(name)
            .map(|h| h.worker_safe())
            .unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct JobQueue {
    pool: PgPool,
}

impl JobQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn enqueue_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &OpContext,
        name: &str,
        payload: Value,
    ) -> QefroResult<Uuid> {
        let key = payload
            .get("idempotency_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let Some(key) = key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM jobs WHERE tenant_id = $1 AND name = $2 AND idempotency_key = $3",
            )
            .bind(ctx.tenant_id)
            .bind(name)
            .bind(key)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?
            {
                return Ok(existing);
            }
        }
        let id = Uuid::new_v4();
        let run_at = payload
            .get("run_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, tenant_id, user_id, name, payload, status, attempts, max_attempts,
                run_at, created_at, updated_at, idempotency_key
            ) VALUES ($1,$2,$3,$4,$5,'pending',0,5, $7, now(), now(), $6)
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(ctx.user_id)
        .bind(name)
        .bind(payload)
        .bind(key.as_deref())
        .bind(run_at)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(id)
    }

    pub async fn enqueue(&self, ctx: &OpContext, name: &str, payload: Value) -> QefroResult<Uuid> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let id = self.enqueue_tx(&mut tx, ctx, name, payload).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(id)
    }

    async fn claim_one(&self) -> QefroResult<Option<JobRecord>> {
        let rec = sqlx::query_as::<_, JobRecord>(
            r#"
            UPDATE jobs SET status = 'running', updated_at = now()
            WHERE id = (
                SELECT id FROM jobs
                WHERE status = 'pending' AND run_at <= now()
                ORDER BY run_at ASC
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            RETURNING id, tenant_id, user_id, name, payload, status, attempts,
                      max_attempts, run_at, last_error, created_at, updated_at, idempotency_key
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rec)
    }

    pub async fn process_one(&self, registry: &JobRegistry) -> QefroResult<bool> {
        let Some(job) = self.claim_one().await? else {
            return Ok(false);
        };
        let mut ctx = OpContext::worker(job.tenant_id, job.user_id.unwrap_or(Uuid::nil()));
        ctx.request_id = job.id;
        if let Ok(Some(apps)) = sqlx::query_scalar::<_, Vec<String>>(
            "SELECT enabled_apps FROM tenant_settings WHERE tenant_id = $1",
        )
        .bind(job.tenant_id)
        .fetch_optional(&self.pool)
        .await
        {
            ctx.enabled_apps = apps;
        }
        let result = match registry.get(&job.name) {
            Some(handler) if handler.worker_safe() => handler.run(&ctx, &job.payload).await,
            Some(_) => Err(QefroError::forbidden(format!(
                "job '{}' is not registered as worker-safe",
                job.name
            ))),
            None => {
                tracing::warn!(job = %job.name, "no job handler registered");
                Err(QefroError::not_found(format!(
                    "job handler '{}' not found",
                    job.name
                )))
            }
        };
        match result {
            Ok(()) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'succeeded', updated_at = now() WHERE id = $1",
                )
                .bind(job.id)
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            }
            Err(err) => {
                let attempts = job.attempts + 1;
                let (status, run_at) = if attempts >= job.max_attempts {
                    ("failed", job.run_at)
                } else {
                    let backoff = 2i64.pow(attempts as u32).min(300);
                    ("pending", Utc::now() + Duration::seconds(backoff))
                };
                sqlx::query(
                    r#"
                    UPDATE jobs
                    SET status = $2, attempts = $3, run_at = $4, last_error = $5, updated_at = now()
                    WHERE id = $1
                    "#,
                )
                .bind(job.id)
                .bind(status)
                .bind(attempts)
                .bind(run_at)
                .bind(err.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            }
        }
        Ok(true)
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<JobRecord> {
        sqlx::query_as::<_, JobRecord>(
            r#"
            SELECT id, tenant_id, user_id, name, payload, status, attempts,
                   max_attempts, run_at, last_error, created_at, updated_at, idempotency_key
            FROM jobs WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("job not found"))
    }

    pub fn to_client_json(job: &JobRecord) -> Value {
        let status = match job.status.as_str() {
            "pending" if job.attempts > 0 => "retrying",
            "pending" => "queued",
            "succeeded" => "completed",
            other => other,
        };
        json!({
            "id": job.id,
            "tenant_id": job.tenant_id,
            "user_id": job.user_id,
            "name": job.name,
            "status": job.status,
            "status_alias": status,
            "queued": job.status == "pending" && job.attempts == 0,
            "completed": job.status == "succeeded",
            "attempts": job.attempts,
            "max_attempts": job.max_attempts,
            "run_at": job.run_at,
            "last_error": job.last_error,
            "created_at": job.created_at,
            "updated_at": job.updated_at,
            "idempotency_key": job.idempotency_key,
        })
    }

    /// After a crash, running jobs were claimed but not finished. Return them to pending.
    pub async fn reclaim_running(&self) -> QefroResult<u64> {
        let res = sqlx::query(
            "UPDATE jobs SET status = 'pending', updated_at = now() WHERE status = 'running'",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(res.rows_affected())
    }

    pub async fn pending_count(&self) -> QefroResult<i64> {
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE status IN ('pending', 'running')")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
    }
}

/// Logs a tenant-aware notification. Applications replace this with email/SMS.
pub struct LogNotificationJob;

#[async_trait]
impl JobHandler for LogNotificationJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        tracing::info!(
            tenant_id = %ctx.tenant_id,
            job_payload_keys = payload.as_object().map(|o| o.len()).unwrap_or(0),
            "notification job"
        );
        Ok(())
    }
}
