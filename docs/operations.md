# Business operations

CRUD is generated from entity metadata. Real processes — confirm a reservation, convert a lead — are **business operations**. Multi-entity transactions use the same `OperationDef` + `OperationHandler` path; see [business-operations.md](business-operations.md). There is no second engine.

```
Define Entity
      ↓
Define Workflow
      ↓
Define Business Operation
      ↓
Implement Handler
      ↓
Expose REST
      ↓
Generate UI Action
      ↓
Generate Agent Tool
      ↓
Audit
      ↓
Emit Event
      ↓
Optional Background Job
```

## Define an operation

```rust
use qefro_api::{operation, InstalledApp, OperationHandler, OperationCtx};

app.operation(
    operation("confirm", "Reservation")
        .label("Confirm")
        .permission("reservation.confirm")
        .roles(&["Manager", "Staff"])
        .transition("confirm")
        .event("reservation.confirmed")
        .job("notify_reservation_confirmed"),
    ConfirmReservation,
);
```

Handlers receive a context that already has the authenticated user and tenant. They must not extract auth themselves, and they must not run raw SQL.

```rust
async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
    let table_id = ctx.uuid_field("table_id")?;
    let table = ctx.get("DiningTable", table_id).await?;
    if table.get("status").and_then(|v| v.as_str()) != Some("available") {
        return Err(OperationCtx::fail(
            "table_unavailable",
            "The selected table is not available",
        ));
    }
    ctx.update("DiningTable", table_id, json!({ "status": "reserved" })).await?;
    ctx.apply_transition("confirm")?;
    Ok(ctx.record.clone())
}
```

`QefroError::Business { code, message }` maps to HTTP 409 / `business_rule_failed`. Stack traces are never sent to clients.

## Unified pipeline

HTTP, the generic UI, the CLI, and agent tools all call `EntityService::execute`:

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
     Input / workflow validation
        ↓
     Business logic handler
        ↓
      SQLx transaction
        ↓
         Audit
        ↓
         Events (after COMMIT)
        ↓
     Background job (optional)
```

There is no HTTP-only or agent-only mutation path.

## Registry

```
GET /api/v1/operations
GET /api/v1/{slug}/{id}/actions
POST /api/v1/{slug}/{id}/actions/{name}
```

`GET /operations` is permission-filtered for the current user and tenant. Record actions are additionally filtered by workflow state. The backend remains authoritative; the UI only hides buttons.

## Transactions

`execute` begins a SQLx transaction, locks the primary row (`FOR UPDATE`), runs hooks and the handler, writes audit and jobs, then commits. Related rows loaded with `ctx.get` are also locked. Any error rolls back every mutation, including audit rows and job inserts. Domain events are published only after a successful commit. A failed handler does not emit a successful business event and does not write a success audit entry.

## CLI

```bash
qefro operations
qefro operations Reservation
qefro action Reservation 123 confirm
```

`qefro action` POSTs to the running API (`QEFRO_URL`, `QEFRO_TOKEN`). It does not reimplement business rules.
