# Workflows

A workflow is a named state machine bound to one entity status field. Status cannot be PATCHed; callers use a transition or a business operation.

## Definition

```rust
WorkflowDef::new("reservation", "Reservation", "Pending")
    .state(StateDef::new("Confirmed"))
    .transition(TransitionDef::new("confirm", "Pending", "Confirmed").roles(&["Manager", "Staff"]))
```

`WorkflowRegistry::apply` checks the current state and the caller's roles. Admin bypasses role lists.

## With business operations

Prefer a business operation when the transition also mutates related records:

```rust
OperationDef::new("confirm", "Reservation").transition("confirm")
```

The framework validates the named transition **before** the handler, inside the same transaction. If the handler does not change status, the framework applies the transition when persisting.

V0.2 `POST /api/v1/{slug}/{id}/transition` still works. If a matching business operation exists, that path delegates to `EntityService::execute` so table occupancy (and similar rules) cannot be skipped.

GET responses include `_workflow` (allowed transitions) and `_actions` (allowed operations). The UI prefers `_actions`.
