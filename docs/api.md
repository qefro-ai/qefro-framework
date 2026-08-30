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

Create user body: `{ name, email, password, roles }`. Same path as `POST /users` (the User entity). Password is write-only and never returned. See [Identity](identity.md).

## Tenants

`GET /tenants` — current tenant only

`GET/PATCH /tenants/me/config` — full configuration. PATCH is Admin-only.

`GET/PATCH /tenant`, `/tenant/branding`, `/tenant/apps`, `/tenant/features` — same tenant, never another tenant's row.

## Health

`GET /health` — process liveness (`status`, `framework`). `GET /ready` — database reachable. `GET /metrics` — process counters. Neither health nor ready returns connection strings.

## Metadata

- `GET /meta/entities`
- `GET /meta/entities/{name}`
- `GET /meta/ui` — entities, branding, locale, and per-entity `permissions: { list, create, read, update, delete }` chrome hints
- `GET /meta/permissions`
- `GET /meta/workflows`
- `GET /meta/modules`
- `GET /meta/dashboards`
- `GET /dashboards/{name}`
- `GET /meta/reports`

## Studio

Requires Studio capabilities (`studio.view` and related). Tenant isolation applies to drafts.

- `GET /studio/overview`
- `GET /studio/apps`, `GET /studio/apps/{app}`
- `GET /studio/entities`, `GET /studio/entities/{entity}`
- `GET /studio/workflows/{entity}`
- `GET /studio/permissions/{entity}`
- `POST /studio/validate`, `POST /studio/publish`, `POST /studio/drafts`

See [studio.md](studio.md).
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
GET    /{slug}/{id}/activity
POST   /{slug}/{id}/comments     { "message": "…" }
GET    /{slug}/{id}/actions
POST   /{slug}/{id}/actions/{name}
```

List/get include `_expanded` (many-to-one labels), `_related` (one-to-many, GET only), `_links` (related counts), `_workflow` (allowed transitions), `_actions` (allowed business operations), and GET `_permissions: { update, delete }` (chrome hints). Unauthorized fields are omitted. The UI hides New/Edit/Delete from these hints; the server still returns 403 for unauthorized writes.

See [sdk.md](sdk.md) for the browser client.

## Platform

| Method | Path |
| --- | --- |
| GET/PATCH | `/settings/{slug}` |
| GET | `/search?q=` |
| GET | `/notifications` |
| POST | `/notifications/{id}/read` |
| GET | `/audit` (Admin) |
| GET | `/webhooks`, `/webhooks/{name}/deliveries` |
| POST | `/webhooks/{name}/test` |
| GET/POST | `/{slug}/{id}/attachments` |
| GET/PATCH/DELETE | `/attachments/{id}` |
| POST | `/attachments/{id}/replace` |
| POST | `/{slug}/import/preview`, `/{slug}/import` |
| GET | `/realtime` (SSE) |
| GET/POST | `/public/{tenant}/{form}` |

See [singletons](singletons.md), [search](search.md), [files](files.md), [attachments](attachments.md), [notifications](notifications.md), [webhooks](webhooks.md), [imports](imports.md), [realtime](realtime.md), [public forms](public-forms.md).

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

Stable public envelope:

```json
{
  "error": "validation_failed",
  "message": "Reservation date is required",
  "details": { "fields": [{ "field": "reservation_date", "code": "required", "message": "..." }] }
}
```

`error` is a string code (not a nested object) so the generic UI can branch on it. Validation and locked documents also include `fields` and nested `nested`.

| Code | HTTP |
| --- | --- |
| `unauthenticated` | 401 |
| `forbidden` | 403 |
| `not_found` | 404 |
| `app_not_enabled` | 404 |
| `bad_request` | 400 |
| `validation_failed` | 422 |
| `locked` | 422 |
| `conflict` | 409 |
| `invalid_transition` | 409 |
| `business_rule_failed` | 409 |
| `migration_required` | 409 |
| `rate_limited` | 429 |
| `payload_too_large` | 413 |
| `dependency_failed` | 500 |
| `internal_error` | 500 |

SQL, connection strings, filesystem paths, stack traces, secrets, and query text are never returned. They belong in server logs keyed by `request_id`.

Cross-tenant reads look like `not_found`. Permission failures use `forbidden`.

## System

`GET /health` — process is alive. `GET /ready` — database ping succeeded. `GET /metrics` — process counters (no tenant PII). `GET /api/v1/meta/version` — framework and schema versions.
