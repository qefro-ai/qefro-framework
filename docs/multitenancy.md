# Multi-tenancy

Every tenant-owned row includes `tenant_id`. That value is taken from the authenticated session, never from the client body, query string, or `X-Tenant-ID`. Supplying `tenant_id` in a create/update/action payload or as a filter returns 400.

## Isolation

- Tenant A cannot read Tenant B (404, not 403).
- Tenant A cannot update or delete Tenant B.
- Tenant A cannot invoke Tenant B's records through agent tools.
- Tenant A cannot read or modify Tenant B branding, feature flags, dashboards, or enabled apps.
- List queries always include `WHERE tenant_id = $1`.
- `GET /api/v1/tenants` returns the current tenant only.

See [Tenant customization](tenants.md), [Identity](identity.md), and [Security](security.md).

## Configuration

```
Tenant
 ├── branding        logo, colors, favicon, company name
 ├── ui_config       navigation, hidden entities, dashboard, terminology
 ├── enabled_apps    tenant choice ∩ plan ∩ globally installed apps
 ├── features        enabled/disabled flags
 └── business        timezone, locale, currency, formats
```

Stored in `tenant_settings` with a short in-memory cache keyed by `tenant_id`. This is the customization seam for a multi-tenant product without forking backends or frontend builds.

## Roles

Register creates an Admin for a new tenant. Admins create further Users through EntityService (`POST /api/v1/users` or the generic Users UI). A User is a login, not a Customer. See [Identity](identity.md).
