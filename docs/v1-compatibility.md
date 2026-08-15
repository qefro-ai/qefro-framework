# V1.0 compatibility contract

Qefro Framework **1.0.0** is a stable, production-ready release. This document is the compatibility contract. Anything marked **Stable** will not change incompatibly without a new major version.

Current versions:

| Surface | Version |
| --- | --- |
| Qefro Framework | `1.0.0` (`FRAMEWORK_VERSION`) |
| Metadata schema | `1` (`METADATA_SCHEMA_VERSION`) |
| UI schema | `1` (`UI_SCHEMA_VERSION`) |
| REST API | `v1` (`API_VERSION`, paths under `/api/v1`) |
| App package format | `1` (`APP_API_VERSION` / `PACKAGE_FORMAT`) |
| Migration record format | `1` (`MIGRATION_FORMAT_VERSION`) |

These are independent of Cargo crate versions. Apps declare `framework_version` (default `>=1.0,<2.0`) and `api_version = "1"`.

## Classification

### Stable

Must not change incompatibly without Qefro 2.0:

- Entity metadata (`EntityDef`, fields, relations, child tables, documents, numbering)
- UI metadata schema (`schema_version: "1"`, widgets, visibility flags)
- App package layout (`app.toml`, allowlisted directories, `qefro-package.json`)
- REST conventions under `/api/v1` (CRUD, actions, search, import, attachments, public forms, webhooks, realtime, Studio)
- Public error envelope (`error`, `message`, `details`) and error codes listed in [API](api.md)
- Operation semantics (named operations go through `EntityService`)
- Tenant isolation (session tenant only; client `tenant_id` rejected)
- Workflow transition semantics
- Permission semantics (RBAC + field levels; UI is not authorization)
- Migration records (`pending` / `applied` / `failed`, checksum, advisory lock)
- Qefro app lifecycle (`validate` → `package` → `install` → `migrate` → tenant enable)

### Experimental

May change in a 1.x release with a deprecation note:

- Studio overlay/publish UX beyond the published metadata types
- In-process event debug log (`GET /api/v1/events`)
- `/metrics` field set (names may be added; existing counters stay)
- YAML notification/webhook files outside `AppModule` builders
- Connector and Qefro SDK surfaces outside this repository

### Internal

Not a public contract. Do not depend on these from apps:

- SQLx schema helpers, repository SQL, job claim SQL
- `qefro_outbox` row layout (the **event id** on `DomainEvent.id` is public)
- Agent tool JSON schema generator internals
- Frontend component file layout

### Deprecated

None in 1.0.0. `POST /api/v1/{slug}/{id}/transition` remains supported; prefer named operations.

## Guarantees

- A committed business mutation is written with audit in the same transaction. Required events are inserted into the outbox in that transaction and dispatched after COMMIT (**at-least-once**).
- Failed migrations are recorded as `failed` and are never silently marked `applied`.
- Cross-tenant reads return `not_found` (404), not a confirmation that the id exists elsewhere.

## Breaking changes

After 1.0.0 is declared, breaking changes to Stable surfaces require a major version and a deprecation period where practical.
