# Multi-tenancy

Every tenant-owned row includes `tenant_id`. That value is taken from the authenticated session, never from the client body or query string. Supplying `tenant_id` in a create/update payload returns 400.

## Isolation

- Tenant A cannot read Tenant B (404, not 403).
- Tenant A cannot update or delete Tenant B.
- Tenant A cannot invoke Tenant B's records through agent tools.
- List queries always include `WHERE tenant_id = $1`.

## Tenant configuration

```
Tenant
 ├── branding        logo, colors, favicon, app name
 ├── ui_config       navigation, hidden entities, default dashboard
 ├── enabled_apps
 └── business_config JSON bag for later SaaS flags
```

Stored in `tenant_settings`. This is the customization seam for a multi-tenant product without forking backends. V0.2 implements branding and navigation only.

## Roles

Register creates an Admin for a new tenant. Admins can `POST /api/v1/users` to add Staff/Manager members. RBAC is evaluated in `EntityService` for both REST and tools.
