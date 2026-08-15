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

- `GET /health` — process is up
- `GET /ready` — database ping
- `GET /metrics` — process counters (no tenant PII)

`qefro serve` and `qefro worker` stop on SIGTERM after finishing the current job. Claimed jobs that were left `running` are reclaimed to `pending` on worker start.

## Migration procedure

1. Back up PostgreSQL.
2. Run `qefro migrate` (or the `migrate` compose service) against the target database. This command always applies schema.
3. Start `qefro serve` and `qefro worker` with `QEFRO_AUTO_MIGRATE=false`.
4. If migrate fails, **do not** start application processes. Fix the error and retry.

Development may keep `QEFRO_AUTO_MIGRATE=true` so `qefro dev` applies schema on boot. Production must not silently mutate schema from the HTTP process.

## Backups and disaster recovery

Qefro does not ship a backup product. Snapshotting the application server is not sufficient.

| What | How |
| --- | --- |
| Database | PostgreSQL `pg_dump` (or WAL-G / provider PITR). Restore onto a scratch instance, run `qefro migrate`, smoke-test. |
| Configuration | Environment / secrets manager (`DATABASE_URL`, `JWT_SECRET`, SMTP, webhook secrets). |
| App definitions | Installed `.qefro` packages plus git of YAML/Rust apps. |
| Tenant files | Copy `QEFRO_STORAGE_PATH` (per-tenant directories) with the dump. |

**Verify:** restore a dump monthly; confirm login, a record, an attachment, and `qefro doctor`.

**Migrations:** take a dump immediately before `qefro migrate`. Failed migrations are recorded as `failed` and are never silently marked successful. Do not blindly retry a destructive migration with a changed checksum.

PostgreSQL advisory locking (`pg_advisory_lock`) ensures only one migrate process applies app SQL at a time.

## Storage

`LocalBlobStore` writes `{QEFRO_STORAGE_PATH}/{tenant_id}/{key}`. Path traversal is rejected. An S3-compatible `BlobStore` can be added later without changing tenant isolation rules.
