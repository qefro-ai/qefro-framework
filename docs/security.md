# Security

## Request pipeline

```
Authentication
      ↓
Tenant Context (session, never the client)
      ↓
Application availability (installed ∩ enabled ∩ plan)
      ↓
RBAC
      ↓
Field permissions
      ↓
Validation
      ↓
Workflow
      ↓
Business operation
      ↓
COMMIT
      ↓
Event (realtime, notification, webhook, job)
```

Workers are not on this path. See [Jobs](jobs.md). Full boundary inventory: [security-audit.md](security-audit.md).

## Tenant isolation

- Every tenant-owned row includes `tenant_id` from `OpContext`.
- Create/update/action/agent bodies that include `tenant_id` return 400.
- `X-Tenant-ID` is ignored.
- `tenant_id` query filters return 400.
- Cross-tenant reads return 404, not 403.
- `GET /api/v1/tenants` returns the current tenant only.
- Tenant configuration cache keys are `tenant_id`.
- Blob keys are prefixed with `tenant_id`.
- In-memory rate-limit keys include `tenant_id`.

## Application entitlements

Frontend navigation is not a security boundary. `EntityService::ensure_app` returns 404 for entities whose module is not enabled. Agent tool lists and invoke use the same check. Feature flag `agent_actions=false` forbids tool invoke.

## Worker authorization

Background jobs rebuild `OpContext` with role `Worker`, not `Admin` or `System`.

- A job handler runs only if `JobHandler::worker_safe()` is true.
- Unregistered jobs fail.
- `EntityService` CRUD is forbidden for workers.
- `execute` is allowed only when `OperationDef.worker_safe` is true. Manager-only operations stay rejected unless they opt in.

## Secrets and logs

Do not log passwords, access tokens, `DATABASE_URL`, or JWT secrets. HTTP 5xx uses `QefroError::public_message`. Structured logs include `request_id`, `tenant_id`, `user_id`, path, duration, and status. Every response may echo `x-request-id`. User `password_hash` and session tokens are never returned by EntityService or `/meta/ui`. Activity, audit, attachments, and agent schemas also strip those keys. `GET /api/v1/audit` is Admin-only. Attachments are tenant-scoped; guessing another tenant's file id returns 404. See [Identity](identity.md), [Audit](audit.md), [Activity](activity.md), and [Files](files.md).

## Token model

Browser and SDK both use **Authorization: Bearer** JWTs bound to a `sessions` row. This is not a cookie session; classic CSRF does not apply. Production CORS is an explicit origin list.

The generic UI stores the access token in `localStorage` so reloads and extra tabs keep working. **XSS can still steal that token.** Mitigations: CSP, HTML sanitization (ammonia + DOM sanitizer), logout/tenant-switch revoke, `POST /api/v1/auth/refresh` to rotate a still-valid JWT, configurable `QEFRO_TOKEN_TTL_HOURS` (default 12). Server-to-server SDK usage is unchanged: the same Bearer token, no cookies, no CSRF tokens.

Tokens must never appear in URLs, logs, error bodies, activity, or analytics. Logout always clears `localStorage` even if the revoke call fails. `QefroClient.switchTenant` overwrites the stored token after the previous session is revoked. Unauthenticated routes (`/studio`, `/settings`, `/audit`, entity pages) render the login screen without fetching metadata. Studio waits for `studio.view` before painting chrome or overview data.

## Rate limits

`RateLimiter` + `RateLimitStore` is an in-memory hook (default `MemoryRateLimiter`). Keys include `tenant:user:path` plus specialized keys for login, register, search, public forms, uploads, imports, exports, dashboards, and reports. Responses may include `Retry-After`. Client-supplied tenant ids cannot change the key.

**Single-instance:** counters are per process. **Multi-instance:** counters are not shared unless a distributed `RateLimitStore` is provided later; put a reverse proxy limiter in front. Redis is not required.

Login is keyed by normalized email **and** client IP. Unknown and known accounts share the same 429 message (`too many login attempts`) so rate limiting does not become a user-existence oracle.

Limits: list page size ≤ 200, max 20 filters, max 100 `IN` values, max 3 sort fields, search ≤ 200 characters, CSV/JSON import ≤ 10 MiB (max 100,000 rows / 64 columns), attachments ≤ 10 MiB, request body ≤ 12 MiB.

## PostgreSQL RLS

Application authorization is authoritative. A small RLS **pilot** is enabled on `qefro_activity` (`SET LOCAL qefro.tenant_id` per transaction). Entity tables are not converted. See [rls.md](rls.md).

## Production gates

- `JWT_SECRET` must be non-default and at least 16 characters when `QEFRO_ENV=production`.
- `DATABASE_URL` must not use the compose default `qefro:qefro` password in production.
- `QEFRO_LOG_LEVEL` must not be `debug` or `trace` in production.
- `QEFRO_ALLOW_REGISTER` defaults to false in production.
- `QEFRO_CORS_ORIGINS` must not be `*`. Unset uses the origin of `QEFRO_PUBLIC_URL`.
- Authentication is the `Authorization: Bearer` header, not cookies. Cookie CSRF tokens are not used. Logout revokes the session row; tenant switch revokes the previous session. Validation failures never print secret values.

See also [threat-model.md](threat-model.md), [security-audit.md](security-audit.md), [dependencies.md](dependencies.md), and [v1-compatibility.md](v1-compatibility.md).

## Field permissions

Unauthorized fields are stripped on read and rejected on write inside `EntityService`. The UI cannot be trusted to hide salary or similar fields.

## Public forms

Public routes resolve tenant from the URL slug. Bodies cannot set `tenant_id`. Only allowlisted fields are accepted. The execution context is `Public`, not Admin. Rate limits apply.

## Webhooks and notifications

Both run after COMMIT. Webhook HMAC secrets are never returned to clients. Attachment storage keys are generated server-side; `..` filenames are rejected.

## Realtime

SSE subscriptions use the session tenant. Record subscriptions require a successful read of that record.

## Metering

`MeteringEvent` is emitted for `api.request`, `agent.tool.executed`, and `workflow.executed`. It is a billing hook, not a billing system.
