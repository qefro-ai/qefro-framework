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
  "name": "reservation.confirmed",
  "entity": "Reservation",
  "entity_id": "...",
  "tenant_id": "...",
  "user_id": "...",
  "timestamp": "...",
  "payload": { "status": "Confirmed" }
}
```

CRUD still emits `{entity}.created|updated|deleted`. Operations emit the name configured on `OperationDef::event` and any extra events the handler queued with `ctx.emit`.

Events are published **after COMMIT**. A rolled-back operation does not emit a successful business event.

The in-process bus keeps a recent debug log (`GET /api/v1/events`). It is not a durable queue. Background work belongs in jobs.
