# Notifications

Notification rules are metadata, not entity methods.

```yaml
notification:
  name: reservation_confirmed
  event: reservation.confirmed
  channels: [in_app, email]
  recipients: [Staff, Manager]
```

Pipeline:

```
Event → Notification → in-app (email / webhook / WhatsApp later)
```

```
Mutation → transaction → COMMIT → Event → Notification dispatcher → channels
```

Nothing is sent before COMMIT. Channel failures are logged; they do not roll back the business transaction.

Framework events include `entity.created`, `entity.updated`, `entity.deleted`, `workflow.transitioned`, `comment.created`, `file.uploaded`, `attachment.created`, and `user.disabled`, plus app-specific names. Events do not bypass `EntityService` authorization.

## Channels

`NotificationChannel::send` is the extension point. V0.9 implements `in_app` (Postgres `qefro_notifications`) and `email` (job, provider from environment). `webhook` reuses the webhook dispatcher. WhatsApp / SMS / push can implement the same trait later.

In-app rows: `tenant_id, user_id, title, body, entity, record_id, read_at, created_at`.

```http
GET  /api/v1/notifications
POST /api/v1/notifications/{id}/read
```

The generic shell shows a notification bell (title, relative time, unread badge). Recipients are filtered by role; users without entity access are not notified of records they cannot read. Notifications are tenant-scoped.

The restaurant example listens for `order.confirmed` and `order.ready` so kitchen workflow transitions surface in the in-app center without a custom backend.
