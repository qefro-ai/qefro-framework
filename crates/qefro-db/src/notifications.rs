use async_trait::async_trait;
use qefro_core::{NotificationDef, OpContext, QefroError, QefroResult};
use qefro_events::{DomainEvent, EventHandler};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::jobs::JobQueue;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InAppNotification {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
    pub entity: Option<String>,
    pub record_id: Option<Uuid>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct NotificationStore {
    pool: PgPool,
}

impl NotificationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        unread_only: bool,
    ) -> QefroResult<Vec<InAppNotification>> {
        let sql = if unread_only {
            r#"
            SELECT id, tenant_id, user_id, title, body, entity, record_id, read_at, created_at
            FROM qefro_notifications
            WHERE tenant_id = $1 AND user_id = $2 AND read_at IS NULL
            ORDER BY created_at DESC LIMIT 100
            "#
        } else {
            r#"
            SELECT id, tenant_id, user_id, title, body, entity, record_id, read_at, created_at
            FROM qefro_notifications
            WHERE tenant_id = $1 AND user_id = $2
            ORDER BY created_at DESC LIMIT 100
            "#
        };
        sqlx::query_as::<_, InAppNotification>(sql)
            .bind(tenant_id)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn insert(&self, row: &InAppNotification) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO qefro_notifications (
                id, tenant_id, user_id, title, body, entity, record_id, read_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(row.id)
        .bind(row.tenant_id)
        .bind(row.user_id)
        .bind(&row.title)
        .bind(&row.body)
        .bind(&row.entity)
        .bind(row.record_id)
        .bind(row.read_at)
        .bind(row.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn mark_read(&self, tenant_id: Uuid, user_id: Uuid, id: Uuid) -> QefroResult<()> {
        sqlx::query(
            "UPDATE qefro_notifications SET read_at = now() WHERE id = $1 AND tenant_id = $2 AND user_id = $3",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn unread_count(&self, tenant_id: Uuid, user_id: Uuid) -> QefroResult<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM qefro_notifications WHERE tenant_id = $1 AND user_id = $2 AND read_at IS NULL",
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }
}

pub struct PlatformDispatcher {
    pool: PgPool,
    jobs: Arc<JobQueue>,
    notifications: Vec<NotificationDef>,
    webhooks: Vec<qefro_core::WebhookDef>,
    store: NotificationStore,
}

impl PlatformDispatcher {
    pub fn new(
        pool: PgPool,
        jobs: Arc<JobQueue>,
        notifications: Vec<NotificationDef>,
        webhooks: Vec<qefro_core::WebhookDef>,
    ) -> Self {
        Self {
            store: NotificationStore::new(pool.clone()),
            pool,
            jobs,
            notifications,
            webhooks,
        }
    }
}

#[async_trait]
impl EventHandler for PlatformDispatcher {
    async fn handle(&self, event: &DomainEvent) -> QefroResult<()> {
        for def in &self.notifications {
            if def.event != event.name && def.event != "*" {
                continue;
            }
            let users = recipient_users(&self.pool, event, def).await?;
            let title = def
                .title
                .clone()
                .unwrap_or_else(|| event.name.replace('.', " "));
            let body = def.body.clone().unwrap_or_default();
            for user_id in users {
                if def.channels.iter().any(|c| c == "in_app") {
                    let _ = self
                        .store
                        .insert(&InAppNotification {
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
                        .await;
                }
                if def.channels.iter().any(|c| c == "email") {
                    let ctx = OpContext::worker(event.tenant_id, user_id);
                    let _ = self
                        .jobs
                        .enqueue(
                            &ctx,
                            "notify.email",
                            json!({
                                "user_id": user_id,
                                "title": title,
                                "body": body,
                                "event": event.name,
                                "entity": event.entity,
                                "record_id": event.entity_id,
                            }),
                        )
                        .await;
                }
            }
        }
        for hook in &self.webhooks {
            if !hook.enabled || (hook.event != event.name && hook.event != "*") {
                continue;
            }
            let ctx = OpContext::worker(event.tenant_id, event.user_id.unwrap_or(Uuid::nil()));
            let key = format!("{}:{}", hook.name, event.id);
            let _ = self
                .jobs
                .enqueue(
                    &ctx,
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
                    }),
                )
                .await;
        }
        Ok(())
    }
}

async fn recipient_users(
    pool: &PgPool,
    event: &DomainEvent,
    def: &NotificationDef,
) -> QefroResult<Vec<Uuid>> {
    let mut users = Vec::new();
    let rows: Vec<(Uuid, Vec<String>)> = sqlx::query_as(
        "SELECT user_id, roles FROM user_tenants WHERE tenant_id = $1",
    )
    .bind(event.tenant_id)
    .fetch_all(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    for (user_id, roles) in rows {
        let match_role = def.recipients.iter().any(|r| {
            roles.iter().any(|have| have.eq_ignore_ascii_case(r))
        });
        if match_role {
            users.push(user_id);
        }
    }
    if def
        .recipients
        .iter()
        .any(|r| r.eq_ignore_ascii_case("owner") || r.eq_ignore_ascii_case("creator"))
    {
        if let Some(owner) = event
            .payload
            .get("created_by")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            if !users.contains(&owner) {
                users.push(owner);
            }
        }
    }
    Ok(users)
}

/// Logs email. Replace with SMTP by registering another `notify.email` handler.
pub struct EmailNotifyJob;

#[async_trait]
impl crate::jobs::JobHandler for EmailNotifyJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &serde_json::Value) -> QefroResult<()> {
        tracing::info!(
            tenant_id = %ctx.tenant_id,
            title = payload.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "email notification"
        );
        Ok(())
    }
}
