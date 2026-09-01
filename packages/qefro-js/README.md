# @qefro/js

Qefro UI runtime: the reusable frontend for metadata-driven Qefro applications.

`@qefro/js` is **not** a generic React component library. It translates:

```
Qefro metadata + application extensions + theme + permissions + runtime state
        → application UI
```

It is **not** a security boundary. Hidden buttons, disabled fields, and frontend permission checks are UX only. Mutations still go through:

```
authenticated session → tenant context → RBAC / RowPolicy → EntityService
```

Restaurant, CRM, Estate, School, Healthcare — and any other Qefro app — should use the same runtime. Application-specific screens (a property map, a custom Property card) register as extensions. They do not belong inside this package.

The Qefro frontend in this repository is the reference application: it consumes `@qefro/js` and keeps Qefro Studio as platform tooling.

## 1. Installation

This package lives in the Qefro monorepo. From the generic UI:

```bash
cd frontend
npm install
```

`frontend` depends on `@qefro/js` via `file:../packages/qefro-js`. Peer dependencies are React 19 and React Router 7.

```ts
import { Qefro } from "@qefro/js";
import "@qefro/js/styles.css";
```

## 2. Initialization

```ts
import { Qefro } from "@qefro/js";

const qefro = new Qefro({
  apiUrl: "/api/v1",
});

await qefro.init();
```

`apiUrl` is the API root **including version**. The default is `/api/v1`, matching `QefroClient`. Do not add a second HTTP client.

The reference app wires the runtime like this:

```tsx
<QefroProvider runtime={qefro} snapshot={snapshot}>
  <AppShell ... extraNav={qefro.extensions.navigation}>
    <QefroRoutes entities={entities} config={config} ... />
  </AppShell>
</QefroProvider>
```

Existing routes stay the same:

```
/:slug
/:slug/new
/:slug/:id
/:slug/:id/edit
```

Plus `/`, `/settings`, `/reports`, `/pages/:name`. Unauthenticated visitors only see login and public forms.

## 3. UI primitives

These are the design-system pieces the runtime actually uses. They are not empty wrappers.

```ts
qefro.ui.button      // Button
qefro.ui.card        // Card
qefro.ui.dialog      // ConfirmDialog
qefro.ui.tabs        // Tabs
qefro.ui.toast("Booking created")
qefro.ui.notify("Saved", "info")
```

Tables, filters, fields, and actions are the metadata-driven views below — not a second set of toys.

## 4. Entity UI

The important API. Names resolve from metadata (entity name, slug, or label):

```ts
qefro.entity("Lead")
qefro.entity("Property")
qefro.entity("Booking")

qefro.ui.list("Lead")
qefro.ui.form("Lead")
qefro.ui.detail("Lead")
```

There is no `LeadPage.tsx` in the runtime. The generic renderer reads fields, relations, permissions, views, actions, and workflow from `GET /api/v1/meta/ui`.

## 5. Forms

```ts
qefro.ui.form("Lead")
```

The form uses the widget registry (`text`, `textarea`, `number`, `currency`, `boolean`, `date`, `datetime`, `select`, `multiselect`, `relation`, `attachment`, `image`, `rich text`, `formula`, `status`, `user`, `child table`, …). Required flags, `visible_when`, `readonly_when`, defaults, and layout sections come from metadata.

Frontend checks are UX only. `EntityService` validation remains authoritative. 422 responses surface as field errors.

## 6. Lists

```ts
qefro.ui.list("Property", { view: "table" })
qefro.ui.table("Property")
```

`view` maps onto the existing view registry: `list` (table), `card`, `kanban`, `calendar`, `chart`. `"table"` is an alias for `list`; `"cards"` / `"compact"` alias `card`.

Metadata controls columns, default sort, page size, grouping, and which views are enabled. Search, filters, saved views, bulk actions, row actions, and column prefs are the same generic list as before.

## 7. Detail views

```ts
qefro.ui.detail("Booking")
```

Header, status, sections/tabs, related records, activity, timeline, attachments, actions, and workflow transitions. `_actions` from the record is already permission-filtered; the click still POSTs `/api/v1/{slug}/{id}/actions/{name}` and the server re-checks.

## 8. Dashboards

```ts
qefro.ui.dashboard("sales")
```

Uses the existing dashboard metadata (`GET /api/v1/meta/dashboards` and `GET /api/v1/dashboards/{name}`). Widget kinds already in Qefro include metric/KPI, chart, list/table, activity, workflow, and saved views. Register extra widget types:

```ts
qefro.register({
  widget: { name: "funnel", component: FunnelWidget },
});
```

## 9. Workspace

```ts
qefro.ui.workspace() // AppShell
```

Application name, logo, grouped navigation, shortcuts, search (command palette), notifications, profile, theme, and density. Tenant branding (logo, colors, favicon, terminology) still comes from `/api/v1/meta/ui` and `/api/v1/tenants/me/config`.

## 10. Themes

```ts
qefro.theme({
  primary: "#2563eb",
  accent: "#2563eb",
  radius: "medium",
  density: "comfortable",
  mode: "light",
});
```

Tokens live in CSS (`--qefro-*`, spacing, type, radius, shadow, z-index, breakpoints). Tenant branding **overrides** application defaults. Arbitrary tenant CSS/JS is still rejected.

Light mode is the default chrome (light navigation, dense workspace). Dark mode remains a user preference.

## 11. Custom pages

```ts
qefro.page("property-map", {
  component: PropertyMap,
  path: "/property-map", // optional; default /pages/property-map
  nav: true,
  label: "Map",
});
```

The page renders inside the authenticated workspace (`QefroRoutes` under `AppShell`). It does not bypass session, tenant, or EntityService.

## 12. Custom components

```ts
qefro.ui.extend("Property", {
  card: PropertyCard,
  list: PropertyBoard,
  form: PropertyForm,
  detail: PropertyDetail,
  header: PropertyHeader,
  field: PropertyField,
});
```

If a registration exists, it renders. Otherwise the generic renderer runs. Custom components still receive entity metadata, including `permissions`. Showing a Delete button in a custom card does not authorize the delete.

## 13. Extensions

Keep this small:

```ts
qefro.register({
  page: { name: "property-map", component: PropertyMap },
  widget: { name: "funnel", component: FunnelWidget },
  field: { name: "plot", component: PlotWidget },
  entity: { name: "Property", card: PropertyCard },
  navigation: { label: "Map", to: "/property-map" },
  action: { entity: "Lead", name: "qualify", label: "Qualify" },
  theme: { name: "estate", primary: "#1d4ed8" },
});
```

UI events (frontend only — not backend event bus):

```ts
qefro.on("entity:created", ({ entity, id }) => { ... });
qefro.on("entity:updated", handler);
qefro.on("entity:deleted", handler);
qefro.on("route:change", handler);
qefro.on("workspace:ready", handler);
```

## 14. Examples

### Reference app (this repo)

`frontend/` loads metadata, applies tenant branding, and renders the workspace with `@qefro/js`. Studio stays in the application (`frontend/src/studio`).

### Qefro Estate (application, not framework)

Estate-specific UI lives in the Estate app. The runtime stays generic:

```ts
import { Qefro } from "@qefro/js";
import { PropertyCard } from "./estate/PropertyCard";
import { PropertyMap } from "./estate/PropertyMap";

const qefro = new Qefro({ apiUrl: "/api/v1" });
await qefro.init();

qefro.theme({ primary: "#1d4ed8", radius: "medium" });

qefro.ui.extend("Property", { card: PropertyCard });
qefro.page("property-map", { component: PropertyMap, nav: true, label: "Map" });

qefro.ui.workspace();
qefro.ui.list("Property");
qefro.ui.form("Lead");
qefro.ui.detail("Booking");
qefro.ui.dashboard("Sales");
qefro.ui.toast("Booking created");
```

Projects, Properties, Leads, Customers, Site Visits, Offers, Bookings, Sales, Payments, and Commissions all use generic list/form/detail unless the Estate app registers an override.

## Security reminder

Never treat hidden UI as authorization. `@qefro/js` has no database access. Every create, update, delete, action, and workflow transition uses `QefroClient` → `/api/v1` → `EntityService`.
