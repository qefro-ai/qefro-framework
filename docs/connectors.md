# Connectors and SDK

V1.0 does not ship a second execution runtime for integrations.

## Rule

External systems call Qefro the same way the UI and agents do:

```
Browser UI     → QefroClient → REST /api/v1 → EntityService
Agent          → EntityOps   → EntityService
Connector/SDK  → REST or tool invoke        → EntityService
```

A connector must never be given a SQLx pool, database credentials, or the ability to run raw SQL.

## REST

Use `/api/v1` with a bearer session token. Tenant is the session tenant. See [API](api.md).

## Agents

`GET /api/v1/agent/tools` then `POST /api/v1/agent/tools/{name}/invoke`. Tools are permission-filtered. See [Agents](agents.md).

## Webhooks (outbound)

Qefro pushes events to your HTTPS endpoint after COMMIT. Delivery is **at-least-once**. Verify HMAC. See [Webhooks](webhooks.md).

## Inbound

Inbound HTTP should hit public forms or authenticated REST. Do not add a backdoor mutation path around `EntityService`.
