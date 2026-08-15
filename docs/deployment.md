# Deployment

Qefro is a modular monolith: one HTTP process, one worker process, one PostgreSQL database, one generic frontend. No Kubernetes, no microservices, no message broker.

## Modes

### Shared SaaS

One runtime, many tenants. Tenant identity comes from the session. Branding, enabled apps, and data are isolated by `tenant_id`.

```
Qefro Platform → Shared Runtime → Tenant A / B / C → Shared PostgreSQL
```

`docker compose up --build` is the local version of this mode.

### Dedicated tenant

One VM (or container host) per customer. Same binary. Point `DATABASE_URL` at that customer's database. Optionally install only that customer's applications.

```
Customer VM → qefro serve + qefro worker → Customer PostgreSQL
```

### Enterprise self-hosted

The customer runs the binary against their PostgreSQL. They own backups, TLS, and secrets. Qefro does not require Redis or object storage; local disk holds tenant assets until an S3 adapter is added.

## Containers

Images:

| Service | Image / command |
| --- | --- |
| `qefro-server` | `qefro serve` |
| `qefro-worker` | `qefro worker` |
| `frontend` | nginx + generic UI |
| `postgres` | official Postgres 16 (optional / externalizable) |

PostgreSQL is a separate container. Do not bake a database into the application image.

To use an external database, set `DATABASE_URL` on `migrate`, `server`, and `worker`, and do not start the `postgres` service.

```bash
docker compose up --build
# UI: http://localhost:8081
# API: http://localhost:8080
```

Health:

- `GET /health` — process is up (no infrastructure details)
- `GET /ready` — database ping

## Migration procedure

1. Back up PostgreSQL.
2. Run `qefro migrate` (or the `migrate` compose service) against the target database. This command always applies schema.
3. Start `qefro serve` and `qefro worker` with `QEFRO_AUTO_MIGRATE=false`.
4. If migrate fails, **do not** start application processes. Fix the error and retry.

Development may keep `QEFRO_AUTO_MIGRATE=true` so `qefro dev` applies schema on boot. Production must not silently mutate schema from the HTTP process.

## Backups

Qefro does not ship a backup product. Recommended practice:

- **PostgreSQL**: nightly `pg_dump` (or WAL-G / provider PITR) of the whole cluster. Restore onto a scratch instance and run `qefro migrate` + a smoke test before you need it.
- **Migration safety**: take a dump immediately before `qefro migrate`. Schema apply is deterministic DDL (`IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`); it is not a substitute for a restore drill.
- **Encryption**: enable storage-level encryption on the database volume; TLS for clients. Application rows are not field-encrypted.
- **Tenant recovery**: tenants share tables. Recovering one tenant from a full dump requires extracting that `tenant_id` from every tenant-owned table (including `tenant_settings`, `jobs`, `audit_logs`). Prefer PITR of the cluster over hand-built per-tenant dumps unless you have practiced the extract.
- **Blobs**: copy `QEFRO_STORAGE_PATH` (per-tenant subdirectories) with the database dump so logos and attachments stay consistent.

## Storage

`LocalBlobStore` writes `{QEFRO_STORAGE_PATH}/{tenant_id}/{key}`. Path traversal is rejected. An S3-compatible `BlobStore` can be added later without changing tenant isolation rules.
