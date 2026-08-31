//! Generic entity attachments: metadata in PostgreSQL, bytes in BlobStore.
//!
//! One table (`qefro_attachments`) for every EntityDef that opted in with
//! `.attachments()`. There is no per-entity attachment service.

use async_trait::async_trait;
use qefro_core::{BlobStore, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use crate::activity::TYPE_SYSTEM;
use crate::jobs::JobHandler;
use crate::service::EntityService;

pub const ATTACHMENT_PURGE_JOB: &str = "attachment.purge";
pub const DEFAULT_MAX_UPLOAD_BYTES: i64 = 10 * 1024 * 1024;
const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 200;
const MAX_FILENAME_CHARS: usize = 200;

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
    #[sqlx(default)]
    pub description: Option<String>,
    #[sqlx(default)]
    pub uploaded_by_name: Option<String>,
}

impl Attachment {
    pub fn to_client_json(&self) -> Value {
        json!({
            "id": self.id,
            "tenant_id": self.tenant_id,
            "entity": self.entity,
            "record_id": self.record_id,
            "filename": self.filename,
            "description": self.description,
            "mime_type": self.mime_type,
            "content_type": self.mime_type,
            "size": self.size,
            "uploaded_by": self.uploaded_by,
            "uploaded_by_name": self.uploaded_by_name,
            "created_at": self.created_at,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentPage {
    pub items: Vec<Attachment>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

impl AttachmentPage {
    pub fn to_client_json(&self) -> Value {
        json!({
            "items": self.items.iter().map(Attachment::to_client_json).collect::<Vec<_>>(),
            "total": self.total,
            "page": self.page,
            "page_size": self.page_size,
        })
    }
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
        Ok(self
            .list_page(tenant_id, entity, record_id, 1, DEFAULT_PAGE_SIZE)
            .await?
            .items)
    }

    pub async fn list_page(
        &self,
        tenant_id: Uuid,
        entity: &str,
        record_id: Uuid,
        page: i64,
        page_size: i64,
    ) -> QefroResult<AttachmentPage> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
        let offset = (page - 1) * page_size;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM qefro_attachments
            WHERE tenant_id = $1 AND entity = $2 AND record_id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(record_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let items = sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.id, a.tenant_id, a.entity, a.record_id, a.filename, a.mime_type, a.size,
                   a.storage_key, a.uploaded_by, a.created_at, a.description, u.name AS uploaded_by_name
            FROM qefro_attachments a
            LEFT JOIN users u ON u.id = a.uploaded_by
            WHERE a.tenant_id = $1 AND a.entity = $2 AND a.record_id = $3
            ORDER BY a.created_at DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(record_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(AttachmentPage {
            items,
            total,
            page,
            page_size,
        })
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> QefroResult<Attachment> {
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.id, a.tenant_id, a.entity, a.record_id, a.filename, a.mime_type, a.size,
                   a.storage_key, a.uploaded_by, a.created_at, a.description, u.name AS uploaded_by_name
            FROM qefro_attachments a
            LEFT JOIN users u ON u.id = a.uploaded_by
            WHERE a.id = $1 AND a.tenant_id = $2
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
                storage_key, uploaded_by, created_at, description
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
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
        .bind(&row.description)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn update_meta(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        filename: &str,
        description: Option<&str>,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_attachments
            SET filename = $3, description = $4
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(filename)
        .bind(description)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }

    pub async fn replace_blob(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        filename: &str,
        mime_type: &str,
        size: i64,
        storage_key: &str,
    ) -> QefroResult<()> {
        sqlx::query(
            r#"
            UPDATE qefro_attachments
            SET filename = $3, mime_type = $4, size = $5, storage_key = $6
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(filename)
        .bind(mime_type)
        .bind(size)
        .bind(storage_key)
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

    pub async fn counts_for_records(
        &self,
        tenant_id: Uuid,
        entity: &str,
        record_ids: &[Uuid],
    ) -> QefroResult<Vec<(Uuid, i64)>> {
        if record_ids.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_as::<_, (Uuid, i64)>(
            r#"
            SELECT record_id, COUNT(*)::bigint
            FROM qefro_attachments
            WHERE tenant_id = $1 AND entity = $2 AND record_id = ANY($3)
            GROUP BY record_id
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .bind(record_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn search(
        &self,
        tenant_id: Uuid,
        q: &str,
        limit: i64,
    ) -> QefroResult<Vec<Attachment>> {
        let pattern = format!("%{q}%");
        sqlx::query_as::<_, Attachment>(
            r#"
            SELECT a.id, a.tenant_id, a.entity, a.record_id, a.filename, a.mime_type, a.size,
                   a.storage_key, a.uploaded_by, a.created_at, a.description, u.name AS uploaded_by_name
            FROM qefro_attachments a
            LEFT JOIN users u ON u.id = a.uploaded_by
            WHERE a.tenant_id = $1
              AND (a.filename ILIKE $2 OR COALESCE(a.description, '') ILIKE $2)
            ORDER BY a.created_at DESC
            LIMIT $3
            "#,
        )
        .bind(tenant_id)
        .bind(pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }
}

pub fn max_upload_bytes() -> i64 {
    std::env::var("QEFRO_MAX_UPLOAD_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_UPLOAD_BYTES)
}

pub fn sanitize_filename(name: &str) -> QefroResult<String> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(QefroError::bad_request("invalid filename"));
    }
    let base = name.trim().trim_start_matches('.');
    if base.is_empty() || base.contains("..") {
        return Err(QefroError::bad_request("invalid filename"));
    }
    let cleaned: String = base
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(*c, '.' | '-' | '_' | ' ' | '(' | ')' | ',')
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned.contains("..") {
        return Err(QefroError::bad_request("invalid filename"));
    }
    if cleaned.chars().count() > MAX_FILENAME_CHARS {
        return Err(QefroError::bad_request("filename is too long"));
    }
    Ok(cleaned)
}

pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 5 && bytes.starts_with(b"%PDF") {
        return Some("application/pdf");
    }
    if bytes.len() >= 8 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    let trimmed = bytes
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .copied()
        .collect::<Vec<_>>();
    if trimmed.first() == Some(&b'{') || trimmed.first() == Some(&b'[') {
        if std::str::from_utf8(bytes).is_ok() {
            return Some("application/json");
        }
    }
    None
}

fn mime_from_ext(filename: &str) -> Option<&'static str> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "txt" => Some("text/plain"),
        "csv" => Some("text/csv"),
        "json" => Some("application/json"),
        _ => None,
    }
}

fn mime_compatible(claimed: &str, sniffed: &str) -> bool {
    if claimed == sniffed {
        return true;
    }
    matches!(
        (claimed, sniffed),
        ("image/jpg", "image/jpeg") | ("image/pjpeg", "image/jpeg")
    )
}

fn is_allowed_mime(mime: &str) -> bool {
    ALLOWED_MIME.iter().any(|m| *m == mime) || mime.starts_with("image/")
}

pub fn resolve_mime(filename: &str, claimed: &str, bytes: &[u8]) -> QefroResult<String> {
    let claimed = claimed
        .split(';')
        .next()
        .unwrap_or(claimed)
        .trim()
        .to_ascii_lowercase();
    if let Some(sniffed) = sniff_mime(bytes) {
        if !claimed.is_empty()
            && claimed != "application/octet-stream"
            && !mime_compatible(&claimed, sniffed)
        {
            return Err(QefroError::bad_request(
                "content type does not match file contents",
            ));
        }
        if !is_allowed_mime(sniffed) {
            return Err(QefroError::bad_request(format!(
                "mime type '{sniffed}' is not allowed"
            )));
        }
        return Ok(sniffed.to_string());
    }
    let fallback = if is_allowed_mime(&claimed) {
        claimed
    } else {
        mime_from_ext(filename)
            .unwrap_or("application/octet-stream")
            .to_string()
    };
    if fallback == "text/plain" || fallback == "text/csv" {
        if bytes.contains(&0) {
            return Err(QefroError::bad_request("binary content is not a text file"));
        }
        if std::str::from_utf8(bytes).is_err() {
            return Err(QefroError::bad_request("text files must be valid UTF-8"));
        }
    }
    if !is_allowed_mime(&fallback) {
        return Err(QefroError::bad_request(format!(
            "mime type '{fallback}' is not allowed"
        )));
    }
    Ok(fallback)
}

pub fn validate_upload(filename: &str, mime: &str, size: i64) -> QefroResult<()> {
    sanitize_filename(filename)?;
    if size <= 0 || size > max_upload_bytes() {
        return Err(QefroError::bad_request("file exceeds size limit"));
    }
    if !is_allowed_mime(mime) {
        return Err(QefroError::bad_request(format!(
            "mime type '{mime}' is not allowed"
        )));
    }
    Ok(())
}

pub fn storage_key(entity: &str, record_id: Uuid, id: Uuid, filename: &str) -> QefroResult<String> {
    let filename = sanitize_filename(filename)?;
    if filename.contains("..") || filename.contains('/') {
        return Err(QefroError::bad_request("invalid storage key"));
    }
    Ok(format!("attachments/{entity}/{record_id}/{id}_{filename}"))
}

fn client_safe_error(err: QefroError) -> QefroError {
    match err {
        QefroError::BadRequest { .. }
        | QefroError::NotFound { .. }
        | QefroError::Forbidden { .. }
        | QefroError::Unauthorized { .. }
        | QefroError::Conflict { .. }
        | QefroError::Validation { .. } => err,
        _ => QefroError::internal("file storage is unavailable"),
    }
}

/// Retry blob deletion when the metadata row is already gone.
pub struct AttachmentPurgeJob {
    blobs: OnceLock<Arc<dyn BlobStore>>,
}

impl AttachmentPurgeJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            blobs: OnceLock::new(),
        })
    }

    pub fn bind(&self, blobs: Arc<dyn BlobStore>) {
        let _ = self.blobs.set(blobs);
    }
}

#[async_trait]
impl JobHandler for AttachmentPurgeJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(blobs) = self.blobs.get() else {
            return Err(QefroError::internal("attachment purge job is not bound"));
        };
        let key = payload
            .get("storage_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("storage_key is required"))?;
        let tenant = payload
            .get("tenant_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or(ctx.tenant_id);
        blobs.delete(tenant, key).map_err(client_safe_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_oversize() {
        assert!(validate_upload("../etc/passwd", "text/plain", 12).is_err());
        assert!(validate_upload("a/b.pdf", "application/pdf", 12).is_err());
        assert!(validate_upload("ok.pdf", "application/pdf", 12).is_ok());
        assert!(validate_upload("ok.pdf", "application/pdf", max_upload_bytes() + 1).is_err());
        assert!(validate_upload("ok.exe", "application/x-msdownload", 12).is_err());
        assert!(storage_key("Invoice", Uuid::nil(), Uuid::nil(), "../x").is_err());
        assert_eq!(sanitize_filename("Invoice.pdf").unwrap(), "Invoice.pdf");
        assert!(sanitize_filename("../../secret").is_err());
    }

    #[test]
    fn sniffs_magic_and_rejects_spoofed_mime() {
        assert_eq!(sniff_mime(b"%PDF-1.4 test"), Some("application/pdf"));
        assert_eq!(
            resolve_mime("x.pdf", "application/pdf", b"%PDF-1.4 test").unwrap(),
            "application/pdf"
        );
        assert!(resolve_mime("x.png", "image/png", b"%PDF-1.4 test").is_err());
        assert!(resolve_mime("ok.txt", "text/plain", b"hello").is_ok());
        assert!(resolve_mime("ok.txt", "text/plain", b"\0\0binary").is_err());
    }

    #[test]
    fn client_json_omits_storage_key() {
        let row = Attachment {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            entity: "Order".into(),
            record_id: Uuid::nil(),
            filename: "Invoice.pdf".into(),
            mime_type: "application/pdf".into(),
            size: 12,
            storage_key: "attachments/secret".into(),
            uploaded_by: None,
            created_at: chrono::Utc::now(),
            description: Some("invoice".into()),
            uploaded_by_name: Some("Ahmed".into()),
        };
        let json = row.to_client_json();
        assert!(json.get("storage_key").is_none());
        assert_eq!(json["filename"], "Invoice.pdf");
        assert_eq!(json["content_type"], "application/pdf");
    }
}

impl EntityService {
    pub async fn list_attachments(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        store: &AttachmentStore,
    ) -> QefroResult<Vec<Attachment>> {
        Ok(self
            .list_attachments_page(ctx, entity_name, record_id, store, 1, DEFAULT_PAGE_SIZE)
            .await?
            .items)
    }

    pub async fn list_attachments_page(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        store: &AttachmentStore,
        page: i64,
        page_size: i64,
    ) -> QefroResult<AttachmentPage> {
        let entity = self.registry().get(entity_name)?;
        self.require_attachments(&entity)?;
        self.get(ctx, &entity.name, record_id).await?;
        store
            .list_page(ctx.tenant_id, &entity.name, record_id, page, page_size)
            .await
    }

    pub async fn list_record_attachments(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
    ) -> QefroResult<Vec<Attachment>> {
        let store = AttachmentStore::new(self.pool().clone());
        self.list_attachments(ctx, entity_name, record_id, &store)
            .await
    }

    fn require_attachments(&self, entity: &qefro_core::EntityDef) -> QefroResult<()> {
        if !entity.attachments {
            return Err(QefroError::bad_request("attachments are not enabled"));
        }
        Ok(())
    }

    async fn enqueue_blob_purge(&self, ctx: &OpContext, storage_key: &str, attachment_id: Uuid) {
        let payload = json!({
            "storage_key": storage_key,
            "tenant_id": ctx.tenant_id,
            "attachment_id": attachment_id,
            "idempotency_key": format!("purge:{attachment_id}:{storage_key}"),
        });
        if let Err(err) = self
            .job_queue()
            .enqueue(ctx, ATTACHMENT_PURGE_JOB, payload)
            .await
        {
            tracing::error!(error = %err, attachment_id = %attachment_id, "failed to enqueue attachment purge");
        }
    }

    fn put_blob(
        &self,
        blobs: &dyn BlobStore,
        tenant_id: Uuid,
        key: &str,
        bytes: &[u8],
    ) -> QefroResult<()> {
        blobs
            .put(tenant_id, key, bytes)
            .map_err(client_safe_error)?;
        Ok(())
    }

    async fn record_file_side_effects(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record_id: Uuid,
        action: &str,
        event_name: &str,
        compat_event: Option<&str>,
        activity_message: &str,
        metadata: Value,
        old_values: Option<&Value>,
        new_values: Option<&Value>,
    ) -> QefroResult<()> {
        let mut metadata = metadata;
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert("request_id".into(), json!(ctx.request_id));
        }
        if entity.activity {
            let _ = self
                .activity
                .record(
                    ctx,
                    &entity.name,
                    record_id,
                    TYPE_SYSTEM,
                    activity_message,
                    metadata.clone(),
                )
                .await;
        }
        if entity.audit {
            let _ = self
                .audit
                .record(
                    ctx,
                    &entity.name,
                    Some(record_id),
                    action,
                    old_values,
                    new_values,
                )
                .await;
        }
        let mut event = qefro_events::DomainEvent::new(
            event_name,
            entity.name.clone(),
            record_id,
            ctx.tenant_id,
            metadata.clone(),
        );
        event.user_id = Some(ctx.user_id);
        self.outbox().enqueue(&event).await?;
        if let Some(alias) = compat_event {
            let mut compat = qefro_events::DomainEvent::new(
                alias,
                entity.name.clone(),
                record_id,
                ctx.tenant_id,
                metadata,
            );
            compat.user_id = Some(ctx.user_id);
            self.outbox().enqueue(&compat).await?;
        }
        let _ = self.dispatch_outbox().await;
        Ok(())
    }

    pub async fn create_attachment(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        filename: &str,
        mime: &str,
        bytes: &[u8],
        blobs: &dyn BlobStore,
        store: &AttachmentStore,
    ) -> QefroResult<Attachment> {
        self.create_attachment_with_description(
            ctx,
            entity_name,
            record_id,
            filename,
            mime,
            None,
            bytes,
            blobs,
            store,
        )
        .await
    }

    pub async fn create_attachment_with_description(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        record_id: Uuid,
        filename: &str,
        mime: &str,
        description: Option<&str>,
        bytes: &[u8],
        blobs: &dyn BlobStore,
        store: &AttachmentStore,
    ) -> QefroResult<Attachment> {
        let entity = self.registry().get(entity_name)?;
        self.require_attachments(&entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Update)?;
        self.get(ctx, &entity.name, record_id).await?;
        let filename = sanitize_filename(filename)?;
        let mime = resolve_mime(&filename, mime, bytes)?;
        validate_upload(&filename, &mime, bytes.len() as i64)?;
        let description = description
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(500).collect::<String>());
        let id = Uuid::new_v4();
        let key = storage_key(&entity.name, record_id, id, &filename)?;
        self.put_blob(blobs, ctx.tenant_id, &key, bytes)?;
        let row = Attachment {
            id,
            tenant_id: ctx.tenant_id,
            entity: entity.name.clone(),
            record_id,
            filename: filename.clone(),
            mime_type: mime,
            size: bytes.len() as i64,
            storage_key: key.clone(),
            uploaded_by: Some(ctx.user_id),
            created_at: chrono::Utc::now(),
            description,
            uploaded_by_name: Some(ctx.activity_actor_name()),
        };
        if let Err(err) = store.insert(&row).await {
            self.enqueue_blob_purge(ctx, &key, id).await;
            return Err(err);
        }
        let meta = json!({
            "message": format!("{} attached", filename),
            "filename": filename,
            "attachment_id": id,
            "size": row.size,
            "mime_type": row.mime_type,
        });
        self.record_file_side_effects(
            ctx,
            &entity,
            record_id,
            "file.uploaded",
            "file.uploaded",
            Some("attachment.created"),
            &format!("{} attached", filename),
            meta,
            None,
            Some(&json!({
                "filename": filename,
                "attachment_id": id,
                "size": row.size,
            })),
        )
        .await?;
        Ok(row)
    }

    pub async fn get_attachment(
        &self,
        ctx: &OpContext,
        id: Uuid,
        store: &AttachmentStore,
        blobs: &dyn BlobStore,
    ) -> QefroResult<(Attachment, Vec<u8>)> {
        let meta = store.get(ctx.tenant_id, id).await?;
        self.get(ctx, &meta.entity, meta.record_id).await?;
        let bytes = blobs
            .get(ctx.tenant_id, &meta.storage_key)
            .map_err(client_safe_error)?;
        Ok((meta, bytes))
    }

    pub async fn update_attachment_meta(
        &self,
        ctx: &OpContext,
        id: Uuid,
        filename: Option<&str>,
        description: Option<&str>,
        store: &AttachmentStore,
    ) -> QefroResult<Attachment> {
        let current = store.get(ctx.tenant_id, id).await?;
        let entity = self.registry().get(&current.entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Update)?;
        self.get(ctx, &current.entity, current.record_id).await?;
        let filename = match filename {
            Some(name) => sanitize_filename(name)?,
            None => current.filename.clone(),
        };
        let description = match description {
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.chars().take(500).collect::<String>())
                }
            }
            None => current.description.clone(),
        };
        store
            .update_meta(ctx.tenant_id, id, &filename, description.as_deref())
            .await?;
        let updated = store.get(ctx.tenant_id, id).await?;
        let meta = json!({
            "attachment_id": id,
            "filename": updated.filename,
            "description": updated.description,
        });
        self.record_file_side_effects(
            ctx,
            &entity,
            current.record_id,
            "file.updated",
            "file.updated",
            None,
            &format!("{} updated", updated.filename),
            meta,
            Some(&json!({
                "filename": current.filename,
                "description": current.description,
            })),
            Some(&json!({
                "filename": updated.filename,
                "description": updated.description,
                "attachment_id": id,
            })),
        )
        .await?;
        Ok(updated)
    }

    pub async fn replace_attachment(
        &self,
        ctx: &OpContext,
        id: Uuid,
        filename: &str,
        mime: &str,
        bytes: &[u8],
        store: &AttachmentStore,
        blobs: &dyn BlobStore,
    ) -> QefroResult<Attachment> {
        let current = store.get(ctx.tenant_id, id).await?;
        let entity = self.registry().get(&current.entity)?;
        self.permissions()
            .check(ctx, &entity.name, Action::Update)?;
        self.get(ctx, &current.entity, current.record_id).await?;
        let filename = sanitize_filename(filename)?;
        let mime = resolve_mime(&filename, mime, bytes)?;
        validate_upload(&filename, &mime, bytes.len() as i64)?;
        let new_key = storage_key(&current.entity, current.record_id, current.id, &filename)?;
        self.put_blob(blobs, ctx.tenant_id, &new_key, bytes)?;
        if let Err(err) = store
            .replace_blob(
                ctx.tenant_id,
                id,
                &filename,
                &mime,
                bytes.len() as i64,
                &new_key,
            )
            .await
        {
            self.enqueue_blob_purge(ctx, &new_key, id).await;
            return Err(err);
        }
        if current.storage_key != new_key {
            if blobs.delete(ctx.tenant_id, &current.storage_key).is_err() {
                self.enqueue_blob_purge(ctx, &current.storage_key, id).await;
            }
        }
        let updated = store.get(ctx.tenant_id, id).await?;
        let meta = json!({
            "attachment_id": id,
            "filename": updated.filename,
            "size": updated.size,
            "mime_type": updated.mime_type,
        });
        self.record_file_side_effects(
            ctx,
            &entity,
            current.record_id,
            "file.replaced",
            "file.replaced",
            None,
            &format!("{} replaced", updated.filename),
            meta,
            Some(&json!({
                "filename": current.filename,
                "size": current.size,
            })),
            Some(&json!({
                "filename": updated.filename,
                "size": updated.size,
                "attachment_id": id,
            })),
        )
        .await?;
        Ok(updated)
    }

    pub async fn delete_attachment(
        &self,
        ctx: &OpContext,
        id: Uuid,
        store: &AttachmentStore,
        blobs: &dyn BlobStore,
    ) -> QefroResult<()> {
        let meta = store.get(ctx.tenant_id, id).await?;
        let entity = self.registry().get(&meta.entity)?;
        self.permissions()
            .check(ctx, &meta.entity, Action::Update)?;
        self.get(ctx, &meta.entity, meta.record_id).await?;
        store.delete(ctx.tenant_id, id).await?;
        if blobs.delete(ctx.tenant_id, &meta.storage_key).is_err() {
            self.enqueue_blob_purge(ctx, &meta.storage_key, id).await;
        }
        let payload = json!({
            "filename": meta.filename,
            "attachment_id": id,
        });
        self.record_file_side_effects(
            ctx,
            &entity,
            meta.record_id,
            "file.deleted",
            "file.deleted",
            Some("attachment.deleted"),
            &format!("{} removed", meta.filename),
            payload,
            Some(&json!({
                "filename": meta.filename,
                "size": meta.size,
                "attachment_id": id,
            })),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn attach_attachment_counts(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        items: &mut [Value],
    ) {
        if !entity.attachments || items.is_empty() {
            return;
        }
        let ids: Vec<Uuid> = items
            .iter()
            .filter_map(|row| {
                row.get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        let store = AttachmentStore::new(self.pool().clone());
        let Ok(counts) = store
            .counts_for_records(ctx.tenant_id, &entity.name, &ids)
            .await
        else {
            return;
        };
        let map: std::collections::HashMap<_, _> = counts.into_iter().collect();
        for item in items {
            let Some(id) = item
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            if let Some(obj) = item.as_object_mut() {
                obj.insert(
                    "_attachment_count".into(),
                    json!(map.get(&id).copied().unwrap_or(0)),
                );
            }
        }
    }
}
