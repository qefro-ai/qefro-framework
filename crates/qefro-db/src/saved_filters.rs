use qefro_core::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedFilter {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub entity: String,
    pub name: String,
    pub query: Value,
}

pub struct SavedFilterStore {
    pool: PgPool,
}

impl SavedFilterStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        entity: &str,
    ) -> QefroResult<Vec<SavedFilter>> {
        sqlx::query_as::<_, SavedFilterRow>(
            r#"
            SELECT id, tenant_id, user_id, entity, name, query
            FROM saved_filters
            WHERE tenant_id = $1 AND user_id = $2 AND entity = $3
            ORDER BY name
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(entity)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        entity: &str,
        name: &str,
        query: Value,
    ) -> QefroResult<SavedFilter> {
        let id = Uuid::new_v4();
        sqlx::query_as::<_, SavedFilterRow>(
            r#"
            INSERT INTO saved_filters (id, tenant_id, user_id, entity, name, query)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, tenant_id, user_id, entity, name, query
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(entity)
        .bind(name)
        .bind(query)
        .fetch_one(&self.pool)
        .await
        .map(Into::into)
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn delete(&self, tenant_id: Uuid, user_id: Uuid, id: Uuid) -> QefroResult<()> {
        let result = sqlx::query(
            "DELETE FROM saved_filters WHERE id = $1 AND tenant_id = $2 AND user_id = $3",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        if result.rows_affected() == 0 {
            return Err(QefroError::not_found("saved filter not found"));
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct SavedFilterRow {
    id: Uuid,
    tenant_id: Uuid,
    user_id: Uuid,
    entity: String,
    name: String,
    query: Value,
}

impl From<SavedFilterRow> for SavedFilter {
    fn from(row: SavedFilterRow) -> Self {
        Self {
            id: row.id,
            tenant_id: row.tenant_id,
            user_id: row.user_id,
            entity: row.entity,
            name: row.name,
            query: row.query,
        }
    }
}
