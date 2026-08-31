# Files and attachments

Any `EntityDef` can opt into documents and files with `.attachments()`. There is one attachment runtime, one permission model, and one tenant boundary.

```
EntityDef.attachments()
        ↓
EntityService
        ↓
REST / SDK / generic UI
        ↓
qefro_attachments metadata  +  BlobStore bytes
```

Do not add `CustomerAttachment` tables, a second file API, or a document CMS. Binary content never belongs in entity JSON.

## Capability

```rust
EntityDef::new("Order")
    .attachments()
    .build()
```

The generic detail page then shows an Attachments tab. `qefro entity show Order` lists `attachments=true`. Studio shows the Attachments capability next to Activity, Audit, and Workflow.

Child tables without `.attachments()` cannot store files. Opt in explicitly.

## Metadata

`qefro_attachments` stores:

```
id, tenant_id, entity, record_id, filename, description,
mime_type, size, storage_key, uploaded_by, created_at
```

Clients receive filename, description, MIME type, size, uploader, and timestamps. They never receive:

- `storage_key`
- bucket credentials
- filesystem paths
- provider secrets

Storage keys are generated server-side: `attachments/{entity}/{record_id}/{id}_{filename}`.

## Storage

`BlobStore` is the storage abstraction (`LocalBlobStore` today; S3-compatible later). Attachments reuse it. Form-widget uploads (`POST /api/v1/files`) are a separate field-widget helper and are not the entity file runtime.

Private files stay private. Downloads use the session Bearer token. There are no permanent public URLs.

If a blob delete fails after the metadata row is removed, JobQueue runs `attachment.purge` until storage catches up.

## Permissions

Existing RBAC. No second permission model.

| Entity action | File action |
| --- | --- |
| Read | list, download, preview |
| Update | upload, replace, rename, describe, delete |

A caller who only knows an attachment id still cannot read it: every request loads the parent record in the session tenant through `EntityService`. Tenant B never sees Tenant A's files through REST, SDK, search, or download.

Guessing another tenant's attachment id returns 404.

## Upload

```http
POST /api/v1/{slug}/{id}/attachments
Content-Type: multipart/form-data
```

The generic UI supports click upload, drag/drop, multiple files, progress, cancel, retry, and error. Server-side validation:

- filename: basename only, no `..` / `/` / `\`
- size: `QEFRO_MAX_UPLOAD_BYTES` (platform default 10 MiB)
- MIME: magic-byte sniff; claimed types that disagree with contents are rejected
- empty files rejected

Do not trust browser MIME types. Tenant and user ids always come from the session.

Successful upload emits `file.uploaded` (and `attachment.created` for compatibility), an Activity row (`Invoice.pdf attached`), and an Audit entry when the entity has audit enabled. Workflow state is not mutated.

## Download and preview

```http
GET /api/v1/attachments/{id}
GET /api/v1/attachments/{id}?disposition=inline
```

Default disposition is `attachment`. Preview uses `inline` for images, PDF, and text in the generic dialog. Unsupported types show “Preview unavailable” with Download. There is no custom document renderer.

## Replace, metadata, delete

```http
POST   /api/v1/attachments/{id}/replace   (multipart)
PATCH  /api/v1/attachments/{id}           { "filename", "description" }
DELETE /api/v1/attachments/{id}
```

Replace keeps the same attachment id (not a versioning CMS). Delete asks for confirmation in the UI. Filename and description are the only client-editable metadata.

## List and search

```http
GET /api/v1/{slug}/{id}/attachments?page=1&page_size=50
```

List returns metadata only — never file bytes. Global search matches filename and description (not binary contents) and groups hits under **Attachments**. Clicking a hit opens the parent record. Entity permissions still apply. Command palette uses the same search.

List pages may show a compact `📎 3` indicator that links to the detail Attachments tab. CSV export may include `attachment_count`; it never includes binaries or storage URLs.

## Comments

A comment may reference an existing attachment on the same record:

```json
POST /api/v1/{slug}/{id}/comments
{ "message": "Customer requested an updated invoice.", "attachment_id": "..." }
```

Activity shows the filename. This is the existing comment system.

## SDK

```ts
client.files.list(slug, id)
client.files.upload(slug, id, file, onProgress, signal)
client.files.download(id)
client.files.delete(id)
client.files.update(id, { filename, description })
client.files.replace(id, file)
```

Existing `attachments` / `uploadAttachment` / `deleteAttachment` helpers remain aliases.

## Events and jobs

| Event | When |
| --- | --- |
| `file.uploaded` / `attachment.created` | upload |
| `file.replaced` | replace |
| `file.updated` | filename/description |
| `file.deleted` / `attachment.deleted` | delete |

Automations subscribe to these names like any other domain event. Do not add a `FileNotificationService`.

`attachment.purge` is worker-safe JobQueue work for leftover blobs.

Thumbnails, virus scanning, and conversion are extension points — not implemented here.

## Restaurant and CRM

Restaurant `Order`, `Reservation`, and `Customer`, plus CRM `CrmCustomer` and platform `Task`, opt in with `.attachments()`. No application-specific file backend is required.
