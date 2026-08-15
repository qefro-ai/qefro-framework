# Webhooks

Event-driven HTTP callbacks after COMMIT. They use the existing job queue. There is no second broker.

```yaml
webhook:
  name: order_created
  event: order.created
  target: https://example.com/webhook
```

## Security

Deliveries include:

```
X-Qefro-Event
X-Qefro-Event-ID
X-Qefro-Timestamp
X-Qefro-Signature   # sha256=HMAC(secret, "{timestamp}.{event_id}.{body}")
```

The secret comes from `secret_env` or `QEFRO_WEBHOOK_SECRET`. Studio and tenant APIs never return secrets.

## Retry and log

Failed HTTP responses re-enter the job queue with backoff. `(tenant_id, webhook, event_id)` is unique so retries stay idempotent.

**Delivery semantics: at-least-once.** Timeouts, HTTP 5xx, and connection failures retry. Duplicate deliveries can happen after a crash; verify HMAC and ignore duplicate `X-Qefro-Event-ID` values. Qefro does not claim exactly-once.

`qefro_webhook_deliveries` records status, retry count, and last error.

```http
GET  /api/v1/webhooks
GET  /api/v1/webhooks/{name}/deliveries
POST /api/v1/webhooks/{name}/test
```

Admin only. Test delivery is authorized independently of listing metadata.
