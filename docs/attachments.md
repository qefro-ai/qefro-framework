# Attachments

First-class files on an entity that opted in with `.attachments()`.

Metadata stored in `qefro_attachments`:

```
id, tenant_id, entity, record_id, filename, mime_type, size, storage_key, uploaded_by, created_at
```

The storage key is generated server-side (`attachments/{entity}/{record_id}/{id}_{filename}`). Client-supplied paths are ignored. Filenames containing `..` or `/` are rejected. MIME type and size (10 MiB) are validated.

If the database commit succeeds and a later file write fails, the attachment row is not created (upload is one request: store blob then insert metadata; a failed insert leaves an orphan blob). If the blob write succeeds and the transaction fails, run storage reconciliation against `qefro_attachments` (delete files with no row). V1.0 does not auto-vacuum orphans on a timer.

## API

```http
GET    /api/v1/{slug}/{id}/attachments
POST   /api/v1/{slug}/{id}/attachments   (multipart)
GET    /api/v1/attachments/{id}
DELETE /api/v1/attachments/{id}
```

List, download, upload, and delete all load the owning record through `EntityService` first. Tenant isolation uses the session tenant, never a client `tenant_id`.

The generic detail page shows the attachment list and an upload control. No per-entity React page. Uploads emit `file.uploaded` (and `attachment.created`) plus an Activity row. `storage_key` is never serialized to clients. Guessing another tenant's attachment id returns 404.

See [Files](files.md) for the full file runtime (preview, replace, search, purge jobs, SDK). See [Business object runtime](business-object-runtime.md).
