# Pages

A **page** is metadata that composes existing Qefro components into an operational workspace. It is not a second UI framework, a page-builder product, or a replacement for the generic entity UI.

```
EntityDef
   ↓
EntityService
   ↓
REST / SDK / generic UI
   ↓
┌──────────┬──────────┐
│ Entities │  Pages   │
└────┬─────┴────┬─────┘
     └─────┬────┘
           ↓
      Components
```

Pages obtain data through the same Entity REST, reports, dashboard widgets, and operations. There is no `GET /api/v1/page/{name}` data API.

## Concepts

| Concept | Role |
| --- | --- |
| **EntityDef** | Source of truth for fields, relations, permissions, workflow, validation |
| **View** | List, card, kanban, calendar, chart, form, detail on an entity |
| **Dashboard** | High-level KPIs, charts, and summaries |
| **Page** | Operational workspace that *embeds* those views, reports, widgets, and actions |
| **Operation** | Named business action executed through EntityService |
| **Report** | Grouped/aggregated query defined in metadata |

A dashboard is not a page. Keep home KPIs on `DashboardDef`. Put floor work (kanban, lists, actions, filters) on `PageDef`.

## Definition

```rust
AppModule::new("restaurant")
    .page(
        PageDef::new("restaurant-operations", "Restaurant Operations")
            .template("operations_dashboard")
            .layout("grid")
            .section(PageSection::widget_from("Today's Sales", "restaurant-ops", "Today's sales"))
            .section(PageSection::entity_view("Kitchen", "Order", "kanban").query("status=Preparing"))
            .section(PageSection::entity_view("Reservations", "Reservation", "list"))
            .action(PageActionRef::new("Order", "create").label("New Order")),
    )
    .nav(NavItem::page_link("Operations", "restaurant-operations").section("Operations"))
```

YAML bundles load the same `PageDef` from `pages/`:

```yaml
name: sales_workspace
label: Sales Workspace
layout:
  type: split
components:
  - entity: Opportunity
    view: kanban
  - entity: Task
    view: list
  - report: sales_pipeline
```

`components` is an alias for `sections`. Layout may be a string (`grid`) or `{ type: grid }`. Allowed layouts: `stack`, `two_column` / `2-column`, `three_column` / `3-column`, `grid`, `split`. There is no freeform canvas.

Templates (`operations_dashboard`, `sales_workspace`, `customer_workspace`) are starter layouts, not a marketplace.

## What a section may reference

| kind | Metadata | Renderer |
| --- | --- | --- |
| `entity_view` | `entity` + `view` | Existing collection views |
| `related` | `entity` + `relation` | Related list through existing inverse/query |
| `report` | `report` | Existing report run |
| `widget` | `dashboard` + `widget` (card title) or inline `card` | Existing dashboard cards |
| `activity` | `entity` + context id | Existing timeline |
| `attachments` | `entity` + context id | Existing attachment panel |
| `action` | `entity` + `action` | Existing ActionBar / operations |

Pages do not redefine fields, relations, permissions, workflow, or validation.

## Routing and URL state

The generic UI registers `/pages/:name` before `/:slug`. Navigation items with `page` link there automatically — you do not add a React route per workspace.

Query parameters reuse entity conventions:

- `?tab=activity`
- `?status=Preparing` (shared filters)
- `?id=` / context param for master-detail deep links (`/pages/customer-workspace?id=…`)

Selected tab, view, filters, and sort continue to use the existing saved-view mechanism when the embedded list supports it.

## Permissions and tenant isolation

- A page may list `roles`. That only gates the page chrome.
- Each section still requires `List` (and card `roles`) on its entity or report. Page access does not grant entity access.
- Unauthorized sections are omitted, same as dashboard cards.
- Backend authorization remains authoritative. Embedded lists call Entity REST, which applies tenant isolation and row policies.
- Studio rejects custom JavaScript, HTML, SQL, and arbitrary URLs.

## CLI

```bash
qefro inspect page sales_workspace
qefro inspect sales_workspace
qefro validate restaurant
```

Inspect prints layout, components, permissions, and the `/pages/…` route. Validate reports unknown components, entities, reports, views, actions, layouts, relations, and duplicate routes.

## Studio

Studio → Analytics → Pages. Pick existing entities, views, reports, and dashboard widgets. Publish goes through the same overlay catalog as dashboards (`kind: page`).

## Examples

The restaurant app ships **Restaurant Operations** (KPI widgets, kitchen kanban, reservations, tables, activity) and a **Customer Workspace** split view. The CRM app ships **Sales Workspace** (revenue widget, pipeline kanban, tasks, customers, pipeline report).
