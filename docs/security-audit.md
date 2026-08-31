# Security audit (Qefro 3.7)

This document is the security-boundary inventory and findings from the framework audit. It does not replace [security.md](security.md), [permissions.md](permissions.md), or [multitenancy.md](multitenancy.md).

**Trust only authenticated server-side context and validated metadata.** The generic UI, SDK, CLI, Studio, automation, import, export, and workers are untrusted entry points. `EntityService` plus the authorization layer remain the boundary.

```
                    HTTP / SDK / CLI / UI
                              │
                              ▼
                    Authentication
                              │
                              ▼
                       Tenant Context
                              │
                              ▼
                       Authorization
                    ┌─────────┴─────────┐
                    ▼                   ▼
                 RowPolicy         Permissions
                    └─────────┬─────────┘
                              ▼
                       EntityService
```

## SECURITY AUDIT SUMMARY

| Severity | Count | Status |
| --- | ---: | --- |
| Critical | 0 | — |
| High | 4 | Fixed in this pass |
| Medium | 6 | Fixed or mitigated |
| Low | 5 | Hardened or documented |
| Informational | several | Documented residual risk |

No known remaining critical findings. No known high-severity tenant-isolation failures after the fixes below.

## Security Hardening v2

Follow-up to the 3.7 audit. No second auth stack, no second permission system, EntityService remains the boundary.

| Severity | Count | Status |
| --- | ---: | --- |
| Critical | 0 | — |
| High | 0 remaining | — |
| Medium | 0 new | Activity RowPolicy, SSRF, production gates |
| Low | residuals | Token XSS, in-memory limits, entity RLS, DNS rebinding, sqlx/rkyv |

### Fixed

- Activity timelines, dashboard activity widgets, and activity counts use `EntityService::get` (same RowPolicy as records).
- Webhook outbound URLs: scheme/host checks, private IP / metadata block, no redirects, DNS resolution before connect.
- Automation jobs ignore payload `tenant_id` / `user_id`; `OpContext` from the job row wins.
- Production startup rejects compose default DB password and debug/trace log level (without printing secrets).
- Query `IN` lists capped at 100 values.
- `POST /api/v1/auth/refresh` reissues a JWT for the same session.
- Login/register/export/dashboard/report/bulk specialized rate limits with `Retry-After`.
- CSP tightened (`frame-src`, `object-src`, `font-src`, swagger-ui origin narrowed). Rich-text `style` attributes stripped in the renderer.
- `qefro_activity` RLS pilot (`SET LOCAL` tenant GUC; purge uses documented `qefro.rls_bypass`).
- `SECURITY.md`, [dependencies.md](dependencies.md), [rls.md](rls.md), CI `cargo audit` (non-blocking).

### Mitigated

- Bearer token still in `localStorage` (SPA + SDK compatibility). Lifetime configurable; UI refreshes before expiry; logout/tenant switch clear storage; tokens not logged. **XSS can still steal the token.**
- Rate limits remain in-memory (`RateLimitStore` boundary for a future distributed adapter).
- sqlx/rust_decimal unused optional features disabled so `rkyv` and `rsa` are not in the runtime graph (`cargo tree -i rkyv` / `rsa` empty).

### Accepted residual

| Risk | Reason | Mitigation | Future action |
| --- | --- | --- | --- |
| Token in `localStorage` | SPA Bearer model; HttpOnly cookies would require CSRF | CSP, sanitizers, session revoke, refresh, TTL | Optional HttpOnly cookie only with CSRF tokens |
| In-memory rate limits | Single-process default; no Redis required | Proxy limiter; `RateLimitStore` trait | Redis adapter when multi-instance |
| Entity tables without RLS | Pooling / query-path complexity | Tenant predicates + RowPolicy; activity RLS pilot | Expand RLS table-by-table with `SET LOCAL` |
| SSE event metadata without row policy | Cost of `get()` per event | Entity Read filter; record subscribe uses `get()` | Optional per-event get |
| Webhook DNS rebinding | reqwest resolves again after our check | Block literals/private names; no redirects | Pin IPs / egress firewall |
| `rkyv` / `rsa` rustsec in `Cargo.lock` | Unused optionals of rust_decimal / sqlx-mysql; not in `cargo tree` on Linux | `default-features = false`; `audit.toml` ignores with rationale | Re-run `cargo audit` when enabling those features |
| Public `/metrics` `/docs` | Operator surfaces | Do not expose API port without edge authz | Optional auth on docs |
| Register email uniqueness timing | Unique index | Uniform error text | Accept |

---

## Boundaries

### Authentication

JWT HS256 access tokens (12h) bound to a server `sessions` row. `jsonwebtoken` is configured to **HS256 only** (`alg: none` and other algorithms are rejected). Production refuses a missing, default, or short `JWT_SECRET`. Passwords are Argon2id. Login always performs a password verify (dummy hash when the email is unknown) and returns **401 invalid credentials** for unknown user, bad password, disabled user, and disabled membership.

Logout sets `sessions.revoked_at`. The generic UI calls `POST /api/v1/auth/logout` before clearing `localStorage`. Tenant switch revokes the previous session and issues a new token.

Open registration (`POST /api/v1/auth/register`) is **off in production** unless `QEFRO_ALLOW_REGISTER=true`.

### Authorization

RBAC and field permissions are enforced in `EntityService`, not in the UI. Role assignment on User create/update requires Admin. Client `roles: ["Admin"]` from Staff is 403.

### Tenant

`tenant_id` comes from the session. Bodies, query filters, and `X-Tenant-ID` cannot set it. Cross-tenant reads are **404**. Blob keys are prefixed with tenant id.

### EntityService

CRUD, bulk, export, import, print, attachments, search, reports, and dashboard metrics go through `EntityService`. Workers cannot generic-CRUD unless an operation is `worker_safe`. Automation runs with `source=automation` but **cannot use Admin/System/Public** as `as_roles` and does not inherit Admin from the event actor.

### Metadata

Formulas, templates, reports, custom fields, and Studio payloads are declarative. They cannot execute SQL, Rust, or JavaScript. Custom field values live in `qefro_custom` JSONB; identifiers are allowlisted.

### Worker / jobs / outbox

Jobs rebuild `OpContext::worker` from the **stored** tenant/user. `AttachmentPurgeJob` deletes blobs for `ctx.tenant_id` only; a payload `tenant_id` that does not match is rejected.

### Files

Generated `storage_key`; `..` / absolute paths rejected. `storage_key` is omitted from client JSON.

### Provider

Webhook HMAC secrets and communication provider credentials are never returned to the browser.

### Database

Parameterized values; identifiers via `quote_ident` / `assert_safe_ident`. Tenant predicates are bound. Application login should not be PostgreSQL SUPERUSER. `qefro_activity` has FORCE RLS keyed by `qefro.tenant_id` (`SET LOCAL` in the activity transaction). Entity tables do not. See [rls.md](rls.md).

### Browser

The generic UI stores the access token in `localStorage` (XSS ⇒ session theft — residual). `POST /auth/refresh` rotates a still-valid JWT. Rich text is sanitized with ammonia on write and again in the React renderer (`style` / `javascript:` stripped). Color values are allowlisted before use in CSS. nginx (and the API) send CSP, `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`. `unsafe-eval` is not used. `unsafe-inline` remains for SPA inline styles and for `/docs` swagger-ui.

Auth is **Authorization: Bearer**, not cookies, so classic cookie CSRF does not apply. Production CORS is an explicit origin list (`QEFRO_CORS_ORIGINS` or the origin of `QEFRO_PUBLIC_URL`), never `*`.

---

## Findings

### H1 — Automation could run as Admin

**Severity:** HIGH  
**Component:** `qefro-db` automation `action_context`  
**Scenario:** Studio `as_roles: ["Admin"]`, or an Admin-triggered event with empty `as_roles`, caused automations to inherit Admin and bypass RowPolicy.  
**Root cause:** Privileged roles were not stripped; empty `as_roles` loaded the event user's membership roles.  
**Fix:** `sanitize_automation_roles` drops Admin/System/Public at validate, Studio payload check, and runtime. Empty result uses `Worker`.  
**Test:** `qefro-core` `validate_rejects_admin_as_roles`; `reject_unsafe_automation_payload` Admin `as_roles`.

### H2 — RowPolicy bypass via search / reports / aggregates

**Severity:** HIGH  
**Component:** `global_search`, `run_report`, `dashboard_card_value`, `entity_aggregates`  
**Scenario:** Staff with `AssignedTo` policy could search or aggregate rows they could not `GET`.  
**Root cause:** List applied `apply_row_policy_filters`; search SQL and aggregate queries did not.  
**Fix:** Search adds the same tenant-safe assigned/created predicates. Reports, dashboard KPIs/charts, and `/aggregates` call `apply_row_policy_filters`.  
**Test:** `security_audit.rs` `row_policy_applies_to_search_aggregates_and_idor`.

### H3 — Attachment purge trusted payload tenant_id

**Severity:** HIGH (poisoned job payload)  
**Component:** `AttachmentPurgeJob`  
**Scenario:** A crafted job payload with another tenant's id could delete that tenant's blob.  
**Fix:** Always delete `ctx.tenant_id`. Mismatched payload `tenant_id` is forbidden.

### H4 — Logout did not revoke the server session from the UI

**Severity:** HIGH (stolen token after "Log out")  
**Component:** frontend `AppShell` + auth  
**Fix:** `QefroClient.logout()` → `POST /auth/logout`. Switch-tenant revokes the previous session.  
**Test:** `security_audit.rs` `logout_revokes_the_session`.

### M1 — Production CORS was `AllowOrigin::any()`

**Severity:** MEDIUM (defense in depth; Bearer is not cookie CSRF)  
**Fix:** Development may allow any origin. Production uses `QEFRO_CORS_ORIGINS` or `QEFRO_PUBLIC_URL`'s origin; `*` is rejected by `Config::validate`.

### M2 — Missing security headers

**Severity:** MEDIUM  
**Fix:** API middleware + nginx: `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, CSP.  
**Test:** `http_security.rs` `security_headers_are_present_on_public_routes`.

### M3 — Open registration in production

**Severity:** MEDIUM  
**Fix:** `QEFRO_ALLOW_REGISTER` defaults false when `QEFRO_ENV=production`.  
**Test:** `registration_can_be_disabled`.

### M4 — Login leaked membership/disable status (403 vs 401)

**Severity:** MEDIUM (user enumeration)  
**Fix:** Disabled membership and unknown tenant membership return **401 invalid credentials**, same as a bad password.

### M5 — JWT accepted `Validation::default()` algorithms

**Severity:** MEDIUM  
**Fix:** Encode/decode HS256 only.  
**Test:** `jwt_none_algorithm_is_rejected`.

### M6 — Unfiltered SSE leaked entity event names

**Severity:** MEDIUM  
**Fix:** Realtime stream drops events for entities the caller cannot `Read`. Record subscriptions still `get()` the row (RowPolicy). Unfiltered streams still do not apply per-row policy to event **metadata** (id/name) — residual.

### L1 — Rich-text `dangerouslySetInnerHTML`

**Severity:** LOW (ammonia on write)  
**Fix:** Client-side DOM sanitizer as defense in depth; `javascript:` colors rejected.

### L2 — Dummy `token_hash` on sessions unused

**Severity:** LOW  
**Mitigation:** Auth binds `sid` in JWT to the session row. Hash column is unused; do not treat it as a second secret.

### L3 — Register still conflicts on unique email/slug

**Severity:** LOW  
**Mitigation:** Message is `could not create account` (no “email exists”). Timing and uniqueness still allow some inference.

### L4 — In-memory rate limits / spoofable `X-Forwarded-For`

**Severity:** LOW  
**Mitigation:** `RateLimitStore` + remaining/`Retry-After`. Specialized login/register/expensive-op limiters. Login 429 text is uniform. Put a trusted proxy in front in production. Multi-instance does not share counters.

### L5 — Activity / audit widgets vs RowPolicy

**Severity:** LOW  
**Fix (v2):** Activity list/get/dashboard/counts call `EntityService::get` (RowPolicy). Audit remains Admin-only. Do not put secrets in activity messages.

---

## Residual risk (accepted)

See **Security Hardening v2** above. The v1 table is superseded; remaining items are token XSS, in-memory limits, entity-table RLS, SSE metadata, DNS rebinding, and transitive rustsec findings.

---

## Production configuration

| Variable | Production requirement |
| --- | --- |
| `QEFRO_ENV` | `production` |
| `JWT_SECRET` | Non-default, ≥ 16 characters |
| `DATABASE_URL` | Required; not the compose default password in real deploys |
| `QEFRO_ALLOW_REGISTER` | Unset/false unless you intend public signup |
| `QEFRO_CORS_ORIGINS` | Explicit origins, or omit to use `QEFRO_PUBLIC_URL` |
| `QEFRO_AUTO_MIGRATE` | false; run `qefro migrate` separately |
| `QEFRO_EMBED_WORKER` | false when running `qefro worker` |
| `QEFRO_TOKEN_TTL_HOURS` | Optional; default 12 |
| `QEFRO_LOG_LEVEL` | not `debug` / `trace` |

TLS terminates at the reverse proxy. The app assumes `X-Forwarded-For` is set by a **trusted** proxy only.

---

## Regression tests

- `crates/qefro-api/tests/security_audit.rs`
- `crates/qefro-api/tests/security_hardening.rs` (v2: activity RowPolicy, tenant matrix, privilege, workflow, secrets, refresh, SSRF, IN cap)
- `crates/qefro-api/tests/http_security.rs` (headers)
- `crates/qefro-api/tests/v1_security.rs` (cross-tenant)
- `crates/qefro-api/tests/identity.rs` (escalation, disable)
- `crates/qefro-core` outbound URL / rate-limit / automation privileged-role tests
- `frontend` FieldValue XSS / CSS tests
- CI: `cargo test --workspace`, `npm test`, `cargo audit` (non-blocking; `.cargo/audit.toml` ignores unused-optional rkyv/rsa)
