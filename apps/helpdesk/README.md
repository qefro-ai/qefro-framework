# Helpdesk (V1.0 benchmark)

Minimal metadata app used to prove attachments, notifications (via events), realtime, public forms, search, and actions. It is not a full service desk product.

```bash
qefro app validate helpdesk
qefro app install helpdesk
qefro migrate --app helpdesk
qefro tenant app enable demo helpdesk
```

Public intake: `GET /api/v1/public/{tenant}/ticket`.
