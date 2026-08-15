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

## Field permissions

`permission_level` on a field plus `FieldLevelGrant` per role. Reads strip unauthorized fields. Writes of unauthorized keys return 403. See [field-permissions.md](field-permissions.md).

## Studio, apps, workflows

Studio routes require capabilities (`studio.view`, `studio.edit`, publish, manage apps/workflows/permissions). Production publish is stricter than development. App enablement is tenant configuration intersected with the installed set and plan.

## Attachments, reports, webhooks, notifications

These APIs load the owning record (or require Admin) through `EntityService` first. Webhook secrets are never returned. Notification lists are tenant-scoped.
