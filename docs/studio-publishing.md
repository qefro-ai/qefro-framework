# Studio publishing

```
Draft → Validate → Preview (diff + impact) → Publish → Overlay + optional ADD COLUMN → Audit
```

`POST /api/v1/studio/drafts` stores a tenant-scoped draft. `POST /validate` runs the metadata validator (relations, formulas, workflows, permissions, schema impact) without writing. `POST /publish` applies the overlay.

## Impact

| Impact | Behavior |
| --- | --- |
| `safe` | Overlay only |
| `additive` | Overlay + `apply_schema` (add columns). Production requires `confirm_migration` |
| `destructive` | Rejected. Message includes `⚠ Database migration required` |

## Versions and rollback

Each successful publish appends `qefro_studio_versions` (user, timestamp, summary, payload). Rollback restores a previous payload **only if** the reverse diff is not schema-changing. Destructive rollback is refused.

Application metadata overlays are process-wide (the registry is shared). Tenant drafts and version rows are isolated by `tenant_id`. Tenant branding is never stored in these tables; it uses `/api/v1/tenants/me/config`.

## Audit

Publishes write `audit_logs` with entity `studio` and actions such as `entity.field.ui.updated`, `workflow.updated`, `permissions.updated`. Secrets are not copied into payloads; field values are metadata only.
