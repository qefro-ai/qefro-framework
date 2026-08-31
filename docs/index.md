# Qefro documentation

Qefro is a Rust-native, metadata-driven framework for building secure, multi-tenant business applications. One entity definition produces PostgreSQL schema, REST APIs, validation, a generic UI, workflows, reports, documents, automation, realtime, integrations, and agent tools.

**App developers start here:** [App Developer Guide](developer-guide.md)

You do not write a React page, REST controller, or SQL migration per entity. Define the business in YAML or Rust; the runtime generates the rest. HTTP, the generic UI, the CLI, and agents all go through `EntityService`. Authorization always runs on the server.

```
Entity YAML / EntityDef
        │
        ├─ schema, REST, validation
        ├─ generic UI (List · Cards · Kanban · Calendar · Form · Detail)
        ├─ workflows, permissions, reports, dashboards
        ├─ documents, numbering, print, attachments
        ├─ automation, notifications, webhooks, realtime
        └─ agent tools
                │
                ▼
         EntityService
```

## Learning path

| Step | Doc |
| --- | --- |
| 1. Install CLI, Postgres, and the generic UI | [Getting started](getting-started.md) |
| 2. Scaffold a YAML app and run it | [Create an application](creating-an-app.md) |
| 3. Use every feature while building | [App Developer Guide](developer-guide.md) |
| 4. Customers, products, and orders walkthrough | [Build a fullstack application](fullstack.md) |
| 5. YAML vs Rust, when you need handlers | [App development](app-development.md) |
| 6. Ship, enable for a tenant, update | [App packaging](app-packaging.md), [App lifecycle](app-lifecycle.md) |
| 7. Production | [Deployment](deployment.md), [Configuration](configuration.md) |

Example apps: [restaurant, CRM, inventory, helpdesk](examples.md).

## Feature catalog

Each row is a feature you can use in an application. The developer guide explains how to define it; the linked doc is the full reference.

### Data model

| Feature | What you get | Doc |
| --- | --- | --- |
| **Entities** | One definition → table, REST, UI, audit, tools | [entities.md](entities.md) |
| **Fields & types** | `string`, `text`, `integer`, `decimal`, `boolean`, `date`, `time`, `datetime`, `uuid`, `enum`, `json`, `relation`, `child_table` | [entities.md](entities.md) |
| **Widgets** | Presentation independent of storage (`currency`, `email`, `status`, `relation`, …) | [ui-widgets.md](ui-widgets.md), [widgets.md](widgets.md) |
| **Relations** | many-to-one pickers, one-to-many related lists, many-to-many | [entities.md](entities.md) |
| **Child tables** | Nested line items in one transaction | [child-tables.md](child-tables.md) |
| **Validation** | Field rules, uniqueness, conditional `when` / `require` / `compare` | [validation.md](validation.md) |
| **Formulas** | Server-computed fields (`SUM(items.amount)`, `quantity * rate`) | [formulas.md](formulas.md) |
| **Singletons** | One settings document per tenant | [singletons.md](singletons.md) |
| **Identity** | Person ≠ User ≠ Organization ≠ Customer | [identity.md](identity.md) |

### Business logic

| Feature | What you get | Doc |
| --- | --- | --- |
| **Workflows** | Named state machines; status is not PATCHed | [workflows.md](workflows.md) |
| **Business operations** | Multi-record transactions (`OperationHandler` in Rust) | [operations.md](operations.md) |
| **Permissions** | Server-side RBAC on every REST, CLI, and agent path | [permissions.md](permissions.md) |
| **Field permissions** | Strip / reject individual fields by level | [field-permissions.md](field-permissions.md) |
| **Documents** | Submit / cancel / lock states on the same entity | [documents.md](documents.md) |
| **Numbering** | Tenant-safe sequences (`INV-{YYYY}-{#####}`) | [numbering.md](numbering.md) |
| **Allow on submit** | Fields still writable after lock | [allow-on-submit.md](allow-on-submit.md) |
| **Actions & links** | Metadata buttons and related lists | [actions-links.md](actions-links.md) |
| **Tasks** | Opt-in follow-ups on any record | [tasks.md](tasks.md) |

### User interface

| Feature | What you get | Doc |
| --- | --- | --- |
| **Generic UI** | One frontend for every app; no per-entity React pages | [ui.md](ui.md), [ui-2.md](ui-2.md) |
| **List views** | Columns, filters, sort, bulk export, grouping | [list-views.md](list-views.md) |
| **Card / Kanban / Calendar / Chart** | Registered views from `views:` metadata | [views.md](views.md), [kanban.md](kanban.md), [calendar.md](calendar.md) |
| **Forms** | Create/edit from widgets, sections, `visible_when` | [forms.md](forms.md) |
| **Layouts** | Tabs, sections, column widths | [layouts.md](layouts.md) |
| **Detail views** | Document header, children, related, attachments, activity | [detail-views.md](detail-views.md) |
| **Saved views** | Per-user filters, sort, and view type | [views.md](views.md) |
| **Workspaces** | Navigation, default dashboard, shortcuts | [workspaces.md](workspaces.md) |
| **Theming** | Tenant branding; no arbitrary CSS/JS | [theming.md](theming.md) |
| **Accessibility** | Shared shell, keyboard, and empty/error states | [accessibility.md](accessibility.md) |

### Documents, reports, and print

| Feature | What you get | Doc |
| --- | --- | --- |
| **Print formats** | HTML / PDF from metadata | [print-formats.md](print-formats.md) |
| **Reports** | Grouped aggregates without SQL | [reports.md](reports.md), [studio-reports.md](studio-reports.md) |
| **Dashboards** | KPI, chart, list, activity, audit cards | [dashboards.md](dashboards.md), [studio-dashboards.md](studio-dashboards.md) |
| **Dashboard drill-down** | Metric cards open the filtered list | [dashboard-drilldown.md](dashboard-drilldown.md) |

### Platform primitives

| Feature | What you get | Doc |
| --- | --- | --- |
| **Attachments** | Files on opted-in entities | [attachments.md](attachments.md) |
| **Activity** | Business timeline and comments | [activity.md](activity.md), [timeline.md](timeline.md) |
| **Audit** | Admin-only security log | [audit.md](audit.md) |
| **Search** | Global and list `ILIKE` over `searchable` fields | [search.md](search.md) |
| **CSV import** | Preview → validate → `EntityService::create` | [imports.md](imports.md) |
| **Public forms** | Unauthenticated intake (`/p/{tenant}/{form}`) | [public-forms.md](public-forms.md) |
| **Notifications** | In-app (and email job) from events | [notifications.md](notifications.md) |
| **Realtime** | SSE after COMMIT | [realtime.md](realtime.md) |

### Automation and integrations

| Feature | What you get | Doc |
| --- | --- | --- |
| **Events** | Outbox facts after COMMIT | [events.md](events.md) |
| **Jobs** | Postgres queue; no Redis/Kafka | [jobs.md](jobs.md) |
| **Automation** | `AutomationDef` rules on the same event path | [automation.md](automation.md) |
| **Webhooks** | HMAC-signed outbound HTTP, at-least-once | [webhooks.md](webhooks.md) |
| **Agents** | Generated tools; never a database connection | [agents.md](agents.md) |
| **Connectors / SDK** | REST + `QefroClient` + `EntityOps` | [connectors.md](connectors.md), [sdk.md](sdk.md) |

### Studio

| Feature | What you get | Doc |
| --- | --- | --- |
| **Studio** | Inspect and publish metadata overlays | [studio.md](studio.md) |
| **Studio entities / workflows / permissions** | Same registries as the runtime | [studio-entities.md](studio-entities.md), [studio-workflows.md](studio-workflows.md), [studio-permissions.md](studio-permissions.md) |
| **Studio publishing** | Validate, overlay, additive migrate | [studio-publishing.md](studio-publishing.md) |

### Apps, tenants, and operations

| Feature | What you get | Doc |
| --- | --- | --- |
| **Applications** | Versioned packages, not hardcoded restaurant/CRM | [apps.md](apps.md) |
| **App dependencies** | Named semver requirements | [app-dependencies.md](app-dependencies.md) |
| **Multi-tenancy** | `tenant_id` from the session, never the client | [multitenancy.md](multitenancy.md), [tenants.md](tenants.md) |
| **Licenses / entitlements** | Installed ∩ tenant.enabled ∩ plan | [licenses.md](licenses.md) |
| **API** | `/api/v1` generated from metadata | [api.md](api.md) |
| **CLI** | `qefro app`, `migrate`, `dev`, `inspect`, … | [creating-an-app.md](creating-an-app.md) |
| **Configuration** | Environment variables | [configuration.md](configuration.md) |
| **Deployment** | One HTTP process, one worker, one Postgres | [deployment.md](deployment.md) |
| **Security** | Pipeline, threat model | [security.md](security.md), [threat-model.md](threat-model.md) |

## Architecture and compatibility

- [Architecture](architecture.md) — modular monolith, metadata as source of truth
- [Business object runtime](business-object-runtime.md) — `EntityService` execution boundary
- [UI schema](ui-schema.md) — additive `schema_version: "1"`
- [View metadata](view-metadata.md) — `views.list` / `kanban` / `calendar` / `chart`
- [V1 compatibility](v1-compatibility.md)
- [Release](release.md)
- [ADRs](adr.md)
- [Benchmarks](benchmarks.md)
- [Operations (business)](operations.md)

## Examples

| App | Kind | Notes |
| --- | --- | --- |
| [Restaurant](examples.md) | Rust + catalog `app.toml` | Reservations, tables, orders; `OperationHandler`s |
| [CRM](examples.md) | Rust + catalog `app.toml` | Lead convert, opportunities |
| [Inventory](examples.md) | YAML | Stock documents, child tables, reports |
| [Helpdesk](examples.md) | YAML | Tickets, public form, kanban |
