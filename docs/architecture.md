# Architecture

Qefro is a modular monolith. One Axum process, one PostgreSQL database. Redis is not required.

V0.3 extends V0.2. It does not rewrite CRUD. Business operations join the same `EntityService` pipeline used by REST and agents.

## Metadata is the source of truth

`EntityDef` describes fields, validation, relations, UI, audit, and workflow binding. `OperationDef` describes named business actions. Together they drive:

- DDL generation
- CRUD SQL (parameterized; identifiers allowlisted)
- REST routes
- OpenAPI
- Generic React widgets and action buttons
- Agent tool JSON schemas
- Dashboard cards
- Audit, events, and optional jobs

## Security pipeline

HTTP APIs, CLI actions, and agent tools share `EntityService`:

```
HTTP / Agent / CLI / UI
        ↓
   BusinessOperation
        ↓
    Authentication
        ↓
      Tenant Context
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

Clients cannot set `tenant_id`. Agents have no SQLx dependency and cannot run SQL. Restaurant and CRM rules live in `examples/`, not in core crates.

## Tenant isolation

Every tenant-owned row has `tenant_id`. The authenticated session supplies tenant identity. Repositories always add `WHERE tenant_id = $1`. A post-read check rejects mismatches as 404.

## Authorization

Permissions are evaluated in `EntityService`. Admin bypasses entity grants; other roles use the matrix registered by application modules. Operations also honor `OperationDef.roles`. The frontend never decides whether an action is legal.

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

Many-to-one fields store a UUID and expand to `{ id, label, slug, entity }` in `_expanded` using batched lookups. One-to-many fields do not store a column; GET responses include `_related`. Many-to-many uses a junction table `{table}_{field}`.

## Application modules

Applications register entities, workflows, permissions, **operations**, jobs, hooks, and dashboards. Core crates contain no restaurant or CRM business rules.

## Agent boundary

`qefro-agent` has no SQLx dependency. Tools call `EntityOps`, including `execute`, implemented by the API crate as a thin adapter over `EntityService`.

## Why not a custom ORM

SQLx `QueryBuilder` binds values. Table and column names come only from validated metadata.
