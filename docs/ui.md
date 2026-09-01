# UI

The backend exposes UI metadata at `GET /api/v1/meta/ui`. The payload is versioned (`schema_version: "1"`). The React renderer is `@qefro/js`; `frontend/` is the reference app that consumes it. It does not hardcode entity names. **UI 2.0** polishes that renderer (shell, lists, documents, theme) without changing `schema_version: "1"`. See [UI 2.0](ui-2.md) and [qefro.js](qefro-js.md).

## Routes

From `EntityDef::new("Customer")`:

```
/customers
/customers/new
/customers/:id
/customers/:id/edit
```

Reserved paths: `/`, `/login`, `/settings`, `/reports`, `/pages/:name`. Dashboard cards come from `GET /api/v1/meta/dashboards` and `GET /api/v1/dashboards/{name}`. Composed pages come from `GET /api/v1/meta/pages/{name}` — see [Pages](pages.md).

## Widget registry

`field.widget` is a string. The frontend looks it up in `registerWidget`. Data type and widget are separate: `decimal` + `currency`, `string` + `color`. See [ui-widgets.md](ui-widgets.md), [forms.md](forms.md), and [layouts.md](layouts.md).

Relation fields render a searchable, paginated picker against the target entity's list API (tenant-scoped). The list view shows `_expanded.{field}.label` instead of a UUID.

## Workflow and operation actions

Detail pages prefer `_actions` from the record (already filtered to operations the current user may run in the current state). Clicking an action POSTs `/api/v1/{slug}/{id}/actions/{name}`. If `_actions` is empty, the page falls back to `_workflow.transitions`.

The server re-checks authentication, tenant, permission, workflow, and business rules. The UI never authorizes.

## Tenant branding

The generic frontend loads `/api/v1/meta/ui` and `/api/v1/tenant`. Company name, colors, logo, favicon, navigation, terminology, locale, timezone, currency, enabled apps, and the selected dashboard come from tenant configuration. CSS variables (`--accent`, `--primary`) theme the shared components. There is one frontend. Disabled applications are omitted by the server, not merely hidden in CSS.

`GET/PATCH /api/v1/tenants/me/config` and `/api/v1/tenant/*` store that configuration. PATCH is Admin-only. This is not a visual page builder.

## Restaurant vs CRM

The same Dashboard, list, form, and detail pages render restaurant or CRM metadata. No restaurant-specific or CRM-specific frontend architecture. `UiShowcase` in the restaurant app exercises the full widget set without custom React pages.
