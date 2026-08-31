# PostgreSQL Row-Level Security

Qefro’s primary isolation is **application authorization**:

```
session tenant_id → SQL tenant predicates → RBAC → RowPolicy → EntityService
```

That already covers CRUD, search, reports, dashboards, import, export, bulk, files, and (as of hardening v2) Activity.

## What RLS would add

PostgreSQL RLS is a **defense in depth** so a missed `WHERE tenant_id = $1` cannot return another tenant’s rows. It does **not** replace RBAC or RowPolicy. RLS typically encodes “this connection is tenant X”, not “this user may only see assigned tickets”.

## What application RBAC already covers

- Role actions (`Read`, `Update`, Studio caps)
- Field permissions
- RowPolicy (`AssignedTo`, `CreatedBy`, …)
- Workflow transitions
- Worker `OpContext` rebuilt from the **job row**, never from payload `tenant_id`

## Migration complexity

Every entity table plus framework tables would need `ENABLE` / `FORCE ROW LEVEL SECURITY` and a policy. `FORCE` is required because the application role is usually the table owner (owners bypass RLS otherwise). Every query path — including workers, Studio, migrations, and tests — must set a transaction-local GUC. Connection pooling makes session-level `SET` unsafe: a reused connection could inherit Tenant B’s GUC.

## Connection pooling

sqlx uses a pool. **Only `SET LOCAL` / `set_config(..., true)` inside a transaction is safe.** A forgotten `SET LOCAL` with `FORCE RLS` fails closed (no rows). A forgotten `SET` (session) would leak tenant context across requests.

Workers must set tenant GUC from `jobs.tenant_id` on the same transaction they query. Migrations and `ActivityStore::purge_older_than` use an explicit `qefro.rls_bypass=on` GUC — this is a **maintenance bypass**, not a database superuser, and is documented here so it is not treated as a normal application path.

## Decision

**Application authorization remains authoritative.**

**Optional RLS defense in depth is enabled as a pilot on `qefro_activity` only.** Entity tables (customers, orders, …) stay application-authorization-only until a broader rollout can wrap every query in a tenant transaction without pooling leaks.

Background jobs that write activity inherit tenant context from `OpContext::worker` (job row), then `ActivityStore` sets `qefro.tenant_id` locally on that transaction.

See [security-audit.md](security-audit.md) and [multitenancy.md](multitenancy.md).
