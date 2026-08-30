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

The framework validates the named transition **before** the handler, inside the same transaction. If the handler does not change status, the framework applies the transition when persisting. Invalid transitions (`Pending → Completed`, `Completed → Pending`) fail with `invalid_transition` (HTTP 409) and do not mutate related records.

V0.2 `POST /api/v1/{slug}/{id}/transition` still works. If a matching business operation exists, that path delegates to `EntityService::execute` so table occupancy (and similar rules) cannot be skipped.

GET responses include `_workflow` (allowed transitions with `id`, `label`, `from_state`, `to_state`, `permissions`, `confirmation`) and `_actions` (allowed operations). The UI prefers `_actions` and never PATCHes workflow status.

```
UI → QefroClient → POST /{slug}/{id}/transition → Workflow engine → EntityService
```

Successful transitions write an Activity row (`workflow_transition`) and a `workflow.transitioned` event. Generic Detail, List row actions, Cards, and Kanban all render transitions from metadata. Confirmation dialogs come from `TransitionDef::confirm`, not hardcoded entity names.
