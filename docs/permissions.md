# Permissions

Authorization is always server-side. The generic UI hiding a button is not a security boundary.

## Pipeline

```
Auth → Tenant → App entitlement → RBAC → Field permissions → Validation → Workflow → EntityService
```

Every protected action (REST, agent tool, CLI `qefro action`, public form, import, attachment, report, Studio publish) is checked on this path.

## Entity RBAC

```yaml
- role: Staff
  entity: Customer
  actions: [create, read, update, delete, list]
```

Admin bypasses role lists after authentication. Workers use role `Worker` and only `worker_safe` operations. Public forms use role `Public` with an allowlisted action set.

## UI hints vs API 403

`GET /api/v1/meta/ui` includes per-entity `permissions: { list, create, read, update, delete }`. GET records include `_permissions: { update, delete }`. The generic UI uses those to hide New / Edit / Delete. They are chrome hints, not a security boundary. Unauthorized writes still return **403** from `EntityService`. Tenant isolation is `WHERE tenant_id = $1` plus a post-read check (mismatch is **404**), not a UI filter.

Search, reports, dashboards, and saved views use the same pipeline. Unauthorized search hits, report fields, dashboard widgets, and saved views are denied or skipped. Audit widgets remain Admin-only.

## Field permissions

`permission_level` on a field plus `FieldLevelGrant` per role. Reads strip unauthorized fields. Writes of unauthorized keys return 403. See [field-permissions.md](field-permissions.md).

## Studio, apps, workflows

Studio routes require capabilities (`studio.view`, `studio.edit`, publish, manage apps/workflows/permissions). Production publish is stricter than development. App enablement is tenant configuration intersected with the installed set and plan.

## Attachments, reports, webhooks, notifications

These APIs load the owning record (or require Admin) through `EntityService` first. Webhook secrets are never returned. Notification lists are tenant-scoped.

## Identity

Person is a tenant-owned individual (canonical name/email/phone once `person_id` is set). User is the existing auth login (roles, membership, enabled). Customer / Patient / Employee are business records and must not be modeled as User. See [identity.md](identity.md). Platform Task uses the same matrix (`Task` Create/Read/Update/Delete) without a separate permission registry. See [tasks.md](tasks.md).
