# Import runtime

Generic CSV and JSON import for any standalone, non-singleton entity. Import is another way of writing business data through EntityService. It is never a backdoor around validation, permissions, workflow, relations, audit, activity, or tenant isolation.

```
CSV / JSON
    │
    ▼
File Runtime (BlobStore)
    │
    ▼
Mapping + Validation (EntityDef)
    │
    ▼
JobQueue (`import.run`)  ← large files
    │
    ▼
EntityService create / update
    │
    ▼
Workflow · Relations · Permissions · Outbox
    │
    ▼
Activity (one summary) · Audit · Automation events
```

Do not implement Excel, AI mapping, ETL, CDC, or a transformation language.

## Sources

Supported:

* CSV (header row required)
* JSON array of objects

Nested objects and child-row graphs are rejected with `Nested relation import is not supported.` Child entity import is left for a later iteration.

Encoding must be UTF-8. Duplicate headers, missing headers, invalid JSON, and invalid encoding return `400` — they never panic.

Limits (override with env):

| Limit | Default | Env |
| --- | --- | --- |
| File size | 10 MiB (same as File Runtime) | `QEFRO_MAX_UPLOAD_BYTES` |
| Rows | 100,000 | `QEFRO_MAX_IMPORT_ROWS` |
| Columns | 64 | `QEFRO_MAX_IMPORT_COLUMNS` |

Small imports (≤ 200 rows and ≤ 256 KiB) may run synchronously. Larger imports store the file in BlobStore and enqueue `import.run`. The HTTP request does not wait for completion.

## Mapping

Fields come from EntityDef. There is no second field list.

Automatic mapping is conservative:

* Exact field name (case-insensitive), or
* Unique field label (so Customer Export CSV round-trips)

Ambiguous labels are left **Ignored**. They are never guessed.

Manual mapping can set a column to a field or leave it ignored. Protected and secret fields cannot be mapped:

```
id, tenant_id, created_at, created_by, updated_at, updated_by
password_hash, tokens, session hashes, API secrets
workflow status
computed / server-managed / ephemeral / child tables
```

CSV values are coerced to existing field types (`string`, `integer`, `decimal`, `boolean`, `date`, `datetime`, `enum`, `relation`). Import does not invent field types.

Saved mappings reuse `saved_filters` with `query.kind = "import_mapping"` when a user wants to repeat a mapping. There is no extra configuration database.

## Preview and validate

```
POST /api/v1/{slug}/import/preview
POST /api/v1/{slug}/import          { "dry_run": true }
```

Preview writes nothing. Dry-run validates through the same rules as EntityService (required fields, types, uniques, relations) and still writes nothing.

Example:

```
1,245 rows
1,220 valid
20 warnings
5 errors
Nothing imported.
```

## Create, update, upsert

```
mode: create | update | upsert
duplicate_policy: fail | skip | update
match_field: <unique EntityDef field>
```

Matching is exact. Fuzzy matching is not supported. Update and upsert require an explicit unique match field (or the entity has exactly one unique importable field). `id` is not a match key.

Duplicate policy is never a silent merge:

* **Fail row** — report the duplicate
* **Skip row** — leave the existing record
* **Update existing** — requires Update permission

A user with Create but not Update cannot upsert or apply `duplicate_policy=update`.

## Relations

Relation columns may be a UUID or a unique lookup on the target (email, `external_id` when present, or any unique field). Multiple matches are reported as `ambiguous relation`. Names are not assumed unique.

## Workflow, operations, identity

Imported records are created in the workflow **initial** state. A CSV cannot insert `status = Confirmed` or jump a state. Transitions remain Operation/workflow APIs.

Fields owned by accounting, inventory, or commerce operations still go through EntityService. A raw CSV cannot post a ledger, change stock balances, or bypass pricing.

User credentials (`password`, `password_hash`, tokens, JWTs) are stripped. Creating Users still goes through AuthService via EntityService.

## Jobs, progress, cancellation, retry

Statuses: `pending`, `validating`, `running`, `completed`, `completed_with_errors`, `failed`, `cancelled`.

Progress is stored on `qefro_import_jobs` (`processed / total`, created, updated, skipped, failed). The generic UI polls `GET /api/v1/imports/{id}`.

Cancellation is cooperative: the worker checks `cancel_requested` between batches and stops. Already imported rows are not rolled back.

Retries resume from `checkpoint`. Unique match fields prevent duplicating rows that already landed. Duplicate job enqueue uses `idempotency_key`.

Batches are bounded (default 100). Each EntityService create/update keeps its own transaction. The whole file is not one database transaction.

## Errors and reports

Partial success is the default: valid rows import, invalid rows are reported. `strict: true` fails the batch on the first error.

Error reports are CSV in File Runtime / BlobStore:

```
row, original columns, field, error, reason
```

Values that start with `=`, `+`, `-`, `@` are escaped with the same CSV-injection protection used by export.

## Activity, audit, events, notifications

One Activity summary is recorded:

```
Qefro Import
1,220 records imported
20 warnings
5 failed
```

Per-row field changes continue to use existing audit. Domain events (`customer.created`, `customer.updated`) use the normal EntityService → Outbox path. Bulk import does **not** silently suppress automations. When a background job finishes, an in-app notification `Import completed` is stored through NotificationStore.

## Permissions and tenants

Import checks Create (and Update when updating). Row policies apply to every EntityService write. Client-supplied `tenant_id` is ignored/rejected. Import jobs, source files, and error reports are tenant-scoped: Tenant B cannot list Tenant A's jobs, download the file, or see imported rows.

## REST

```
POST /api/v1/{slug}/import/preview
POST /api/v1/{slug}/import
POST /api/v1/{slug}/import/upload
GET  /api/v1/{slug}/imports
GET  /api/v1/imports
GET  /api/v1/imports/{id}
POST /api/v1/imports/{id}/cancel
POST /api/v1/imports/{id}/retry
GET  /api/v1/imports/{id}/errors
```

Body (JSON):

```json
{
  "csv": "name,email\nAda,ada@example.com",
  "json": "[{\"name\":\"Ada\",\"email\":\"ada@example.com\"}]",
  "mapping": [{ "column": "Customer Name", "field": "name" }],
  "mode": "create",
  "duplicate_policy": "fail",
  "match_field": "email",
  "dry_run": false,
  "batch_size": 100,
  "blob_key": null,
  "idempotency_key": "migration-2026-08-31"
}
```

## SDK

```ts
api.importPreview(slug, { csv });
api.importRun(slug, { csv, dry_run: true });
api.importUpload(slug, file);
api.importJobs(slug);
api.importJob(id);
```

## CLI

```bash
qefro import Customer customers.csv
qefro import Customer customers.csv --dry-run
qefro import Customer customers.json --mode upsert --match-field email
```

Requires `QEFRO_URL` and `QEFRO_TOKEN`. Output:

```
Importing Customer
Rows: 1,245

Validation
✓ 1,220 valid
⚠ 20 warnings
✕ 5 errors

Dry run complete.
```

`qefro inspect Customer` prints `import=true` when the entity is standalone and not a singleton, plus unique matching fields.

## Studio

Entity Studio shows **Import** enabled when `standalone && !singleton`, and lists unique fields used for matching. There is no second importer to configure.

## UI

Entity List → More → Import:

Select entity (current list) → Upload → Map → Preview → Validate → Import → Results.

Mapping table, paginated preview, error filter, progress, import history, and error-report download use existing Material 3 cards, tables, chips, dialogs, and progress indicators.

Export CSV uses field labels as headers; conservative label mapping makes Export → Import work without a manual transform for basic files.

## Accounting, inventory, commerce

Do not import posted journal state, stock balances, or confirmed orders as raw rows. Create documents in their initial workflow state, then run the existing operations.
