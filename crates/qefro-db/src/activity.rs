use chrono::{DateTime, Utc};
use qefro_core::{field_changes, strip_secrets, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
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
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        apply_activity_rls(&mut tx, tenant_id, false).await?;
        let rows = sqlx::query_as::<_, ActivityRecord>(
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
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_recent(
        &self,
        tenant_id: Uuid,
        entity_type: Option<&str>,
        limit: i64,
    ) -> QefroResult<Vec<ActivityRecord>> {
        let limit = limit.clamp(1, 200);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        apply_activity_rls(&mut tx, tenant_id, false).await?;
        let rows = if let Some(entity_type) = entity_type {
            sqlx::query_as::<_, ActivityRecord>(
                r#"
                SELECT id, tenant_id, entity_type, entity_id, actor_id, actor_name,
                       activity_type, message, metadata, created_at
                FROM qefro_activity
                WHERE tenant_id = $1 AND entity_type = $2
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(tenant_id)
            .bind(entity_type)
            .bind(limit)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?
        } else {
            sqlx::query_as::<_, ActivityRecord>(
                r#"
                SELECT id, tenant_id, entity_type, entity_id, actor_id, actor_name,
                       activity_type, message, metadata, created_at
                FROM qefro_activity
                WHERE tenant_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(tenant_id)
            .bind(limit)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?
        };
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rows)
    }

    pub async fn purge_older_than(&self, days: i64) -> QefroResult<u64> {
        let days = days.max(1);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        apply_activity_rls(&mut tx, Uuid::nil(), true).await?;
        let result = sqlx::query(
            "DELETE FROM qefro_activity WHERE created_at < now() - make_interval(days => $1::int)",
        )
        .bind(days as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

async fn apply_activity_rls(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    bypass: bool,
) -> QefroResult<()> {
    sqlx::query("SELECT set_config('qefro.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    if bypass {
        sqlx::query("SELECT set_config('qefro.rls_bypass', 'on', true)")
            .execute(&mut **tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
    }
    Ok(())
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
    apply_activity_rls(tx, ctx.tenant_id, false).await?;
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

    /// Tenant-scoped recent activity with the same RowPolicy as `get`.
    /// Counts and messages never include records the caller cannot read.
    pub async fn list_recent_activity(
        &self,
        ctx: &OpContext,
        entity_name: Option<&str>,
        limit: i64,
    ) -> QefroResult<Vec<ActivityRecord>> {
        let want = limit.clamp(1, 50);
        let raw = self
            .activity
            .list_recent(ctx.tenant_id, entity_name, (want * 4).min(200))
            .await?;
        let mut out = Vec::new();
        for row in raw {
            if self.get(ctx, &row.entity_type, row.entity_id).await.is_ok() {
                out.push(row);
                if (out.len() as i64) >= want {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub async fn add_comment(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        message: &str,
    ) -> QefroResult<ActivityRecord> {
        self.add_comment_with_attachment(ctx, entity_name, record_id, message, None)
            .await
    }

    pub async fn add_comment_with_attachment(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        message: &str,
        attachment_id: Option<Uuid>,
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
        let mentions = parse_mentions(message);
        let mut metadata = json!({ "mentions": mentions });
        if let Some(attachment_id) = attachment_id {
            let store = crate::attachments::AttachmentStore::new(self.pool().clone());
            let file = store.get(ctx.tenant_id, attachment_id).await?;
            if file.entity != entity.name || file.record_id != record_id {
                return Err(QefroError::bad_request(
                    "attachment does not belong to this record",
                ));
            }
            if let Some(obj) = metadata.as_object_mut() {
                obj.insert("attachment_id".into(), json!(attachment_id));
                obj.insert("filename".into(), json!(file.filename));
            }
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
                metadata.clone(),
            )
            .await?;
        let event = {
            let mut event = qefro_events::DomainEvent::new(
                "comment.created",
                entity.name.clone(),
                record_id,
                ctx.tenant_id,
                json!({
                    "message": message,
                    "mentions": mentions,
                    "attachment_id": attachment_id,
                }),
            );
            event.user_id = Some(ctx.user_id);
            event
        };
        crate::outbox::Outbox::enqueue_tx(&mut tx, &event).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let _ = self.dispatch_outbox().await;
        if !mentions.is_empty() {
            let _ = self
                .notify_mentions(ctx, &entity.name, record_id, message, &mentions)
                .await;
        }
        Ok(row)
    }

    async fn notify_mentions(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        message: &str,
        tokens: &[String],
    ) -> QefroResult<()> {
        let users = self.resolve_mention_users(ctx, tokens).await;
        if users.is_empty() {
            return Ok(());
        }
        let store = crate::notifications::NotificationStore::new(self.pool().clone());
        let title = format!("You were mentioned on {entity_name}");
        let body: String = message.chars().take(280).collect();
        for (user_id, _) in users {
            let _ = store
                .insert(&crate::notifications::InAppNotification {
                    id: Uuid::new_v4(),
                    tenant_id: ctx.tenant_id,
                    user_id,
                    title: title.clone(),
                    body: body.clone(),
                    entity: Some(entity_name.into()),
                    record_id: Some(record_id),
                    read_at: None,
                    created_at: Utc::now(),
                })
                .await;
        }
        Ok(())
    }

    async fn resolve_mention_users(
        &self,
        ctx: &OpContext,
        tokens: &[String],
    ) -> Vec<(Uuid, String)> {
        let Some(auth) = self.identity_service() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for token in tokens {
            let Ok((users, _)) = auth
                .list_tenant_users(ctx.tenant_id, Some(token), 1, 10)
                .await
            else {
                continue;
            };
            for user in users {
                let Some(id) = user
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                else {
                    continue;
                };
                if id == ctx.user_id {
                    continue;
                }
                let name = user.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let email = user.get("email").and_then(|v| v.as_str()).unwrap_or("");
                let needle = token.to_ascii_lowercase();
                let first = name.split_whitespace().next().unwrap_or("");
                if name.eq_ignore_ascii_case(token)
                    || first.eq_ignore_ascii_case(token)
                    || email.to_ascii_lowercase().starts_with(&needle)
                {
                    if !found.iter().any(|(uid, _)| *uid == id) {
                        found.push((id, name.to_string()));
                    }
                }
            }
        }
        found
    }
}

/// `@Ahmed` tokens from a comment. Identity lookup happens separately.
pub fn parse_mentions(message: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = message.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '@' {
            continue;
        }
        let mut token = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_alphanumeric() || next == '_' || next == '.' || next == '-' {
                token.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if token.is_empty() {
            continue;
        }
        if !out
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&token))
        {
            out.push(token);
        }
    }
    out
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

    #[test]
    fn parse_mentions_collects_unique_tokens() {
        let tokens = parse_mentions("Hey @Ahmed and @sara — ping @Ahmed again");
        assert_eq!(tokens, vec!["Ahmed".to_string(), "sara".to_string()]);
        assert!(parse_mentions("no mentions here").is_empty());
    }
}
