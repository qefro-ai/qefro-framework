# Audit

Audit is the **security / system** record of mutations. It is not the business timeline.

```
user_id=123
entity=Customer
record=456
operation=update
field=status
old=Lead
new=Qualified
timestamp=…
```

## Storage

Existing `audit_logs` table (no second log). Rows include actor, tenant, entity, record, operation, old/new JSON, request id, and timestamp. Indexes cover `tenant_id`, `(entity, entity_id)`, and `user_id`. `AuditLogger::purge_older_than` supports retention (minimum 30 days).

Updates record changed fields. **Secrets are never stored or returned:**

```
password, password_hash, session_token, JWT, reset_token, storage_key, …
```

## Authorization

```
GET /api/v1/audit?entity=&entity_id=&limit=
```

**Admin only.** Other roles receive `403`. The generic Detail timeline does **not** load this endpoint.

The generic **Audit log** page (`/settings/audit`) is a dense table for administrators. Staff never see it in navigation.

## Activity vs audit

Ordinary users see [Activity](activity.md). Audit is a security record and is not shown on entity timelines.
