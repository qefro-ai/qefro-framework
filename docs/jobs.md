# Background jobs

Qefro V0.3 ships a PostgreSQL-backed job queue. There is no Kafka, RabbitMQ, or Redis requirement.

## Table

`jobs` stores `tenant_id`, `user_id`, `name`, `payload`, `status`, `attempts`, `max_attempts`, `run_at`, `last_error`, and optional `idempotency_key`.

Statuses: `pending` → `running` → `succeeded` | `failed`. Failed attempts with remaining retries return to `pending` with exponential backoff (capped at 5 minutes). Client JSON may alias `pending` as `queued` and `succeeded` as `completed` without renaming columns.

Workers claim rows with `FOR UPDATE SKIP LOCKED`.

## API

```rust
ctx.enqueue_job("notify_reservation_confirmed", json!({ "entity_id": id }));
```

Jobs listed on `OperationDef::job` are enqueued inside the operation transaction. If the operation rolls back, the job row rolls back too.

Register a handler on the installed app:

```rust
app.job("notify_reservation_confirmed", LogNotificationJob)
```

The default `LogNotificationJob` logs tenant id and payload key count. It does not log customer PII. Applications replace it with email/SMS.

The HTTP server may poll jobs (`QEFRO_EMBED_WORKER`, default on in development). Production should run `qefro worker` as a separate process with `QEFRO_EMBED_WORKER=false`. On start, workers reclaim `running` rows left by a crash. SIGTERM stops claiming new jobs, finishes the current handler, then exits. Tests call `JobQueue::process_one` directly.

Jobs are tenant-aware: enqueue uses the operation's tenant, and `JobQueue::get` requires `tenant_id`.

## Worker security

Workers do **not** run as Admin or System.

```
User / agent operation  → user RBAC
Worker operation        → explicit WorkerPolicy
```

`OpContext::worker` assigns role `Worker`. A job handler runs only when `JobHandler::worker_safe()` is true (the default is false). `LogNotificationJob` opts in. Unregistered job names fail.

Workers cannot call generic `EntityService` CRUD. `EntityService::execute` is allowed only when `OperationDef.worker_safe` is true. A Manager-only mutation is still rejected unless that operation opts in.

## Idempotency

If the payload includes `idempotency_key`, enqueue is unique per `(tenant_id, name, idempotency_key)`. A retry that re-enqueues the same key returns the existing job id instead of inserting a second row.

Retries of a claimed job reuse that same row (`pending` → `running` → `succeeded` / `failed`). Handlers should be safe to run again after a transient failure. Do not enqueue a second irreversible business mutation from a notification retry.
