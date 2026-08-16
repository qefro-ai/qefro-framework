# Kanban

A registered collection view. Columns come from the grouping field's enum values (or distinct values on the current page). Cards use `views.kanban.card` title, subtitle, and fields.

## Workflow drag-and-drop

If the grouping field is the workflow status, the frontend **does not PATCH `status`**.

```
Drag → find transition where to = destination → POST /api/v1/{slug}/{id}/transition
     → RBAC → workflow validation → transaction → audit → COMMIT → event
```

If no allowed transition exists, the board shows the server error (or `Cannot move {entity} from {from} to {to}.`) and reloads. Card buttons use the same `_workflow.transitions` / `_actions` metadata as the detail page.

Non-workflow grouping (a plain select) may `PATCH` that field; EntityService still enforces permissions.

## Loading

Kanban uses the list API with filters, search, and a larger page size. It does not load the entire table into React. Realtime SSE on the entity triggers a reload so cards move after a successful commit.
