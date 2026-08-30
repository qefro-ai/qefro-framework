# Business operations

Qefro already has one operation runtime: [`OperationDef`](operations.md) plus `OperationHandler`. This document describes how that same path composes **multi-entity business transactions**. There is no second `BusinessOperation` type and no second execution engine.

```
EntityDef
   │
   ▼
EntityService          CRUD / persistence for one business object
   │
   ├──────────► Workflow          state, transition, guard, permission
   │
   └──────────► OperationDef      a named business action
                  │
                  ▼
           OperationCtx           one SQLx transaction
                  │
         ┌────────┼────────┐
         ▼        ▼        ▼
     Entities   Events   Activity
                  │
                  ▼
               Outbox  →  Automation / Jobs / Notifications
```

## When to use which primitive

| Primitive | Use it for |
| --- | --- |
| **EntityService** | Create, read, update, delete one record. Validation, permissions, activity, and audit run here. |
| **Workflow** | A record's own status field. Transitions, guards, and roles. Never `PATCH status`. |
| **OperationDef** | A named business action on a record (`Confirm`, `Convert`, `Complete Order`). The generic UI renders it as an action. |
| **Business operation** (same `OperationDef`) | Several entity changes that must succeed or fail together: create children, transition, create a related Task, emit events. |

Do not replace Workflow with operations. A typical flow is:

```
Order  →  workflow transition Confirm  →  OperationDef
                                      →  reserve inventory
                                      →  create invoice (when that entity exists)
```

## Transaction boundary

`EntityService.execute` begins **one** PostgreSQL transaction and passes the same `Transaction` into `OperationCtx`. Every nested `get` / `create` / `update` / `delete` / `execute` uses that connection.

```
BEGIN
  lock primary row FOR UPDATE
  handler steps (EntityService-equivalent checks on the same tx)
  audit / activity
  outbox rows
COMMIT
  → event processing, automation, jobs
```

If step 3 fails, steps 1 and 2 roll back. Events are not published before commit. External HTTP calls must not run inside the transaction; enqueue a JobQueue job instead (existing `ctx.enqueue_job`).

## Result

The presented record stays backward compatible. An `_operation` envelope is added:

```json
{
  "id": "...",
  "status": "Completed",
  "_operation": {
    "id": "…",
    "operation": "complete",
    "status": "completed",
    "message": "Order completed",
    "navigate": { "entity": "Task", "slug": "tasks", "id": "…" }
  }
}
```

Internal transaction details are not returned. The generic UI may open `navigate.slug/id` and show `message`.

## Events and correlation

`ctx.emit` and entity mutation events write to the transactional outbox. After COMMIT, the existing dispatcher publishes them. Each payload includes:

```
operation_id
request_id
```

so `order.completed`, `task.created`, and `entity.created` can be traced as one business process. Failed transactions do not publish successful business events.

Existing `AutomationDef` hooks on those event names. There is no operation-specific automation engine.

## Permissions and tenant isolation

Before the handler:

- the caller needs `Update` on the primary entity
- `OperationDef.roles` is enforced (Admin always allowed)
- worker principals require `worker_safe`

During nested steps:

- `create` requires `Create` on the target entity
- `update` / `delete` / `get` require the matching action
- workflow transitions still go through `apply_transition` (guards + roles)
- row policies apply
- `tenant_id` in the payload is rejected
- related IDs are loaded with the same tenant predicate (`get_tx`)

There is no hidden system user. If a step needs a permission the caller does not have, the operation returns 403.

## Validation

Nested creates and updates run `validate_record` and `apply_entity_rules`. Workflow guards still run on `apply_transition`. Optimistic concurrency uses `_expected_updated_at` on `ctx.update`, same as PATCH. Workflow status fields cannot be patched from an operation; use a transition.

## Nested operations

`ctx.execute(entity, id, name, input)` runs another `OperationDef` on the **same** transaction. Cycles (`A → B → A`) are rejected. Asynchronous operations cannot be nested.

## Idempotency

Send `Idempotency-Key` on `POST /api/v1/{slug}/{id}/actions/{name}`. Replays return the original result instead of creating a second order, invoice, or task. `OperationDef.idempotent()` makes the header required. Runs are stored in `qefro_operation_runs` (not a second job table).

## Synchronous vs asynchronous

Default `execution` is `sync` (the HTTP request is the transaction).

`.async_execution()` enqueues `qefro.operation.execute` on the existing JobQueue and returns `_operation.status = queued`. Poll `GET /api/v1/operation-runs/{id}` for `queued` / `running` / `progress` / `completed` / `failed`. The job reconstructs the original caller so permissions are not elevated to Worker. Do not add this to ordinary CRUD.

## UI, SDK, CLI, Studio

- Actions come from `_actions` / metadata. Confirmation uses existing `requires_confirmation` / `confirmation_message`.
- `input_schema` is rendered as a dialog in the generic ActionBar. No custom React page.
- SDK: `client.action(slug, id, name, inputs)` or `client.execute({ entity, id, action, inputs })`. No `createOrder()` helpers in the generic client.
- `qefro inspect Order` lists declared operations.
- Studio → Permissions shows source-managed operations (inputs, transition, execution). Composition stays in version-controlled Rust handlers.

## Restaurant and CRM examples

**Complete Order** transitions the order, creates a related Task (the restaurant app has no Invoice entity), writes activity/audit, and emits `order.completed`.

**Convert Lead** creates a `CrmCustomer`, a follow-up Task, qualifies the lead, and may return `navigate` to the customer.

## Architecture invariant

There is still one EntityDef system, one EntityService, one permission registry, one workflow engine, one event/outbox path, one JobQueue, one automation system, and one generic UI. Operations compose them; they do not duplicate them.
