# Automation

`AutomationDef` is a declarative rule layer on top of the existing runtime. It is **not** a second EntityService, event bus, job queue, notification system, workflow engine, or scheduler.

```
                       Business Event
                             │
                             ▼
                           Outbox
                             │
                             ▼
                       Automation Engine
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
          Condition        Action          Wait
              │              │              │
              └──────────────┼──────────────┘
                             ▼
                       Existing Runtime
```

> **Workflow defines what a business record can do. Automation defines what Qefro should do when something happens.**

AI, agents, and LLMs are out of scope. Automations are business metadata.

## Workflow vs Automation

| | Workflow | Automation |
|---|---|---|
| Scope | One record's lifecycle | Reactions to events |
| Example | Draft → Confirm → Confirmed | Order confirmed → wait → notify |
| Engine | `WorkflowDef` / `TransitionDef` | `AutomationDef` / `AutomationEngine` |
| May invoke the other | No — workflow is not an automation engine | Yes — `transition` goes through EntityService |

## How the primitives compose

| Type | Role |
|---|---|
| `OperationDef` | Named business action a user/API invokes (Confirm, Cancel). Handlers stay in Rust. |
| `NotificationDef` | Template: event or automation `notify` → in-app (and email job stub). |
| `WebhookDef` | Outbound HTTP delivery with HMAC. Secrets stay server-side (`secret_env`). |
| `CommunicationDef` | Channel templates. Automation selects template/channel/recipient; it never calls a provider. |
| `AutomationDef` | When an event (or schedule) matches, run steps through EntityService / JobQueue. |
| `JobQueue` | Delayed wait, retries, scheduled triggers. Same queue as communications and webhooks. |
| `Condition` | Shared predicate DSL (`equals`, `all` / `any`). No second expression language. |

## Definition

Automations live on the application module (same place as notifications and webhooks), not inside every `EntityDef`.

```yaml
name: order_confirmed_followup
trigger:
  event: order.confirmed
steps:
  - send_communication:
      template: order_confirmed
  - wait: 30m
  - condition:
      field: status
      equals: Preparing
    then:
      - notify:
          role: Manager
```

Linear `actions:` is still supported. When `steps` is empty, actions run in order.

## Triggers

| Trigger | Source |
|---|---|
| `entity.created` / `updated` / `deleted` | Existing DomainEvent names |
| `workflow.transitioned` | Workflow engine (payload includes `from` / `to`) |
| Field change | Same events / workflow payload (`from` → `to`, current fields after refresh) |
| `scheduled` | Cron on `JobQueue` (`trigger.schedule: "0 9 * * *"`) |

There is no second event bus. Events are published **after COMMIT** via `qefro_outbox`. Automation never holds an HTTP request open.

## Conditions

Safe operators: `equals`, `not_equals`, `contains`, `in`, `not_in`, `greater_than` (`gt`), `less_than` (`lt`), `greater_or_equal` (`gte`), `less_or_equal` (`lte`), `is_empty`, `is_not_empty`. Compose with `all` / `any` (AND / OR). There is no `NOT` node; use `not_equals`. No Rust, JavaScript, SQL, or shell.

After a wait, the engine refreshes the record through EntityService so `status == Preparing` sees current data.

## Steps

| Node | Meaning |
|---|---|
| Action | Existing `notify`, `send_communication`, `transition`, `create_entity`, `update_entity`, `assign`, `create_activity`, `create_comment`, `print_document`, `send_webhook` |
| Wait | `30m` / `1h` / `3d` or `{ until_field: due_date }` — persisted `run_at` on JobQueue |
| Branch | Condition → `then` / `else` |
| End | Stop |

Wait-until uses the record field and schedules `run_at`. It does not poll.

## Actions

Every action reuses existing infrastructure and the same RBAC, validation, workflow, row policies, audit, and tenant isolation. Never write tables directly. If a field is workflow-controlled, use `transition`.

| Action | Goes through |
|---|---|
| `notify` | `NotificationDef` / in-app store / `notify.email` job |
| `send_communication` | named `CommunicationDef` → `communication.deliver` job |
| `send_webhook` | named `WebhookDef` + `webhook.deliver` job |
| `update_entity` / `create_entity` / `assign` | `EntityService` |
| `transition` | `EntityService::transition` |
| `print_document` | Document runtime |
| `create_activity` / `create_comment` | Activity store. Actor label is **Qefro Automation** |

`{{entity}}` and `{{record_id}}` interpolate from the event. Arbitrary server objects are not exposed.

`as_roles` is an explicit, auditable OpContext. It is never implied Admin. Event-triggered automations default to the actor's tenant roles. Scheduled automations without `as_roles` run as Worker. `source = automation` is tenant-scoped and authorized; it does not bypass row policies.

## Delay, jobs, retries

Wait persists `cursor` + `def_snapshot` on `qefro_automation_executions` and enqueues `automation.run` with `run_at`. A process restart resumes from the snapshot, so publishing v2 does not rewrite in-flight runs.

Retries use JobQueue `max_attempts` (default 5, configurable on the def) and exponential backoff. A failed **step** does not mark the source business entity failed.

Disabled automations do not start new runs. In-flight waiting jobs continue from their snapshot.

## Idempotency

One logical execution per `(tenant_id, automation_id, event_id)`. Step retries keep the same cursor so communication/webhook jobs reuse their idempotency keys.

## Loop prevention

Mutations from automation attach `_automation_depth` on the DomainEvent payload. Matching stops at `max_depth` (default 8).

## Execution history

Authorized users inspect tenant-scoped runs: `started` / `waiting` / `retrying` / `completed` / `failed`, the record, and a step log. Errors use `public_message` (no secrets or stack traces).

Studio and the generic `AutomationRuns` component show this. Internal wait/condition steps are not dumped onto the business timeline; meaningful `create_activity` / communication actions are.

## Studio

Studio is a visual editor for the same primitives: Trigger, Action, Wait, Condition, End. Drag to reorder, connect branches, publish, disable, and dry-run. Dry-run plans steps and does **not** send communications.

Statuses: **Draft** (unpublished Studio draft), **Published** (`enabled`), **Disabled**. Only published automations execute.

Validation (Studio and `qefro validate`): missing trigger, unknown action/entity/field, invalid wait/transition/condition, empty communication template, unreachable step after End, secrets in metadata.

## CLI and SDK

```bash
qefro inspect automation order_confirmed_followup
qefro validate
```

`QefroClient` lists, inspects, previews, and reads runs through the existing Studio API. There is no separate automation SDK.

## Security

- Tenant A workers never read or send as tenant B.
- Row policies and permissions still apply to EntityService calls.
- Workflow status cannot be patched; use `transition`.
- Metadata must not contain API keys, passwords, tokens, or provider credentials.
- No arbitrary code nodes.

## Examples

Restaurant: `order.confirmed` → send `order_confirmed` → wait 30m → if `status == Preparing` → notify Manager.

CRM: `opportunity.won` → send onboarding communication → create Task → notify Manager.

Commerce/Accounting: `invoice.issued` → wait until `due_date` → if still `Issued` → send reminder. Ledgers are not touched; only Communication / EntityService operations run.
