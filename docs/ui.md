# UI

The backend exposes UI metadata at `GET /api/v1/meta/ui`. The React app in `frontend/` is a generic renderer. It does not hardcode entity names.

## Routes

From `EntityDef::new("Customer")`:

```
/customers
/customers/new
/customers/:id
/customers/:id/edit
```

Reserved paths: `/`, `/login`, `/settings`. Dashboard cards come from `GET /api/v1/meta/dashboards` and `GET /api/v1/dashboards/{name}`.

## Widgets

The widget registry currently includes:

`text`, `textarea`, `email`, `number`, `boolean`, `date`, `datetime`, `select`, `relation`

Additional widgets can be registered with `registerWidget`.

Relation fields render a searchable picker against the target entity's list API. The list view shows `_expanded.{field}.label` instead of a UUID.

## Workflow and operation actions

Detail pages prefer `_actions` from the record (already filtered to operations the current user may run in the current state). Clicking an action POSTs `/api/v1/{slug}/{id}/actions/{name}`. If `_actions` is empty, the page falls back to `_workflow.transitions`.

The server re-checks authentication, tenant, permission, workflow, and business rules. The UI never authorizes.

## Tenant branding

`GET/PATCH /api/v1/tenants/me/config` stores logo, colors, favicon, navigation order, hidden entities, and enabled apps. PATCH is Admin-only. This is branding/navigation configuration, not a visual page builder.

## Restaurant dashboard

The restaurant app registers a `DashboardDef` with count/sum cards. The same Dashboard page renders CRM cards when that app is installed. No restaurant-specific frontend architecture.
