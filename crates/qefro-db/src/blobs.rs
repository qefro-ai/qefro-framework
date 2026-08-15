use qefro_core::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMeta {
    pub key: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

pub struct BlobMetaStore {
    pool: PgPool,
}

impl BlobMetaStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        tenant_id: Uuid,
        created_by: Uuid,
        meta: &BlobMeta,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO blobs (tenant_id, key, filename, content_type, size, created_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(tenant_id)
        .bind(&meta.key)
        .bind(&meta.filename)
        .bind(&meta.content_type)
        .bind(meta.size)
        .bind(created_by)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn get(&self, tenant_id: Uuid, key: &str) -> QefroResult<BlobMeta> {
        sqlx::query_as::<_, BlobMetaRow>(
            "SELECT key, filename, content_type, size FROM blobs WHERE tenant_id = $1 AND key = $2",
        )
        .bind(tenant_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .map(Into::into)
        .ok_or_else(|| QefroError::not_found("file not found"))
    }

    pub async fn delete(&self, tenant_id: Uuid, key: &str) -> QefroResult<()> {
        sqlx::query("DELETE FROM blobs WHERE tenant_id = $1 AND key = $2")
            .bind(tenant_id)
            .bind(key)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| QefroError::database(e.to_string()))
    }
}

#[derive(sqlx::FromRow)]
struct BlobMetaRow {
    key: String,
    filename: String,
    content_type: String,
    size: i64,
}

impl From<BlobMetaRow> for BlobMeta {
    fn from(row: BlobMetaRow) -> Self {
        Self {
            key: row.key,
            filename: row.filename,
            content_type: row.content_type,
            size: row.size,
        }
    }
}
