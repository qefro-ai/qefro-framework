# Tasks

**Framework Task / assignment / follow-up primitive**

Task is a normal Qefro business object. It is not a CRM feature, a workflow engine, or a notification product.

```
EntityDef (Task)
      ↓
EntityService
      ↓
REST · QefroClient · Generic UI · Search · Studio
```

Do not add `TaskService`, `TaskRepository`, `TaskController`, or a Task page. Applications opt a record in with `EntityDef::with_tasks()`.

```
Customer / Order / Lead / Ticket
        └── Task (title, assignee, due, workflow status)
```

## Entity

Platform `Task` (`/api/v1/tasks`) is registered with Person / User / Organization. Fields:

| Field | Role |
| --- | --- |
| `title` | Required, searchable |
| `description` | Optional, searchable |
| `status` | Workflow-managed: Open, In Progress, Completed, Cancelled |
| `priority` | `low` / `normal` / `high` / `urgent` |
| `due_at` | Optional datetime. Overdue is **derived**, not stored |
| `completed_at` | Set on Complete, cleared if the record leaves Completed |
| `assigned_to` | User in this tenant (`FieldDef::assigned_to()`). Defaults to the current user |
| `entity_type` / `entity_id` | Optional related record. Same convention as Activity |

`qefro inspect Task` and `qefro validate` see it as any other entity. `UI_SCHEMA_VERSION` stays `"1"`.

## Workflow

Status is not PATCHed. The existing workflow engine owns transitions:

```
Open
 ├── start     → In Progress
 ├── completed → Completed
 └── cancelled → Cancelled

In Progress
 ├── completed → Completed
 └── cancelled → Cancelled
```

```
POST /api/v1/tasks/:id/transition   { "transition": "start" }
```

The generic UI shows workflow actions. Kanban columns are Open / In Progress / Completed / Cancelled; dragging calls `transition`, never `PATCH status`.

Events: `task.created`, `task.assigned`, `task.completed`, `task.cancelled`, plus `entity.*` / `workflow.transitioned`. Outbox publishes after COMMIT.

## Assignment

`assigned_to` is a User id. EntityService checks:

- the user belongs to **this tenant** (`get_tenant_user`)
- the user is enabled
- assigning someone else requires `User.Read` (Staff can assign to themselves; Manager can assign teammates)

Cross-tenant assignment is rejected. There is no second employee model.

On assign, Activity records an assignment row and `task.assigned` / `entity.assigned` feed the existing notification pipeline (`recipients: assignee`).

## Related records

Task cannot hold typed FKs to every app entity. Related records use Activity’s polymorphic pair plus the existing `LinkDef` / FK-prefill path:

```rust
EntityDef::new("Customer")
    .field(FieldDef::string("name").required())
    .with_tasks()
    .build()
```

`with_tasks()` adds a virtual `tasks` one-to-many and a Related-panel link filtered by `entity_type = Customer`. **Add** opens:

```
/tasks/new?entity_id=<id>&entity_type=Customer
```

The generic create form prefills those query params. Get-task expands `entity_id` in `_expanded` so Detail can link **Open** without `if entity === Customer`.

Customer Activity may show “Task created” because Task writes a copy onto the related record through the existing Activity store. Comments and attachments are the generic entity features.

## Due dates and overdue

Overdue is:

```
status not in {Completed, Cancelled}  AND  due_at < now
```

The UI chip is Overdue / Due today / Due tomorrow. List and dashboard filters use `due_at.lt=now`, `due_at.gte=today`, placeholders `me` / `current_user` / `now`.

Reminders use JobQueue job `due.reminder` (not a Task scheduler). Create/update/transition enqueue with `run_at = due_at` and `idempotency_key = due:Task:{id}:{due_at}`. The worker-safe handler no-ops when the row is missing, completed, cancelled, or the due timestamp changed. Outbox ids are deterministic so retries do not duplicate `task.due` / `entity.due`.

## Permissions and row policy

`PermissionRegistry` grants (Admin via `ensure_admin`):

| Role | Actions |
| --- | --- |
| Manager | Create, Read, Update, Delete, List, Export |
| Staff | Create, Read, Update, List |

Task does **not** enable `row_policy` by default so a manager still sees the team queue. `RowPolicy::AssignedTo`, `CreatedBy`, and `AssignedToOrCreatedBy` exist for apps that want a personal inbox. List `assigned_to=me` / `created_by=me` is the saved-view / query equivalent of “My Tasks”.

## Automation, search, UI

Default `AutomationDef`s stay generic: notify the assignee on `task.created` when `assigned_to` is set; write activity on complete/cancel via `workflow.transitioned`. Apps add more automations the usual way.

Title and description are searchable. Command palette and global search use the existing search index. List, cards, kanban, and calendar come from `EntityViews`. Studio edits Task like any entity.

Dashboard cards (My open tasks, Overdue, Due today) are `DashboardDef` configuration, not a Task dashboard system.

## Restaurant and CRM

Restaurant `Customer` and `Order`, and CRM `CrmCustomer`, call `.with_tasks()`. Example:

```
Order → Add Task → "Verify special dietary request"
Customer → Add Task → "Call customer"
```

No restaurant- or CRM-specific Task code lives in framework crates.

## What this is not

AI, agents, a second workflow engine, event bus, notification system, or UI framework. Task composes EntityService, Workflow, Activity, Audit, Notification, JobQueue, Automation, Permissions, Search, and the generic renderer.
