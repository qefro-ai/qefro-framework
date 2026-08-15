# Realtime

Post-commit events fan out over **SSE** (`GET /api/v1/realtime`). One transport is enough; WebSockets are not implemented.

```
Mutation → transaction → COMMIT → Event → Audit / Job / Webhook / Notification / Realtime
```

Query filters: `entity`, `record_id`. Tenant comes from the authenticated session. A record subscription requires Read on the entity **and** a successful `EntityService::get` of that id.

Payload:

```json
{
  "event": "order.updated",
  "entity": "Order",
  "record_id": "...",
  "changed_fields": ["status"]
}
```

The generic list, detail, dashboard, and notification UI refresh on events. Authorization is never delegated to the client.

SSE sends a heartbeat every 15 seconds (`KeepAlive`). Dead connections are dropped by the HTTP stack; the broadcast channel has a bounded buffer (1024). Slow clients that lag are disconnected rather than stalling the process.
