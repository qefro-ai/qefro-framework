# Threat model

Qefro is a multi-tenant business application framework. The primary assets are tenant business data, credentials, file attachments, and metadata that drives APIs and UI.

## Actors

| Actor | Goal |
| --- | --- |
| Malicious tenant | Read or mutate another tenant's data |
| Malicious user | Escalate role, skip workflow, read restricted fields |
| Malicious app package | Escape the install directory, run SQL, steal secrets |
| Malicious webhook consumer | Replay deliveries, extract HMAC secrets |
| Malicious public visitor | Switch tenant, inject hidden fields, flood public forms |
| Compromised connector / agent | Use tools as a confused deputy against EntityService |
| Database / file storage exposure | Dump rows or blobs without going through Qefro |

## Mitigations

### Malicious tenant

Tenant id comes from the authenticated session. Create/update/action/agent bodies that include `tenant_id` return 400. `X-Tenant-ID` is ignored. SQL for tenant-owned entities always includes `tenant_id`. Blob keys are prefixed with tenant id. SSE and notifications filter by session tenant. Cross-tenant reads return 404.

### Malicious user

RBAC and field permissions are enforced in `EntityService`, not in the generic UI. Staff cannot invoke Manager transitions. Locked documents reject writes except `allow_on_submit` fields. Studio capabilities are server-checked.

### Malicious app package

`.qefro` archives reject path traversal, absolute paths, zip-bomb ratios, oversized files, duplicate names, and unexpected paths outside the allowlisted package layout. Manifest and `framework_version` are validated before install. Packages must not contain secrets.

### Malicious webhook

Deliveries are HMAC-signed. Secrets are never returned by APIs. Delivery is **at-least-once** with retries and backoff; consumers must be idempotent using `event_id`. Invalid endpoints fail the job, not the business transaction.

### Malicious public visitor

Public forms resolve tenant from the URL slug. Only allowlisted fields are accepted. Context is `Public`, not Admin. Rate limits apply. Attachments and operations are not exposed unless explicitly allowlisted.

### Compromised agent

Agents call tools that call `EntityService`. They never receive a SQLx pool, credentials, or raw SQL. Tool lists are permission-filtered. The same tenant and field-permission rules apply as REST.

### Database or file exposure

Treat PostgreSQL and `QEFRO_STORAGE_PATH` as trusted systems. Application-level isolation is not a substitute for database ACLs, TLS, and backups. `qefro doctor` and `/ready` must not print connection strings or JWT secrets.

## Residual risk

In-memory rate limiting is per process. A distributed limiter can implement the same `RateLimiter` trait later. RLS is not generated in V1.0; tenant predicates in SQL remain the isolation mechanism.
