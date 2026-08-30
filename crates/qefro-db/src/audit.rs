use chrono::{DateTime, Utc};
use qefro_core::{field_changes, strip_secrets, OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    #[sqlx(default)]
    pub actor_name: Option<String>,
}

impl AuditRecord {
    pub fn to_client_json(&self) -> Value {
        let mut old = self.old_values.clone();
        let mut new = self.new_values.clone();
        if let Some(v) = old.as_mut() {
            strip_secrets(None, v);
        }
        if let Some(v) = new.as_mut() {
            strip_secrets(None, v);
        }
        json!({
            "id": self.id,
            "tenant_id": self.tenant_id,
            "user_id": self.user_id,
            "actor": self.actor_name,
            "entity": self.entity,
            "entity_id": self.entity_id,
            "action": self.action,
            "operation": self.action,
            "changes": field_changes(old.as_ref(), new.as_ref()),
            "old_values": old,
            "new_values": new,
            "request_id": self.request_id,
            "ip": self.ip,
            "user_agent": self.user_agent,
            "created_at": self.created_at,
        })
    }
}

fn sanitize_opt(
    entity_hint: Option<&qefro_core::EntityDef>,
    value: Option<&Value>,
) -> Option<Value> {
    value.map(|v| {
        let mut cloned = v.clone();
        strip_secrets(entity_hint, &mut cloned);
        cloned
    })
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
        let old = sanitize_opt(None, old_values);
        let new = sanitize_opt(None, new_values);
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
        .bind(old)
        .bind(new)
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
        let old = sanitize_opt(None, old_values);
        let new = sanitize_opt(None, new_values);
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
        .bind(old)
        .bind(new)
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
            SELECT a.id, a.tenant_id, a.user_id, a.entity, a.entity_id, a.action,
                   a.old_values, a.new_values, a.request_id, a.ip, a.user_agent, a.created_at,
                   u.name as actor_name
            FROM audit_logs a
            LEFT JOIN users u ON u.id = a.user_id
            WHERE a.tenant_id = $1
              AND ($2::text IS NULL OR a.entity = $2)
              AND ($3::uuid IS NULL OR a.entity_id = $3)
            ORDER BY a.created_at DESC
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

    pub async fn purge_older_than(&self, days: i64) -> QefroResult<u64> {
        let days = days.max(30);
        let result = sqlx::query(
            "DELETE FROM audit_logs WHERE created_at < now() - make_interval(days => $1::int)",
        )
        .bind(days as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_json_strips_secrets_and_exposes_changes() {
        let rec = AuditRecord {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            user_id: None,
            entity: "User".into(),
            entity_id: None,
            action: "update".into(),
            old_values: Some(json!({ "status": "Lead", "password_hash": "old" })),
            new_values: Some(json!({ "status": "Qualified", "password_hash": "new" })),
            request_id: None,
            ip: None,
            user_agent: None,
            created_at: Utc::now(),
            actor_name: Some("Ahmed".into()),
        };
        let json = rec.to_client_json();
        assert_eq!(json["actor"], "Ahmed");
        assert_eq!(json["changes"]["status"]["old"], "Lead");
        assert!(json["old_values"].get("password_hash").is_none());
        assert!(json["new_values"].get("password_hash").is_none());
    }
}
