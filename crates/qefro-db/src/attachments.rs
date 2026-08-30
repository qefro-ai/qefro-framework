use qefro_core::{BlobStore, QefroError, QefroResult};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::service::EntityService;

const MAX_SIZE: i64 = 10 * 1024 * 1024;
const ALLOWED_MIME: &[&str] = &[
    "application/pdf",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "text/plain",
    "text/csv",
    "application/json",
];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity: String,
    pub record_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    #[serde(skip_serializing)]
    pub storage_key: String,
    pub uploaded_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct AttachmentStore {
    pool: PgPool,
}

impl AttachmentStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        entity: &str,
        record_id: Uuid,
    ) -> QefroResult<Vec<Attachment>> {
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT id, tenant_id, entity, record_id, filename, mime_type, size,
                   storage_key, uploaded_by, created_at
            FROM qefro_attachments
            WHERE tenant_id = $1 AND entity = $2 AND record_id = $3
            ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(record_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<Attachment> {
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT id, tenant_id, entity, record_id, filename, mime_type, size,
                   storage_key, uploaded_by, created_at
            FROM qefro_attachments WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("attachment not found"))
    }

    pub async fn insert(&self, row: &Attachment) -> QefroResult<()> {
        sqlx::query(
            r#"
            INSERT INTO qefro_attachments (
                id, tenant_id, entity, record_id, filename, mime_type, size,
                storage_key, uploaded_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(row.id)
        .bind(row.tenant_id)
        .bind(&row.entity)
        .bind(row.record_id)
        .bind(&row.filename)
        .bind(&row.mime_type)
        .bind(row.size)
        .bind(&row.storage_key)
        .bind(row.uploaded_by)
        .bind(row.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<()> {
        sqlx::query("DELETE FROM qefro_attachments WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }
}

pub fn validate_upload(filename: &str, mime: &str, size: i64) -> QefroResult<()> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(QefroError::bad_request("invalid filename"));
    }
    if size <= 0 || size > MAX_SIZE {
        return Err(QefroError::bad_request("file exceeds size limit"));
    }
    if !ALLOWED_MIME.iter().any(|m| *m == mime) && !mime.starts_with("image/") {
        return Err(QefroError::bad_request(format!(
            "mime type '{mime}' is not allowed"
        )));
    }
    Ok(())
}

pub fn storage_key(entity: &str, record_id: Uuid, id: Uuid, filename: &str) -> QefroResult<String> {
    if filename.contains("..") || filename.contains('/') {
        return Err(QefroError::bad_request("invalid storage key"));
    }
    Ok(format!("attachments/{entity}/{record_id}/{id}_{filename}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_oversize() {
        assert!(validate_upload("../etc/passwd", "text/plain", 12).is_err());
        assert!(validate_upload("a/b.pdf", "application/pdf", 12).is_err());
        assert!(validate_upload("ok.pdf", "application/pdf", 12).is_ok());
        assert!(validate_upload("ok.pdf", "application/pdf", MAX_SIZE + 1).is_err());
        assert!(validate_upload("ok.exe", "application/x-msdownload", 12).is_err());
        assert!(storage_key("Invoice", Uuid::nil(), Uuid::nil(), "../x").is_err());
    }
}

impl EntityService {
    pub async fn list_attachments(
        &self,
        ctx: &qefro_core::OpContext,
        entity_name: &str,
        record_id: Uuid,
        store: &AttachmentStore,
    ) -> QefroResult<Vec<Attachment>> {
        let entity = self.registry().get(entity_name)?;
        self.get(ctx, &entity.name, record_id).await?;
        store.list(ctx.tenant_id, &entity.name, record_id).await
    }

    pub async fn list_record_attachments(
        &self,
        ctx: &qefro_core::OpContext,
        entity_name: &str,
        record_id: Uuid,
    ) -> QefroResult<Vec<Attachment>> {
        let store = AttachmentStore::new(self.pool().clone());
        self.list_attachments(ctx, entity_name, record_id, &store)
            .await
    }

    pub async fn create_attachment(
        &self,
        ctx: &qefro_core::OpContext,
        entity_name: &str,
        record_id: Uuid,
        filename: &str,
        mime: &str,
        bytes: &[u8],
        blobs: &dyn BlobStore,
        store: &AttachmentStore,
    ) -> QefroResult<Attachment> {
        let entity = self.registry().get(entity_name)?;
        if !entity.attachments && !entity.standalone {
            return Err(QefroError::bad_request("attachments are not enabled"));
        }
        self.permissions()
            .check(ctx, &entity.name, Action::Update)?;
        self.get(ctx, &entity.name, record_id).await?;
        validate_upload(filename, mime, bytes.len() as i64)?;
        let id = Uuid::new_v4();
        let key = storage_key(&entity.name, record_id, id, filename)?;
        blobs.put(ctx.tenant_id, &key, bytes)?;
        let row = Attachment {
            id,
            tenant_id: ctx.tenant_id,
            entity: entity.name.clone(),
            record_id,
            filename: filename.to_string(),
            mime_type: mime.to_string(),
            size: bytes.len() as i64,
            storage_key: key,
            uploaded_by: Some(ctx.user_id),
            created_at: chrono::Utc::now(),
        };
        store.insert(&row).await?;
        if entity.activity {
            let (message, metadata) = crate::activity::mutation_activity(
                &entity.label,
                crate::activity::TYPE_SYSTEM,
                None,
                None,
                Some(json!({ "message": format!("{} attached", filename), "filename": filename })),
            );
            let _ = self
                .activity
                .record(
                    ctx,
                    &entity.name,
                    record_id,
                    crate::activity::TYPE_SYSTEM,
                    &message,
                    metadata,
                )
                .await;
        }
        let mut event = qefro_events::DomainEvent::new(
            "attachment.created",
            entity.name.clone(),
            record_id,
            ctx.tenant_id,
            json!({ "filename": filename, "attachment_id": id }),
        );
        event.user_id = Some(ctx.user_id);
        self.outbox().enqueue(&event).await?;
        let _ = self.dispatch_outbox().await;
        Ok(row)
    }

    pub async fn get_attachment(
        &self,
        ctx: &qefro_core::OpContext,
        id: Uuid,
        store: &AttachmentStore,
        blobs: &dyn BlobStore,
    ) -> QefroResult<(Attachment, Vec<u8>)> {
        let meta = store.get(ctx.tenant_id, id).await?;
        self.get(ctx, &meta.entity, meta.record_id).await?;
        let bytes = blobs.get(ctx.tenant_id, &meta.storage_key)?;
        Ok((meta, bytes))
    }

    pub async fn delete_attachment(
        &self,
        ctx: &qefro_core::OpContext,
        id: Uuid,
        store: &AttachmentStore,
        blobs: &dyn BlobStore,
    ) -> QefroResult<()> {
        let meta = store.get(ctx.tenant_id, id).await?;
        self.permissions()
            .check(ctx, &meta.entity, Action::Update)?;
        self.get(ctx, &meta.entity, meta.record_id).await?;
        blobs.delete(ctx.tenant_id, &meta.storage_key)?;
        store.delete(ctx.tenant_id, id).await
    }
}
