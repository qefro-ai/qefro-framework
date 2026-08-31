# Communications

Communication is metadata on existing entities. It does not replace notifications, events, automation, or jobs. Business logic decides what happened. Communication decides how to tell someone about it.

```text
Qefro Business Event
         │
         ▼
      Outbox
         │
         ▼
  Automation / CommunicationDef
         │
         ▼
  communication.deliver job
         │
    ┌────┼────┐
    ▼    ▼    ▼
  Email SMS WhatsApp / in-app
         │
         ▼
   Delivery log
         │
    ┌────┼────┐
    ▼    ▼    ▼
 Activity Audit Notification
```

The entity never knows whether the message went through email, SMS, WhatsApp, or in-app.

Staff in-app alerts remain [`NotificationDef`](notifications.md). `CommunicationDef` is the customer-facing template (and in-app when the recipient has a login). Do not put `if order: send_whatsapp(...)` on application entities.

## Channels

```text
in_app
email
sms
whatsapp
```

Only these four are valid. Ordered fallback is enough: preferred channel first, then the rest of the template's list. The first channel that has an address is queued. Preference `none` skips send.

## Templates

```rust
CommunicationDef::new("order_confirmed", "order.confirmed", "Order")
    .channels(&[CHANNEL_WHATSAPP, CHANNEL_EMAIL, CHANNEL_IN_APP])
    .purpose(PURPOSE_TRANSACTIONAL)
    .subject("Order {{ number }} confirmed")
    .body("Hello {{ customer.name }},\nyour order {{ number }} is confirmed.\nTotal: {{ total | currency }}")
    .recipient_path("customer")
    .preferred_channel_field("communication_channel")
    .opt_out_field("marketing_opt_out")
```

YAML under `communications/` is equivalent. Templates reuse the document renderer (`{{ field }}`, `| currency` / `| date` / `| time` / `| number`). Locale and currency come from the tenant. Paths resolve against EntityDef fields and relations (`customer` → `customer_id`). `number` aliases `doc_no`.

Templates are declarative. They must not execute JavaScript, Rust, Python, SQL, shell, filesystem, or network. `qefro validate` rejects unknown entities, unknown fields, invalid relations, and unsafe markup.

## Purpose and consent

`purpose` is `transactional` or `marketing`. Marketing honors an optional boolean on the recipient (`opt_out_field`, for example `marketing_opt_out`). Transactional messages ignore that flag. Qefro does not silently classify every message as marketing.

Preferred channel (`communication_channel`) is opt-in. Applications that do not define the field still send using the template's channel list.

## Recipients

Identity stays `Person ≠ User ≠ Customer`. Recipients come from entity data (`recipient_path`, then nested `person` when `person_id` is set). A customer does not need a login. In-app without `user_id` is skipped, not failed.

## Providers

```text
Communication → Channel → Provider
```

`CommunicationProvider::send` is the boundary. The default loggers write tracing lines only. They are not SMTP, Meta, Twilio, or a gateway. Applications replace a channel with their own adapter. Credentials use existing secret / configuration mechanisms — never metadata, git, or the delivery log.

Tests use `RecordingProvider` (`MockEmail` / `MockSms` / `MockWhatsApp` equivalents). Tests must not call real vendors.

## Delivery lifecycle

```text
queued → sending → sent (→ delivered)
                 ↘ failed → retry → dead_letter
```

External send always runs on JobQueue (`communication.deliver`). HTTP and EntityService mutations never wait on a provider. After COMMIT, Outbox publishes the domain event; `CommunicationDispatcher` enqueues the job.

Retries use the existing job backoff (cap 5 minutes, max 5 attempts). After the limit the log is `dead_letter`. The business transaction stays successful: an unavailable WhatsApp provider does not fail Confirm Order.

## Outbox and idempotency

```text
Confirm Order → DB transaction → outbox event → COMMIT → communication job
```

Nothing is sent before COMMIT. The unique key is `(tenant_id, template, entity_id, event_id, channel)` plus the job `idempotency_key`. A retried `order.confirmed` event does not send WhatsApp × 2. Manual **Send message** uses a new event id so an authorized resend is allowed.

## Attachments

`attach_document: true` attaches filenames/ids already stored by the File/Document runtimes. Communication does not generate a PDF. Internal storage keys are never exposed.

## Activity, audit, log

Queued and sent appear once on the business timeline (`Email notification queued`, `WhatsApp sent`). Job retries do not flood Activity. Studio / provider / template publishes go through existing Audit.

`qefro_communications` is the tenant-scoped operational log: recipient, channel, template, status, timestamps. Message bodies and provider secrets are not stored. Search is by recipient, entity, record, channel, status — not body text.

## Permissions and tenancy

Send and read use the source entity's **Read** permission (same overlay as `generate_document`). Arbitrary recipient overrides require Admin. Tenant A cannot see Tenant B logs, templates, or credentials, including from workers.

## REST / SDK

Same Qefro conventions. There is no separate messaging API.

- `GET /api/v1/{slug}/{id}/communications`
- `GET /api/v1/communications?entity=&record=&channel=&status=&recipient=`
- `POST /api/v1/{slug}/{id}/actions/send_communication` `{ template, channel? }`

The SDK calls `sendCommunication` / `communications` on the existing client. Automation uses `AutomationAction::send_communication("template")`.

## Studio / CLI

Studio **Templates** edits channel, subject, body, and variables. Preview renders sample entity data and never sends (`sent: false`). `qefro inspect` lists Communication on the entity. `qefro validate` checks templates against EntityDef.

## Examples

Restaurant: `order.confirmed` / `reservation.confirmed` → customer preferred channel.

CRM: empty `event` on the template plus `AutomationDef` `send_communication` when an opportunity is won (avoids double-send with the dispatcher).

Commerce: `invoice.issued`, `payment.received`, `sales_order_confirmed` on the platform sales entities.

Do not implement AI messaging, campaign managers, SMTP servers, WhatsApp clones, or push infrastructure here.
