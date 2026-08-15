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
Validation
      ↓
Workflow
      ↓
Business operation
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

Do not log passwords, access tokens, `DATABASE_URL`, or JWT secrets. HTTP 5xx uses `QefroError::public_message`. Structured logs include `request_id`, `tenant_id`, `user_id`, operation, entity, duration, and status.

## Rate limits

`RateLimiter` is an in-memory hook keyed by `tenant:user:path`. It is not a distributed limiter. A Redis adapter can implement the same trait later.

## Metering

`MeteringEvent` is emitted for `api.request`, `agent.tool.executed`, and `workflow.executed`. It is a billing hook, not a billing system.
