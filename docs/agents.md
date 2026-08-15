# Agents

Qefro generates a tool per entity operation. Tools never receive a database connection.

## Registry

Example:

```json
{
  "name": "create_reservation",
  "description": "Create a restaurant reservation",
  "entity": "Reservation",
  "operation": "create",
  "input_schema": {},
  "required_permissions": ["reservation.create"]
}
```

Operations: `create`, `get`, `find`, `update`, `delete`, `transition` when the entity has a workflow, and every registered business operation (`confirm_reservation`, `seat_customer`, …). Tool JSON schemas are generated from `OperationDef`.

Example:

```json
{
  "name": "confirm_reservation",
  "description": "Confirm a pending restaurant reservation",
  "entity": "Reservation",
  "operation": "confirm",
  "input_schema": {},
  "required_permissions": ["reservation.confirm"]
}
```

## Discovery

```
GET /api/v1/tools
GET /api/v1/agent/tools
```

Both return tools the authenticated user may invoke in the current tenant. The list is already permission-filtered. Invoke still goes through `EntityService`.

```
POST /api/v1/agent/tools/{name}/invoke
```

## Pipeline

```
Agent
  ↓
Tool Registry
  ↓
EntityService / OperationService
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
Business Operation
  ↓
Audit
  ↓
Event
```

The agent cannot:

- access PostgreSQL directly (`qefro-agent` has no SQLx)
- bypass permissions
- bypass workflow
- specify an arbitrary tenant
- execute arbitrary SQL

`qefro tools` prints the generated names for the selected app.
