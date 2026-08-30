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

Operations: `create`, `get`, `find`, `update`, `delete`, `transition` when the entity has a workflow, `list_activity` / `comment` when activity is enabled, `list_attachments` when attachments are enabled, workspace tools `search` / `run_report` / `get_dashboard`, and every registered business operation. Tool JSON schemas are generated from `OperationDef`. Agents obey the same tenant, RBAC, entity permissions, and workflow rules as human users. There is no agent bypass.

When an agent mutates a record, Activity shows **Qefro Agent** as the actor. Internal reasoning is never stored.

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

Both return tools the authenticated user may invoke in the current tenant. The list is filtered by CRUD permission **and** `OperationDef.roles` (Staff does not see Manager-only tools). Invoke still goes through `EntityService` and re-checks roles. Knowing a tool name does not make it executable.

```
POST /api/v1/agent/tools/{name}/invoke
```

## Pipeline

```
Agent
  ↓
Tool Registry
  ↓
EntityOps (in-process adapter)
  ↓
EntityService
  ↓
Authentication → Tenant → RBAC → Validation → Workflow → Operation → Audit → Event
```

The browser uses `QefroClient` → REST → `EntityService`. Agents never get a SQLx pool. See [sdk.md](sdk.md).

The agent cannot:

- access PostgreSQL directly (`qefro-agent` has no SQLx; Cargo.lock is tested for this)
- bypass permissions or operation roles
- bypass workflow
- specify an arbitrary tenant (`tenant_id` in tool input is rejected)
- execute arbitrary SQL

Agents are not principals. They run as the authenticated user. There is no `agent_admin` role.

`qefro tools` prints the generated names for the selected app.
