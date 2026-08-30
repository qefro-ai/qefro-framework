# Business Object Runtime (Qefro 1.2)

Qefro 1.2 makes an `EntityDef` a richer **business object** without a second entity system, workflow engine, event bus, agent runtime, or UI API.

```
                         Entity
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
     Identity          Relations           Workflow
        │                  │                  │
        └──────────────────┼──────────────────┘
                           ▼
                    Business Object
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
     Activity            Audit            Attachments
        │                  │                  │
        └──────────────────┼──────────────────┘
                           ▼
                    Notifications
                           │
                           ▼
                  Generic Qefro UI
                           │
                  ┌────────┴────────┐
                  ▼                 ▼
             QefroClient        EntityOps
                  │                 │
                 UI               Agents
```

```
EntityDef
   ↓
EntityService
   ├── CRUD
   ├── Workflow
   ├── Activity
   ├── Audit
   ├── Attachments
   └── Notifications
          ↓
      QefroClient
          ↓
       Generic UI

Agent
   ↓
EntityOps
   ↓
EntityService
```

**Define the business object once.** Qefro provides the runtime, API, UI, workflow, identity, relationships, activity, audit, files, notifications, search, reports, dashboards, workspaces, and agent access around it.

```
EntityDef
  ↓
EntityService
  ├── CRUD
  ├── Workflow
  ├── Activity
  ├── Audit
  └── Business capabilities
        ↓
Reports / Search / Dashboards
        ↓
QefroClient
        ↓
Generic UI
```

Agents continue `EntityOps → EntityService`. There is no second query engine.

See [Search](search.md), [Saved views](views.md), [Reports](reports.md), [Dashboards](dashboards.md), [Workspaces](workspaces.md).

Capabilities are discovered from metadata (`EntityDef` → `capabilities` on `GET /meta/ui`) and record payloads (`_workflow`, `_actions`, `_related`). The generic UI never branches on `if entity === "Customer"`.

Entities that omit workflow, activity, attachments, or comments continue to work. `UI_SCHEMA_VERSION` remains `"1"`.

See [Identity](identity.md), [Tasks](tasks.md), [Files](files.md), [Activity](activity.md), [Audit](audit.md), [Workflows](workflows.md), [Attachments](attachments.md), [Notifications](notifications.md), [Automation](automation.md), [Validation](validation.md).
