# Background jobs

Qefro V0.3 ships a PostgreSQL-backed job queue. There is no Kafka, RabbitMQ, or Redis requirement.

## Table

`jobs` stores `tenant_id`, `user_id`, `name`, `payload`, `status`, `attempts`, `max_attempts`, `run_at`, and `last_error`.

Statuses: `pending` → `running` → `succeeded` | `failed`. Failed attempts with remaining retries return to `pending` with exponential backoff (capped at 5 minutes).

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

The HTTP server polls every two seconds. Tests call `JobQueue::process_one` directly.

Jobs are tenant-aware: enqueue uses the operation's tenant, and `JobQueue::get` requires `tenant_id`.
