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

Workers are not on this path. See [Jobs](jobs.md).

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

## Rate limits

`RateLimiter` is an in-memory hook keyed by `tenant:user:path` (and specialized keys for login, search, public forms, uploads, and imports). Client-supplied tenant ids cannot change the key. It is not a distributed limiter. A Redis adapter can implement the same trait later.

Limits: list page size ≤ 200, max 20 filters, max 3 sort fields, search ≤ 200 characters, CSV/JSON import ≤ 10 MiB (max 100,000 rows / 64 columns), attachments ≤ 10 MiB, request body ≤ 12 MiB.

See also [threat-model.md](threat-model.md) and [v1-compatibility.md](v1-compatibility.md).

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
