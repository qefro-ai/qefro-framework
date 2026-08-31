# Activity

Activity is the **business-facing timeline** on a record. It is not the security audit log.

```
Ahmed moved Order #1042
Preparing → Ready
Today, 10:42 AM
```

## Model

Tenant-scoped rows in `qefro_activity`:

| Field | Purpose |
| --- | --- |
| `tenant_id` | Isolation |
| `entity_type` + `entity_id` | Owning record |
| `actor_id` / `actor_name` | Who did it (`Qefro Agent` when `ctx.source = agent`) |
| `activity_type` | `created`, `updated`, `deleted`, `workflow_transition`, `comment`, `assignment`, `system` |
| `message` | Human summary |
| `metadata` | Optional extras (`from` / `to`, field changes). Secrets are stripped. |
| `created_at` | Timestamp |

Indexes: `(tenant_id, entity_type, entity_id, created_at DESC)`, `(tenant_id, actor_id, created_at DESC)`, `(tenant_id, created_at DESC)`. High-volume rows may be purged with `ActivityStore::purge_older_than` (default 90 days). Partitioning is not required in 1.2.

## API

```
GET  /api/v1/{slug}/{id}/activity
POST /api/v1/{slug}/{id}/comments   { "message": "…" }
```

List requires read access to the record (404 across tenants), which applies RowPolicy via `EntityService::get`. Dashboard activity widgets and recent-activity counts use the same `get` path so a hidden record cannot appear as “12 activities”. Comments are Activity rows with `activity_type = comment`. There is no separate messaging system.

`QefroClient.activity` / `getActivity` / `addComment` and `EntityOps.list_activity` / `add_comment` use the same `EntityService` path.

## UI

The generic Detail **Activity** tab renders the shared `Timeline` (day groups, Material 3 chips/dots) and an optional comment form when `capabilities.comments` is true. The same component works for Customer, Order, Ticket, or any `EntityDef`.

## Activity vs audit

| | Activity | Audit |
| --- | --- | --- |
| Audience | People working the record | Administrators / security |
| Example | Ahmed changed status to Qualified | `user_id`, `entity`, `operation`, field diffs |
| UI | Timeline on Detail | `/settings/audit`, Admin only |

See [Audit](audit.md).
