# Events

Events are business facts emitted after a successful mutation.

```
reservation.confirmed
reservation.seated
reservation.completed
reservation.cancelled
order.confirmed
order.preparing
order.ready
order.completed
```

Payload:

```json
{
  "id": "...",
  "event_id": "...",
  "name": "reservation.confirmed",
  "event_type": "reservation.confirmed",
  "entity": "Reservation",
  "entity_id": "...",
  "record_id": "...",
  "tenant_id": "...",
  "user_id": "...",
  "actor": "...",
  "timestamp": "...",
  "payload": { "status": "Confirmed" }
}
```

CRUD still emits `{entity}.created|updated|deleted` plus framework names `entity.created` / `entity.updated` / `entity.deleted`. Workflow emits `workflow.transitioned`. Comments emit `comment.created`. Files emit `file.uploaded` / `file.replaced` / `file.updated` / `file.deleted` (`attachment.created` remains as a compatibility alias on upload). Operations emit the name configured on `OperationDef::event` and any extra events the handler queued with `ctx.emit`.

Events are published **after COMMIT**. A rolled-back operation does not emit a successful business event.

V1.0 durability: the mutation transaction also inserts an **outbox** row (`qefro_outbox`) with a stable `DomainEvent.id`. After COMMIT the dispatcher publishes to the in-process bus (realtime, notifications, webhook jobs). Delivery is **at-least-once**. Consumers should deduplicate on `id` + `tenant_id`.

Job rows are still inserted in the same SQLx transaction as the mutation, so they roll back together.

The in-process bus keeps a recent debug log (`GET /api/v1/events`). It is not a durable queue. Background work belongs in jobs.

Trace a business action with `request_id` (HTTP), `DomainEvent.id` (event), automation `execution_id`, job id, and webhook delivery id. See [Automation](automation.md).
