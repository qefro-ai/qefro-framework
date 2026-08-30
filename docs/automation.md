# Automation

`AutomationDef` is a declarative rule layer on top of the existing runtime. It is **not** a second EntityService, event bus, job queue, notification system, or webhook system.

```
Entity operation
      ↓
EntityService (transaction)
      ↓
COMMIT
      ↓
DomainEvent (outbox)
      ↓
AutomationDef (match + conditions)
      ↓
Actions → EntityService / NotificationDef / WebhookDef / JobQueue
```

AI, agents, and LLMs are out of scope. Automations are business metadata.

## How the primitives compose

| Type | Role |
|---|---|
| `OperationDef` | Named business action a user/API invokes (Confirm, Cancel). Handlers stay in Rust. |
| `NotificationDef` | Template: event or automation `notify` → in-app (and email job stub). |
| `WebhookDef` | Outbound HTTP delivery with HMAC. Secrets stay server-side (`secret_env`). |
| `AutomationDef` | When an event (or schedule) matches conditions, run those actions. |

## Definition

Automations live on the application module (same place as notifications and webhooks), not inside every `EntityDef`.

```rust
AppModule::new("restaurant")
    .automation(
        AutomationDef::new(
            "order_ready_notification",
            AutomationTrigger::event("workflow.transitioned"),
        )
        .conditions(Condition::all(vec![
            Condition::field_equals("entity", "Order"),
            Condition::field_equals("to_state", "Ready"),
        ]))
        .action(AutomationAction::notify("Staff")),
    )
```

```yaml
name: order_ready_notification
trigger:
  event: workflow.transitioned
conditions:
  all:
    - field: entity
      equals: Order
    - field: to_state
      equals: ready
actions:
  - notify:
      notification: order_ready
      role: Staff
```

## Triggers

| Trigger | Source |
|---|---|
| `entity.created` / `updated` / `deleted` | Existing DomainEvent names |
| `workflow.transitioned` | Workflow engine (payload includes `from` / `to`) |
| `scheduled` | Cron on `JobQueue` (`trigger.schedule: "0 9 * * *"`) |

There is no second event bus. Event JSON aliases (`event_type`, `record_id`, `actor`) sit beside `name`, `entity_id`, `user_id`.

Events are published **after COMMIT** via `qefro_outbox`.

## Conditions

Safe operators: `equals`, `not_equals`, `contains`, `in`, `not_in`, `greater_than` (`gt`), `less_than` (`lt`), `greater_or_equal` (`gte`), `less_or_equal` (`lte`), `is_empty`, `is_not_empty`. Compose with `all` / `any`. No Rust, JavaScript, SQL, or shell.

String equals is case-insensitive so `Ready` matches `ready`.

## Actions

Every action reuses existing infrastructure and the same RBAC, validation, workflow, and tenant isolation.

| Action | Goes through |
|---|---|
| `notify` | `NotificationDef` / in-app store / `notify.email` job |
| `send_communication` | named `CommunicationDef` → `communication.deliver` job (never calls a provider here) |
| `send_webhook` | named `WebhookDef` + `webhook.deliver` job |
| `update_entity` / `create_entity` / `assign` | `EntityService` |
| `transition` | `EntityService::transition` (OperationDef when bound, otherwise workflow; never a raw status PATCH) |
| `create_activity` / `create_comment` | Activity store / comment API |

`as_roles` is an explicit, auditable OpContext. It is never implied Admin. Event-triggered automations default to the actor's tenant roles. Scheduled automations without `as_roles` run as Worker and cannot mutate unless permissions allow.

## Idempotency

One logical execution per `(tenant_id, automation_id, event_id)` in `qefro_automation_executions`. Retries of the same event do not send duplicate notifications or webhooks.

Jobs keep internal statuses `pending` / `succeeded`. Client JSON may include aliases `queued` / `completed`.

## Scheduling

```yaml
trigger:
  type: scheduled
  schedule: "0 9 * * *"
```

The worker calls `enqueue_scheduled` and enqueues `automation.schedule` onto the existing `JobQueue` with `run_at`. Timezone is `AutomationDef.timezone`, else tenant `business.timezone`, else **UTC**. Slot identity (`YYYYMMDDHHMM`) is part of the idempotency key.

## Observability

Correlate `request_id` (HTTP) → `DomainEvent.id` (`event_id`) → `execution_id` (automation) → job id → webhook delivery id. Errors use snake_case codes `automation_failed` and `job_failed`. Secrets are never logged or returned.

## Permissions and tenancy

Automation cannot read or write another tenant. Updates still pass `EntityService` validation. Workflow state changes must use a transition operation. Studio inspects automations as version-controlled metadata; it is not a drag-and-drop builder.

The generic UI keeps showing notifications, activity, and status. It does not expose retry internals on the business timeline.
