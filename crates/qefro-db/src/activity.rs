use chrono::{DateTime, Utc};
use qefro_core::{field_changes, strip_secrets, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::service::EntityService;

pub const TYPE_CREATED: &str = "created";
pub const TYPE_UPDATED: &str = "updated";
pub const TYPE_DELETED: &str = "deleted";
pub const TYPE_WORKFLOW: &str = "workflow_transition";
pub const TYPE_COMMENT: &str = "comment";
pub const TYPE_ASSIGNMENT: &str = "assignment";
pub const TYPE_SYSTEM: &str = "system";

/// High-volume activity older than this may be purged. Not automatic.
pub const DEFAULT_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActivityRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub activity_type: String,
    pub message: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

pub struct ActivityStore {
    pool: PgPool,
}

impl ActivityStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        ctx: &OpContext,
        entity_type: &str,
        entity_id: Uuid,
        activity_type: &str,
        message: &str,
        metadata: Value,
    ) -> QefroResult<ActivityRecord> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let row = insert_tx(
            &mut tx,
            ctx,
            entity_type,
            entity_id,
            activity_type,
            message,
            metadata,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(row)
    }

    pub async fn record_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        ctx: &OpContext,
        entity_type: &str,
        entity_id: Uuid,
        activity_type: &str,
        message: &str,
        metadata: Value,
    ) -> QefroResult<ActivityRecord> {
        insert_tx(
            tx,
            ctx,
            entity_type,
            entity_id,
            activity_type,
            message,
            metadata,
        )
        .await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
    ) -> QefroResult<Vec<ActivityRecord>> {
        let limit = limit.clamp(1, 200);
        sqlx::query_as::<_, ActivityRecord>(
            r#"
            SELECT id, tenant_id, entity_type, entity_id, actor_id, actor_name,
                   activity_type, message, metadata, created_at
            FROM qefro_activity
            WHERE tenant_id = $1 AND entity_type = $2 AND entity_id = $3
            ORDER BY created_at DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn purge_older_than(&self, days: i64) -> QefroResult<u64> {
        let days = days.max(1);
        let result = sqlx::query(
            "DELETE FROM qefro_activity WHERE created_at < now() - make_interval(days => $1::int)",
        )
        .bind(days as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

async fn insert_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &OpContext,
    entity_type: &str,
    entity_id: Uuid,
    activity_type: &str,
    message: &str,
    mut metadata: Value,
) -> QefroResult<ActivityRecord> {
    strip_secrets(None, &mut metadata);
    let id = Uuid::new_v4();
    let actor_name = ctx.activity_actor_name();
    let actor_id = if ctx.user_id.is_nil() {
        None
    } else {
        Some(ctx.user_id)
    };
    sqlx::query(
        r#"
        INSERT INTO qefro_activity (
            id, tenant_id, entity_type, entity_id, actor_id, actor_name,
            activity_type, message, metadata, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9, now())
        "#,
    )
    .bind(id)
    .bind(ctx.tenant_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(actor_id)
    .bind(&actor_name)
    .bind(activity_type)
    .bind(message)
    .bind(&metadata)
    .execute(&mut **tx)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(ActivityRecord {
        id,
        tenant_id: ctx.tenant_id,
        entity_type: entity_type.into(),
        entity_id,
        actor_id,
        actor_name: Some(actor_name),
        activity_type: activity_type.into(),
        message: message.into(),
        metadata,
        created_at: Utc::now(),
    })
}

pub fn mutation_activity(
    entity_label: &str,
    activity_type: &str,
    old: Option<&Value>,
    new: Option<&Value>,
    extra: Option<Value>,
) -> (String, Value) {
    let mut metadata = extra.unwrap_or_else(|| json!({}));
    let changes = field_changes(old, new);
    if !changes.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert("changes".into(), changes.clone());
        }
    }
    let message = match activity_type {
        TYPE_CREATED => format!("{entity_label} created"),
        TYPE_DELETED => format!("{entity_label} deleted"),
        TYPE_COMMENT => new
            .and_then(|v| v.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("Comment")
            .to_string(),
        TYPE_WORKFLOW => {
            let from = metadata.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = metadata.get("to").and_then(|v| v.as_str()).unwrap_or("");
            if from.is_empty() {
                format!("{entity_label} moved to {to}")
            } else {
                format!("{entity_label} moved {from} → {to}")
            }
        }
        TYPE_ASSIGNMENT => format!("{entity_label} assignment updated"),
        TYPE_SYSTEM => metadata
            .get("message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{entity_label} updated")),
        _ => format!("{entity_label} updated"),
    };
    (message, metadata)
}

pub fn assignment_changed(old: Option<&Value>, new: Option<&Value>) -> bool {
    let changes = field_changes(old, new);
    changes
        .as_object()
        .map(|obj| {
            obj.keys()
                .any(|k| k.contains("assign") || k == "owner" || k == "owner_id")
        })
        .unwrap_or(false)
}

impl EntityService {
    pub fn activity(&self) -> &ActivityStore {
        &self.activity
    }

    pub async fn list_activity(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        limit: i64,
    ) -> QefroResult<Vec<ActivityRecord>> {
        let entity = self.registry().get(entity_name)?;
        self.get(ctx, &entity.name, record_id).await?;
        self.activity
            .list(ctx.tenant_id, &entity.name, record_id, limit)
            .await
    }

    pub async fn add_comment(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        message: &str,
    ) -> QefroResult<ActivityRecord> {
        let entity = self.registry().get(entity_name)?;
        if !entity.comments {
            return Err(QefroError::bad_request("comments are not enabled"));
        }
        self.permissions().check(ctx, &entity.name, Action::Read)?;
        self.get(ctx, &entity.name, record_id).await?;
        let message = message.trim();
        if message.is_empty() {
            return Err(QefroError::bad_request("comment message is required"));
        }
        if message.len() > 4000 {
            return Err(QefroError::bad_request("comment is too long"));
        }
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let row = self
            .activity
            .record_tx(
                &mut tx,
                ctx,
                &entity.name,
                record_id,
                TYPE_COMMENT,
                message,
                json!({}),
            )
            .await?;
        let event = {
            let mut event = qefro_events::DomainEvent::new(
                "comment.created",
                entity.name.clone(),
                record_id,
                ctx.tenant_id,
                json!({ "message": message }),
            );
            event.user_id = Some(ctx.user_id);
            event
        };
        crate::outbox::Outbox::enqueue_tx(&mut tx, &event).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let _ = self.dispatch_outbox().await;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_message_includes_states() {
        let (msg, _) = mutation_activity(
            "Order",
            TYPE_WORKFLOW,
            None,
            None,
            Some(json!({ "from": "Preparing", "to": "Ready" })),
        );
        assert_eq!(msg, "Order moved Preparing → Ready");
    }

    #[test]
    fn secrets_are_not_in_changes() {
        let (_msg, meta) = mutation_activity(
            "User",
            TYPE_UPDATED,
            Some(&json!({ "password_hash": "a", "name": "A" })),
            Some(&json!({ "password_hash": "b", "name": "B" })),
            None,
        );
        assert!(meta["changes"].get("password_hash").is_none());
        assert_eq!(meta["changes"]["name"]["new"], "B");
    }
}
