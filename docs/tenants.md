# Tenant customization

Each tenant is an isolated workspace on a shared Qefro runtime. Entity names stay stable. Presentation, enabled applications, and business locale are configuration.

```
Tenant
 ├── identity          id, name, slug
 ├── branding          company name, logo, favicon, colors
 ├── ui_config         navigation, dashboard, terminology
 ├── enabled_apps      intersection of installed apps, plan, and tenant choice
 ├── feature_flags     simple enabled/disabled map
 └── business          timezone, locale, currency, date/number formats
```

Stored in `tenant_settings`. Typed columns/JSON: branding, ui_config, enabled_apps, business_config, feature_flags, plan. Empty `enabled_apps` means every globally installed app the plan allows.

## APIs

All routes use the authenticated session tenant. `X-Tenant-ID`, `tenant_id` in JSON, and `tenant_id` query parameters are ignored or rejected.

| Method | Path | Who |
| --- | --- | --- |
| GET | `/api/v1/tenant` | any member |
| PATCH | `/api/v1/tenant` | Admin |
| GET/PATCH | `/api/v1/tenant/branding` | GET any, PATCH Admin |
| GET/PATCH | `/api/v1/tenant/apps` | GET any, PATCH Admin |
| GET/PATCH | `/api/v1/tenant/features` | GET any, PATCH Admin |
| GET/PATCH | `/api/v1/tenants/me/config` | GET any, PATCH Admin |
| GET | `/api/v1/tenants` | current tenant only |
| GET | `/api/v1/meta/ui` | filtered entities + branding + locale |

`GET /api/v1/tenants` never lists other tenants.

## Branding and white-label

The generic frontend loads branding after login. There is one React app. Company name, colors, logo, and favicon replace platform chrome in the tenant shell. The pre-login screen stays a generic sign-in page.

## Navigation and dashboards

Applications contribute entities and `DashboardDef`s. Tenant `ui_config.navigation` orders slugs. `default_dashboard` selects which dashboard to show. Disabled application modules are omitted from `/meta/ui` and `/meta/dashboards`. This is not a page builder.

## Terminology

`ui_config.terminology` maps entity names to labels (`Reservation` → `Booking`). The entity type and API slug do not change. `/meta/ui` applies the map before the frontend renders.

## Locale

`business.timezone`, `locale`, `currency`, `date_format`, and `number_format` are stored per tenant and copied onto `OpContext`. Server timestamps remain `TIMESTAMPTZ` (UTC). Business code that needs a local day boundary should use `ctx.timezone`, not the host timezone.

## Plans (no billing)

`Entitlements` resolves enabled apps as:

```
installed ∩ tenant.enabled_apps ∩ plan.apps
```

Empty plan apps (Enterprise) allow every installed application. Empty tenant `enabled_apps` means all installed apps the plan allows. Default plan is Enterprise until a billing product assigns Starter/Growth. No payment provider is integrated.
