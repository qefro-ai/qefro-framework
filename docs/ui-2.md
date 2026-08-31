# Qefro UI 2.0 / 2.1

Qefro **1.0.0** is the production-hardened backend. **UI 2.0** polished the generic React renderer. **UI 2.1** is a metadata-driven **Business Views Engine** (List, Cards, Kanban, Calendar, Form, Detail) on the same architecture. It does not replace EntityService, RBAC, tenant isolation, workflows, Studio, or the REST API.

```
Entity Metadata → UI Schema (schema_version: "1") → View Registry → Generic React Renderer → EntityService
```

Developers customize screens through **metadata, `registerWidget`, and `registerView`**, not per-entity React pages. If a new entity makes you reach for a custom page, the generic UI is not finished.

## What UI 2.0 adds

The V1 generic UI already had lists, forms, dashboards, widgets, filters, search, notifications, and Studio preview. UI 2.0 polishes that same renderer:

- Application shell (collapsible nav, top bar, breadcrumbs, command palette, user menu)
- Workspace homepage from dashboard metadata
- Rich lists (columns, saved views, export, bulk delete via EntityService)
- Filter builder with date presets (translated to `between`, never SQL)
- Document detail header, related records, timeline, attachments
- Theme, density, and table preferences scoped to tenant + user (+ entity)
- Dark / system theme without tenant CSS or JavaScript injection

## What UI 2.1 adds

- View registry (`list`, `card`, `kanban`, `calendar`, plus custom registrations)
- Automatic view detection from workflow / date fields, overridable with `views:`
- Opt-in Cards view when `views.card` is present (`enabled: false` hides it)
- Kanban drag that calls workflow **transitions**, never PATCHes status
- Calendar day/week/month with tenant timezone, slot-create, and EntityService reschedule
- Dashboard metric drill-down and dashboard-level filters
- List grouping, numeric footers, detail workflow strip, section `visible_when`
- Permission chrome hints (`permissions` on `/meta/ui`, `_permissions` on GET)
- Browser SDK: `QefroClient` in `frontend/src/sdk/client.ts` (see [sdk.md](sdk.md))

## Non-negotiable

React presents and interacts. The server remains authoritative for permissions, tenant, workflow, validation, and calculations.

Studio preview uses the **same** `FormLayout`, widget registry, and view registry as production. Overlays are presentation only.

## Docs in this set

- [QefroClient](sdk.md)
- [Views](views.md)
- [View metadata](view-metadata.md)
- [Kanban](kanban.md)
- [Calendar](calendar.md)
- [Dashboard drill-down](dashboard-drilldown.md)
- [UI schema](ui-schema.md)
- [Components](ui-components.md)
- [Widgets](widgets.md)
- [List views](list-views.md)
- [Forms](forms.md)
- [Detail views](detail-views.md)
- [Workspaces](workspaces.md)
- [Pages](pages.md)
- [Timeline](timeline.md)
- [Accessibility](accessibility.md)
- [Theming](theming.md)

See also [UI](ui.md), [UI widgets](ui-widgets.md), and [Layouts](layouts.md).


Qefro **1.0.0** is the production-hardened backend. **UI 2.0** is the generic React renderer on top of that architecture. It does not replace EntityService, RBAC, tenant isolation, workflows, Studio, or the REST API.

```
Entity Metadata → UI Schema (schema_version: "1") → Generic React Renderer → EntityService
```

Developers customize screens through **metadata and `registerWidget`**, not per-entity React pages. If a new entity makes you reach for a custom page, the generic UI is not finished.

## What UI 2.0 adds

The V1 generic UI already had lists, forms, dashboards, widgets, filters, search, notifications, and Studio preview. UI 2.0 polishes that same renderer:

- Application shell (collapsible nav, top bar, breadcrumbs, command palette, user menu)
- Workspace homepage from dashboard metadata
- Rich lists (columns, saved views, export, bulk delete via EntityService)
- Filter builder with date presets (translated to `between`, never SQL)
- Document detail header, related records, timeline, attachments
- Theme, density, and table preferences scoped to tenant + user (+ entity)
- Dark / system theme without tenant CSS or JavaScript injection

## Non-negotiable

React presents and interacts. The server remains authoritative for permissions, tenant, workflow, validation, and calculations.

Studio preview uses the **same** `FormLayout` and widget registry as production (`frontend/src/studio/preview/FormPreview.tsx`).

## Docs in this set

- [UI schema](ui-schema.md)
- [Components](ui-components.md)
- [Widgets](widgets.md)
- [List views](list-views.md)
- [Forms](forms.md)
- [Detail views](detail-views.md)
- [Workspaces](workspaces.md)
- [Pages](pages.md)
- [Timeline](timeline.md)
- [Accessibility](accessibility.md)
- [Theming](theming.md)

See also [UI](ui.md), [UI widgets](ui-widgets.md), and [Layouts](layouts.md).
