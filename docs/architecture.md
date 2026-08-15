# Architecture

Qefro is a modular monolith. One HTTP process, an optional dedicated worker, one PostgreSQL database, one generic frontend. Redis is not required.

Building an app end to end: [Build a fullstack application](fullstack.md).

V0.7 extends V0.6. It does not rewrite the runtime. It turns repository-local app directories into versioned, installable `.qefro` packages with validation, a registry, tenant enablement, and additive migrations.

V0.6 extends V0.5. It does not rewrite the runtime. It turns metadata-driven CRUD into business documents: child tables, formulas, numbering, print, and reports.

V0.5 extends V0.4. It does not rewrite the runtime. It turns generic CRUD into a metadata-driven form engine: data type stays independent of widget, UI schema is versioned, and the frontend resolves widgets through a registry.

V0.4 extends V0.3. It does not rewrite CRUD, operations, or the agent boundary. It adds production tenant customization, application entitlements, and an explicit worker policy.

## Metadata is the source of truth

`EntityDef` describes fields, validation, relations, UI, audit, and workflow binding. `OperationDef` describes named business actions. Together they drive:

- DDL generation
- CRUD SQL (parameterized; identifiers allowlisted)
- REST routes
- OpenAPI
- Generic React widgets and action buttons
- Agent tool JSON schemas
- Dashboard cards
- Child tables and computed fields
- Document numbering, print, and reports
- Audit, events, and optional jobs

## Security pipeline

HTTP APIs, CLI actions, and agent tools share `EntityService`:

```
HTTP / Agent / CLI / UI
        ↓
    Authentication
        ↓
    Tenant Context
        ↓
    Application availability
        ↓
         RBAC
        ↓
       Validation
        ↓
        Workflow
        ↓
     Handler + transaction
        ↓
         Audit
        ↓
         Event (after COMMIT)
        ↓
     Background job (optional)
```

```
REST ─────────────┐
CLI  ─────────────┤
                  ↓
             EntityService
                  ↑
Agent ────────────┘
```

Clients cannot set `tenant_id` on create, update, action, or agent invoke. `X-Tenant-ID` is ignored. Agents have no SQLx dependency and cannot run SQL. Restaurant and CRM rules live in `examples/`, not in core crates.

User and agent calls use user RBAC. Workers use `OpContext::worker` and may run only handlers/operations marked `worker_safe`.

`ctx.get` inside an operation transaction uses `SELECT … FOR UPDATE` so exclusive resources (a dining table, a room) cannot be acquired twice. HTTP 5xx responses use `QefroError::public_message`: SQL, credentials, and stack traces are not returned to clients.

## Tenant isolation

Every tenant-owned row has `tenant_id`. The authenticated session supplies tenant identity. Repositories always add `WHERE tenant_id = $1`. A post-read check rejects mismatches as 404. Tenant branding, feature flags, and dashboards are loaded for that session tenant only.

## Authorization

Permissions are evaluated in `EntityService`. Admin bypasses entity grants; other roles use the matrix registered by application modules. Operations also honor `OperationDef.roles`. Application entitlements run before RBAC. The frontend never decides whether an action is legal.

## Workflows and operations

Status fields listed on a workflow cannot be PATCHed directly. Named transitions remain available. When an application registers a business operation with the same name, `POST .../transition` delegates to that operation so related-entity rules cannot be skipped.

GET responses include `_workflow` and `_actions`.

## Hooks and transactions

| Hook | Transaction |
| --- | --- |
| `before_create` / `after_create` | CRUD uses auto-commit statements; not wrapped in an operation transaction |
| `before_update` / `after_update` | same |
| `before_delete` / `after_delete` | same |
| `before_operation` | **inside** the operation SQLx transaction, before the handler |
| `after_operation` | **inside** the transaction, after the primary write, audit, and job enqueue |

If `after_operation` fails, the transaction rolls back (no event, no job). Events are published only after COMMIT.

## Relationships

Many-to-one fields store a UUID and expand to `{ id, label, slug, entity }` in `_expanded` using batched lookups. One-to-many fields do not store a column; GET responses include `_related`. Child tables are nested collections on the parent payload and form. Many-to-many uses a junction table `{table}_{field}`.

## Application modules

Applications register entities, workflows, permissions, **operations**, jobs, hooks, and dashboards. Core crates contain no restaurant or CRM business rules.

## Agent boundary

`qefro-agent` has no SQLx dependency. Tools call `EntityOps`, including `execute`, implemented by the API crate as a thin adapter over `EntityService`.

## Why not a custom ORM

SQLx `QueryBuilder` binds values. Table and column names come only from validated metadata.

## Multi-tenant SaaS runtime

```
                    QEFRO PLATFORM
                          │
               Shared Runtime + Platform Admin
                          │
            Tenant A / Tenant B / Tenant C
            (branding, apps, features, UI config)
                          │
                    EntityService
```

One application module is installed once and enabled per tenant. Customization is configuration, not a fork. See [Deployment](deployment.md) and [Tenants](tenants.md).

