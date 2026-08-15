# API

Base URL: `/api/v1`

## Auth

| Method | Path | Auth |
| --- | --- | --- |
| POST | `/auth/register` | no |
| POST | `/auth/login` | no |
| POST | `/auth/logout` | yes |
| GET | `/auth/me` | yes |
| POST | `/auth/switch-tenant` | yes |
| POST | `/users` | Admin |

Register body: `{ name, email, password, tenant_name, tenant_slug }`

Create user body: `{ name, email, password, roles }`

## Tenants

`GET/POST /tenants`

`GET/PATCH /tenants/me/config` — branding and navigation. PATCH is Admin-only.

## Metadata

- `GET /meta/entities`
- `GET /meta/entities/{name}`
- `GET /meta/ui`
- `GET /meta/permissions`
- `GET /meta/workflows`
- `GET /meta/modules`
- `GET /meta/dashboards`
- `GET /dashboards/{name}`
- `GET /api/openapi.json`
- `GET /docs`

## Entities

Generated per registered entity slug:

```
GET    /{slug}?search=&status=&sort=-created_at&page=1&page_size=25
POST   /{slug}
GET    /{slug}/{id}
PATCH  /{slug}/{id}
DELETE /{slug}/{id}
GET    /{slug}/{id}/workflow
POST   /{slug}/{id}/transition   { "transition": "confirm" }
GET    /{slug}/{id}/actions
POST   /{slug}/{id}/actions/{name}
```

List/get include `_expanded` (many-to-one labels), `_related` (one-to-many, GET only), `_workflow` (allowed transitions), and `_actions` (allowed business operations).

```
GET /operations
```

returns operations the current user may invoke.

Filter operators: `field`, `field.gt`, `field.lt`, `field.gte`, `field.lte`, `field.contains`, `field.in`.

`tenant_id` is rejected as a client filter.

## Agent

- `GET /tools` and `GET /agent/tools` — permission-filtered
- `POST /agent/tools/{name}/invoke`

## Errors

```json
{ "error": "forbidden", "message": "...", "details": {} }
```

Business-rule failures use `error: "business_rule_failed"` (HTTP 409) with a stable `code` in `details`. Invalid workflow states use `workflow_error`. Permission failures use `forbidden`. Cross-tenant reads look like `not_found`.
