# App Developer Guide

This is the handbook for building applications on Qefro. It covers every feature you can ship in a YAML or Rust app. Deep-dive references live next to each section; the [documentation index](index.md) lists them all.

**Mental model:** define the business once. Qefro generates PostgreSQL schema, REST, validation, a generic UI, workflows, reports, documents, automation, realtime, and agent tools. You do **not** write a React page, REST controller, or SQL migration per entity.

```
Define EntityDef / YAML
        ↓
EntityService (auth → tenant → RBAC → validation → workflow → handler → audit → COMMIT)
        ↓
REST · QefroClient · Generic UI · CLI · Agent tools · Public forms · Import
```

HTTP, the generic UI (`frontend/`), the CLI, and agents share that path. Authorization is always server-side. Hiding a button in the UI is not a security boundary.

---

## Contents

1. [Choose YAML or Rust](#1-choose-yaml-or-rust)
2. [Scaffold, validate, run](#2-scaffold-validate-run)
3. [App anatomy](#3-app-anatomy)
4. [Entities](#4-entities)
5. [Fields, types, and widgets](#5-fields-types-and-widgets)
6. [Relations](#6-relations)
7. [Child tables](#7-child-tables)
8. [Validation](#8-validation)
9. [Formulas](#9-formulas)
10. [Views (list, cards, kanban, calendar, chart)](#10-views)
11. [Forms and layouts](#11-forms-and-layouts)
12. [Detail views](#12-detail-views)
13. [Workflows](#13-workflows)
14. [Business operations](#14-business-operations)
15. [Permissions](#15-permissions)
16. [Field permissions](#16-field-permissions)
17. [Identity](#17-identity)
18. [Documents, numbering, print](#18-documents-numbering-print)
19. [Actions, links, and allow-on-submit](#19-actions-links-and-allow-on-submit)
20. [Singletons](#20-singletons)
21. [Attachments](#21-attachments)
22. [Activity, comments, and audit](#22-activity-comments-and-audit)
23. [Tasks](#23-tasks)
24. [Workspaces and navigation](#24-workspaces-and-navigation)
25. [Dashboards](#25-dashboards)
26. [Reports](#26-reports)
27. [Saved views and search](#27-saved-views-and-search)
28. [Theming](#28-theming)
29. [Public forms](#29-public-forms)
30. [CSV import](#30-csv-import)
31. [Events, jobs, automation](#31-events-jobs-automation)
32. [Notifications](#32-notifications)
33. [Webhooks](#33-webhooks)
34. [Realtime](#34-realtime)
35. [Agents](#35-agents)
36. [Studio](#36-studio)
37. [Seeds](#37-seeds)
38. [Packaging, lifecycle, tenants](#38-packaging-lifecycle-tenants)
39. [API and SDK](#39-api-and-sdk)
40. [CLI](#40-cli)
41. [Configuration and deployment](#41-configuration-and-deployment)
42. [Security rules for app developers](#42-security-rules-for-app-developers)

---

## 1. Choose YAML or Rust

Both paths use the same generic UI. Do not scaffold a second React app.

| Path | Command | Use when |
| --- | --- | --- |
| **YAML app** | `qefro app new myshop` | CRUD, relations, child tables, formulas, workflows, permissions, numbering, print, reports, dashboards, public forms, automation |
| **Rust app** | `qefro new my-app` | Same as YAML, plus **`OperationHandler`** (multi-record transactions, custom jobs) |

YAML apps cannot register `OperationHandler`s. Those live on `InstalledApp` in Rust. Restaurant and CRM follow that path. Inventory and Helpdesk are YAML benchmarks.

You can load extra YAML with `EntityDef::from_yaml` inside a Rust app. Do not fork `EntityForm`.

See [App development](app-development.md) and [Create an application](creating-an-app.md).

---

## 2. Scaffold, validate, run

Prerequisites: Rust/Cargo, Node.js 20+, PostgreSQL 16. Install the CLI and start Postgres:

```bash
cargo install qefro-cli
# from this repo: cargo install --path crates/qefro-cli --locked --force
docker compose up -d postgres
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro doctor
```

On macOS 26/27, if `sqlx` fails with `mis-aligned LINKEDIT string pool`:

```bash
CARGO_PROFILE_RELEASE_STRIP=none cargo install qefro-cli
```

Create and run a YAML app from the **framework repository root**:

```bash
qefro app new myshop
qefro app validate myshop
qefro app install myshop
qefro migrate --app myshop
qefro dev --app myshop
```

Register a tenant, then start the generic UI:

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"name":"Ada","email":"ada@example.com","password":"password123","tenant_name":"Demo","tenant_slug":"demo"}'

qefro tenant app enable demo myshop   # if the tenant already existed

cd frontend && npm install && npm run dev
```

There is no `qefro app migrate` or `qefro app run`. Schema is `qefro migrate`; the server is `qefro dev` (development) or `qefro serve` + `qefro worker` (production).

Inspect what was generated:

```bash
qefro entity list --app myshop
qefro entity show Customer --app myshop
qefro inspect Customer --app myshop
qefro routes --app myshop
qefro permissions --app myshop
qefro workflows --app myshop
qefro operations --app myshop
qefro tools --app myshop
```

Shop tutorial: [Build a fullstack application](fullstack.md). Restaurant walkthrough: [Getting started](getting-started.md).

---

## 3. App anatomy

```
apps/myshop/
├── app.toml              # package name, version, navigation, branding, dependencies
├── entities/             # EntityDef YAML
├── workflows/
├── permissions/
├── reports/
├── dashboards/
├── print_formats/
├── seeds/
├── hooks/                # declarative lifecycle hooks (no shell)
├── migrations/           # additive SQL recorded in qefro_app_migrations
├── assets/
└── README.md
```

`app.toml` describes the **package**. Entity, workflow, permission, report, and dashboard definitions stay in their directories. Do not duplicate them in the manifest.

```toml
name = "myshop"
version = "0.1.0"
label = "Myshop"
api_version = "1"
framework_version = ">=1.0,<2.0"

[dependencies]
# inventory = ">=1.0,<2.0"

[[navigation]]
label = "Customers"
entity = "Customer"
section = "Sales"
```

What an app contributes: entities, child tables, formulas, documents, workflows, permissions, reports, dashboards, print formats, seeds, default navigation, and (Rust only) business operations.

See [Applications](apps.md), [App dependencies](app-dependencies.md).

---

## 4. Entities

An entity is the unit of metadata. One definition produces schema, REST, UI, validation, audit, and tools.

```yaml
name: Company
label: Company
label_plural: Companies
display_field: name
soft_delete: true
audit: true
fields:
  - name: name
    type: string
    required: true
    searchable: true
  - name: website
    type: string
    ui:
      widget: url
  - name: status
    type: enum
    values: [Lead, Active, Inactive]
    required: true
    default: Lead
    ui:
      widget: status
views:
  list:
    columns:
      - field: name
      - field: status
        widget: status
      - field: website
  card:
    title: name
    subtitle: website
    fields: [status]
  kanban:
    group_by: status
    card:
      title: name
      subtitle: website
```

Rust equivalent:

```rust
EntityDef::new("Company")
    .field(FieldDef::string("name").required().searchable())
    .field(FieldDef::string("website").ui(UiConfig::url()))
    .field(FieldDef::enum_("status", vec!["Lead", "Active", "Inactive"]).required())
    .build();
```

Scaffold more files with `qefro entity create Contact --app myshop`.

Optional entity flags used throughout this guide: `workflow`, `attachments`, `document`, `public_form`, `child_tables`, `naming`, `print_format`, `.with_tasks()`, `.single()` (singleton).

Unknown relations fail startup with a suggestion (`Did you mean 'DiningTable'?`).

See [Entities](entities.md).

---

## 5. Fields, types, and widgets

**Storage type** and **widget** are independent. A decimal can render as currency. A string can render as a color picker.

### Field types

| Type | PostgreSQL | Default widget |
| --- | --- | --- |
| `string` | TEXT | text |
| `text` | TEXT | textarea |
| `integer` | BIGINT | number |
| `decimal` | NUMERIC(18,6) | number |
| `boolean` | BOOLEAN | checkbox |
| `date` | DATE | date |
| `time` | TIME | time |
| `datetime` | TIMESTAMPTZ | datetime |
| `uuid` | UUID | text |
| `enum` | TEXT | select / status |
| `json` | JSONB | json |
| `relation` | UUID | relation |
| `child_table` | (child rows) | child_table |

Convenience builders keep the storage type and set validation + widget: `email()`, `phone()`, `url()`, `color()`, `FieldDef::currency("amount")`, `percentage()`.

### UI field flags

`label`, `description`, `placeholder`, `help` / `help_text`, `hidden`, `disabled`, `readonly`, `required`, `list_visible` / `list`, `form_visible` / `form`, `detail_visible` / `detail`, `searchable`, `sortable`, `filterable`, `width`, `widget`, `widget_options`, `section`, `tab`, `order`, `visible_when`, `readonly_when`, `permission_level`, `allow_on_submit`, `search_weight`, `search_exact`.

```yaml
- name: price
  type: decimal
  ui:
    widget: currency
    widget_options:
      currency: INR
      precision: 2
- name: brand_color
  type: string
  ui:
    widget: color
- name: created_at
  type: datetime
  ui:
    widget: datetime
    widget_options:
      timezone: tenant
```

Built-in widgets: `text`, `textarea`, `number`, `currency`, `percentage`, `date`, `time`, `datetime`, `duration`, `color`, `select`, `multiselect`, `relation`, `checkbox`, `switch`, `radio`, `tags`, `phone`, `url`, `email`, `password`, `rich_text`, `markdown`, `file`, `image`, `json`, `status`, `child_table`.

Unknown widgets fall back to `text`. An application can register a custom widget in the frontend registry without changing framework core:

```ts
import { registerWidget } from "./metadata/registry";
registerWidget("table-status", TableStatusWidget);
```

Datetimes are stored as UTC and displayed in the tenant timezone. Dynamic defaults (`current_user`, `current_date`, `current_datetime`, `tenant_timezone`, `tenant_currency`) are applied in `EntityService::create`, not in React.

See [UI widgets](ui-widgets.md), [Widgets](widgets.md).

---

## 6. Relations

| Kind | Storage | UI |
| --- | --- | --- |
| many-to-one | UUID column | searchable relation picker |
| one-to-many | none (inverse filter) | related list on detail |
| child table | child table + parent FK | nested editable table |
| many-to-many | junction `{table}_{field}` | relation widget |

```yaml
# Contact → Company
- name: company_id
  type: relation
  required: true
  relation:
    target_entity: Company
    kind: many_to_one
  ui:
    widget: relation
    section: Details

# Company → Contacts (inverse)
- name: contacts
  type: relation
  relation:
    target_entity: Contact
    kind: one_to_many
    inverse_field: company_id
```

List/get expand many-to-one in `_expanded`. Inverse collections appear in `_related` / `_links`. Relation pickers can Open, Create (full form, then return), search, and clear. They are tenant-scoped; never send raw UUIDs in the UI.

A business entity that uses the `person_id` convention is listed automatically on Person detail. See [Identity](#17-identity).

---

## 7. Child tables

A parent owns nested child rows. One metadata definition produces schema, nested REST, validation, and a generic editable table.

```yaml
# StockEntry
child_tables:
  - name: items
    child_entity: StockEntryItem
    parent_field: entry_id
```

```rust
EntityDef::new("Order")
    .field(FieldDef::relation("customer_id", "Customer").required())
    .child_table(ChildTableDef::new("items", "OrderItem").parent_field("order_id"))
    .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
    .build();

EntityDef::new("OrderItem")
    .field(FieldDef::many_to_one("order_id", "Order").required().hidden())
    .field(FieldDef::relation("menu_item_id", "MenuItem").required())
    .field(FieldDef::integer("quantity").required().min(1.0))
    .child_of("Order", "items")
    .build();
```

`child_of` hides the child from navigation unless `.standalone()` is set. The child still has a table, REST, and RBAC.

Create/update accept the child array on the parent. Parent and children write in **one transaction**. Updates sync by child `id`: missing `id` inserts, omitted existing rows delete, foreign/tenant ids are rejected. Insertion order is a hidden `sort_order` column.

Child field errors are nested: `{ "field": "items.0.quantity", "message": "Quantity is required" }`.

See [Child tables](child-tables.md).

---

## 8. Validation

Authoritative validation runs on the server (`validate_record` + entity-level rules). The generic UI may mirror rules for UX. Hidden fields are still validated. `visible_when` is presentation-only.

### Field rules

| Rule | Semantics |
| --- | --- |
| `required` | Non-empty on create |
| `unique` | Tenant-scoped uniqueness |
| `min` / `max` | Inclusive bounds |
| `greater_than` / `less_than` | Exclusive bounds |
| `range` | Inclusive min and max |
| `min_length` / `max_length` | Character counts |
| `regex` | Pattern |
| `email` / `phone` / `url` | Format |

```yaml
- name: email
  type: string
  required: true
  validation:
    email: true
- name: quantity
  type: integer
  validation:
    greater_than: 0
```

### Entity-level rules

```yaml
validation:
  - field: email
    rule: email
  - field: quantity
    rule: greater_than
    value: 0
  - when:
      field: status
      equals: confirmed
    require:
      - customer_id
  - compare:
      field: end_date
      greater_than: start_date
  - field: customer_id
    rule: exists
```

There is no executable expression language. REST, SDK, import, public forms, and automation share the same checks.

See [Validation](validation.md).

---

## 9. Formulas

Computed fields are declared in metadata and evaluated on the server. The browser may preview; the server always recalculates and **ignores** client-supplied computed numbers.

```yaml
- name: amount
  type: decimal
  computed: quantity * unit_price
  ui:
    widget: currency
- name: subtotal
  type: decimal
  computed: SUM(items.amount)
- name: grand_total
  type: decimal
  computed: subtotal - discount
```

Language: arithmetic `+ - * / %`, parentheses, `SUM MIN MAX COUNT ROUND CONCAT`, field references (`quantity`, `items.amount`), string literals. Unknown functions and leftover SQL are rejected at parse time. Circular dependencies fail metadata validation.

Computed values are stored after calculation so they can be filtered and reported.

See [Formulas](formulas.md).

---

## 10. Views

You do not create pages. The generic UI reads `GET /api/v1/meta/ui`.

| View | When it appears |
| --- | --- |
| List | Always |
| Cards | Only if `views.card` is set |
| Kanban | `views.kanban`, or a workflow + status/enum grouping field |
| Calendar | `views.calendar`, or a non-system date/datetime field |
| Chart | `views.chart` |
| Form / Detail | Routes `/:slug/new`, `/:slug/:id`, `/:slug/:id/edit` |

```yaml
views:
  default: list
  list:
    columns:
      - field: name
        width: 240
      - field: status
        widget: status
      - field: total
        widget: currency
    default_sort:
      field: created_at
      direction: desc
  card:
    title: name
    subtitle: website
    fields: [status]
  kanban:
    group_by: status
    card:
      title: name
      subtitle: website
  calendar:
    start: reservation_date
    time: reservation_time
    title: guest_name
    subtitle: status
  chart:
    type: bar
    dimension: status
    measure:
      field: amount
      aggregation: sum
```

**Kanban:** if the grouping field is workflow status, drag calls `POST .../transition`, not `PATCH status`. Invalid moves show the server error and reload.

**Calendar:** empty-slot click opens the generic form with query defaults (`/reservations/new?reservation_date=…`). Drag reschedule is `PATCH` through `EntityService`. Locked documents cannot be rescheduled.

Do not force Cards or Kanban on entities that omit that metadata. Deep links: `/{slug}?view=list|kanban|calendar`.

See [Views](views.md), [List views](list-views.md), [Kanban](kanban.md), [Calendar](calendar.md), [View metadata](view-metadata.md), [UI 2.1](ui-2.md).

---

## 11. Forms and layouts

Generic create/edit pages are driven by metadata. There are no per-entity React form components.

Layout comes from field flags, or from `views.form.sections`:

```yaml
- name: notes
  type: text
  ui:
    widget: textarea
    section: Notes
    width: full
- name: cancellation_reason
  type: text
  ui:
    visible_when:
      field: status
      equals: Cancelled
```

| Metadata | Effect |
| --- | --- |
| `section` | Fieldset heading |
| `tab` | Tab on form and detail |
| `order` | Sort order |
| `width` | `full` (default), `half`, `third` |
| `visible_when` | Presentation-only show/hide |
| `readonly_when` | Presentation-only lock |

The renderer groups by `views.form.sections` when present, otherwise `tab` then `section` then `order` / `width`. Server `FieldError`s appear next to the matching input. Unsaved navigation warns Stay / Discard.

See [Forms](forms.md), [Layouts](layouts.md).

---

## 12. Detail views

`EntityDetail` is the generic document screen: number, status, owner, primary **Edit**, `_actions` (or workflow transitions), More (print, PDF, delete).

Tabs appear when the matching capability exists: **Details**, each child table, **Related records**, **Attachments**, **Activity**. Related links open the generic list with a filter — the frontend does not join tables.

GET responses include `_expanded`, `_related`, `_links`, `_workflow`, `_actions`, and `_permissions: { update, delete }` (chrome hints). The UI hides New/Edit/Delete from those hints; unauthorized writes still return **403**.

See [Detail views](detail-views.md).

---

## 13. Workflows

A workflow is a named state machine bound to one entity status field. **Status cannot be PATCHed.** Callers use a named transition or a business operation.

```yaml
# apps/myshop/workflows/company.yaml
name: company
entity: Company
initial: Lead
states:
  - name: Lead
  - name: Active
  - name: Inactive
    terminal: true
transitions:
  - name: activate
    from: Lead
    to: Active
    allowed_roles: [Staff, Manager]
  - name: deactivate
    from: Active
    to: Inactive
    allowed_roles: [Staff, Manager]
```

On the entity set `workflow: company`. Admin bypasses role lists. Invalid transitions fail with `invalid_transition` (HTTP 409) and do not mutate related records.

GET includes `_workflow` (allowed transitions with labels, confirmation) and `_actions`. The UI never PATCHes workflow status. Kanban drag, list row actions, cards, and detail all use the same metadata.

Prefer a **business operation** when the transition also mutates related records (seat a reservation and occupy a table). If a matching operation exists, `POST /{slug}/{id}/transition` delegates to `EntityService::execute`.

See [Workflows](workflows.md).

---

## 14. Business operations

CRUD is generated. Real processes — confirm a reservation, convert a lead — are **business operations**. They require Rust (`OperationHandler`). YAML workflows alone cannot update related records in one transaction.

```rust
app.operation(
    operation("confirm", "Reservation")
        .label("Confirm")
        .permission("reservation.confirm")
        .roles(&["Manager", "Staff"])
        .transition("confirm")
        .event("reservation.confirmed")
        .job("notify_reservation_confirmed"),
    ConfirmReservation,
);

async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
    let table_id = ctx.uuid_field("table_id")?;
    let table = ctx.get("DiningTable", table_id).await?;
    if table.get("status").and_then(|v| v.as_str()) != Some("available") {
        return Err(OperationCtx::fail(
            "table_unavailable",
            "The selected table is not available",
        ));
    }
    ctx.update("DiningTable", table_id, json!({ "status": "reserved" })).await?;
    ctx.apply_transition("confirm")?;
    Ok(ctx.record.clone())
}
```

Handlers already have the authenticated user and tenant. They must not extract auth themselves or run raw SQL. `QefroError::Business { code, message }` maps to HTTP 409. Stack traces are never sent to clients.

HTTP, UI, CLI (`qefro action Reservation <id> confirm`), and agents all call `EntityService::execute`.

See [Operations](operations.md). Restaurant and CRM examples live under `examples/`.

---

## 15. Permissions

Authorization is always server-side.

```
Auth → Tenant → App entitlement → RBAC → Field permissions → Validation → Workflow → EntityService
```

```yaml
# apps/myshop/permissions/staff.yaml
- role: Staff
  entity: Customer
  actions: [create, read, update, delete, list]
- role: Staff
  entity: Company
  actions: [create, read, update, delete, list]
- role: Public
  entity: Ticket
  actions: [create]
```

Admin bypasses role lists after login. Workers use role `Worker` and only `worker_safe` operations. Public forms use role `Public` with an allowlisted action set.

`GET /api/v1/meta/ui` includes per-entity `permissions` chrome hints. Search, reports, dashboards, and saved views use the same pipeline. Tenant isolation is `WHERE tenant_id = $1` plus a post-read check (mismatch is **404**, not 403).

See [Permissions](permissions.md).

---

## 16. Field permissions

Entity RBAC still gates CRUD. Field permissions then hide or reject individual fields.

| Level | Meaning |
| --- | --- |
| 0 | Normal |
| 1 | Restricted |
| 2 | Sensitive |
| 3 | Highly sensitive |

```yaml
- name: salary
  type: decimal
  permission_level: 2
  ui:
    widget: currency
```

```rust
FieldLevelGrant::new("HR", "Employee", 2)
FieldLevelGrant::new("Manager", "Employee", 1).read_only()
```

Reads strip unauthorized fields before the response. Writes of unauthorized keys return 403. Admin bypasses field levels after entity access.

See [Field permissions](field-permissions.md).

---

## 17. Identity

```
Person ≠ User ≠ Organization ≠ Business entity (Customer / Patient / Employee / Supplier)
```

| Concept | What it is |
| --- | --- |
| **Person** | Real-world individual (canonical name/email/phone once linked) |
| **Organization** | Company / legal entity |
| **User** | Optional login: password, roles, membership, enabled |
| **Customer / …** | Business record. May point at Person and/or Organization. Not a User. |

Link with nullable many-to-ones: `person_id`, `organization_id`, optional `party_type` = `Person` | `Organization`.

When `person_id` is set, Person is the source of truth for name/email/phone. The business entity still stores its own fields for unlinked and legacy rows — do not drop Customer name/email/phone.

Walk-in guest: create a Customer with name/email/phone, leave Person empty. Linked individual: Settings → People, then set Customer → Person. Create a User only if they should sign in.

Do not model Customer as User. Authentication stays in `qefro-auth`. Person, Organization, and User are `EntityDef`s so REST, UI, and agents go through `EntityService`.

See [Identity](identity.md).

---

## 18. Documents, numbering, print

Document behavior is metadata on an entity. There is no second execution engine.

```yaml
document:
  submit_enabled: true
  cancel_enabled: true
  lock_states: [Submitted]
naming:
  pattern: "INV-{YYYY}-{#####}"
  field: doc_no
  assign_on: submit   # or create
```

```rust
EntityDef::new("Invoice")
    .workflow("invoice")
    .document(
        DocumentConfig::new()
            .submit()
            .cancel()
            .duplicate()
            .lock_states(&["Submitted", "Cancelled"])
            .number_on("submit"),
    )
    .naming(NamingConfig::new("INV-{YYYY}-{#####}").field("doc_no").assign_on("submit"))
    .print_format(
        PrintFormat::new("Invoice Standard", "Invoice")
            .title("Invoice")
            .item_table("items")
            .total_fields(&["subtotal", "tax", "grand_total"]),
    )
```

When status is in `lock_states`, PATCH of ordinary fields is rejected. Fields with `allow_on_submit: true` remain writable. If submit/cancel/duplicate are enabled and the app did not register those operations, Qefro registers generic handlers.

Numbering tokens: `{YYYY}` / `{YY}`, `{MM}`, `{#####}` (padding = number of `#`). Sequences are per tenant, entity, and period. Concurrent requests cannot share a number.

Print:

- `GET /api/v1/{slug}/{id}/print` — HTML (tenant branding, locale, timezone, currency)
- `GET /api/v1/{slug}/{id}/print.pdf` — PDF of the same document

See [Documents](documents.md), [Numbering](numbering.md), [Print formats](print-formats.md), [Allow on submit](allow-on-submit.md).

---

## 19. Actions, links, and allow-on-submit

Actions are metadata over existing operations. Discovery is filtered by role; invocation is re-checked in `EntityService`.

```yaml
actions:
  - name: submit
    label: Submit
    operation: submit
  - name: create_invoice
    label: Create Invoice
    operation: create_invoice
    confirmation:
      required: true
      message: Create an invoice from this order?

links:
  - label: Invoices
    entity: Invoice
    relation: customer

fields:
  - name: remarks
    type: text
    allow_on_submit: true
```

GET includes `_actions` and `_links` with counts. The generic detail page never hardcodes business buttons. Confirmation is UI-only; the backend still enforces the operation.

See [Actions and links](actions-links.md).

---

## 20. Singletons

One document per tenant (restaurant settings, tax settings). Storage is the same entity table with a unique `(tenant_id)` constraint.

```rust
EntityDef::single("RestaurantSettings")
    .field(FieldDef::string("restaurant_name"))
    .field(FieldDef::string("timezone"))
    .build();
```

```http
GET  /api/v1/settings/{slug}
PATCH /api/v1/settings/{slug}
```

`GET` creates the row with defaults if none exists. A second `POST` returns 409. The generic list page renders a settings form instead of a table.

See [Singletons](singletons.md).

---

## 21. Attachments

Opt in on the entity:

```yaml
attachments: true
```

```rust
EntityDef::new("Ticket").attachments()
```

```http
GET    /api/v1/{slug}/{id}/attachments
POST   /api/v1/{slug}/{id}/attachments   (multipart)
GET    /api/v1/attachments/{id}
DELETE /api/v1/attachments/{id}
```

List, download, upload, and delete all load the owning record through `EntityService` first. Storage keys are generated server-side. Client-supplied paths, `..`, and `/` in filenames are rejected. MIME type and size (10 MiB) are validated. `storage_key` is never serialized. Guessing another tenant's id returns 404.

Uploads emit `attachment.created` and an Activity row. The generic detail page shows the list and upload control.

See [Attachments](attachments.md).

---

## 22. Activity, comments, and audit

**Activity** is the business-facing timeline on a record. **Audit** is the Admin-only security log. They are not the same store.

```http
GET  /api/v1/{slug}/{id}/activity
POST /api/v1/{slug}/{id}/comments   { "message": "…" }
GET  /api/v1/audit?entity=&entity_id=&limit=    # Admin only
```

Activity types: `created`, `updated`, `deleted`, `workflow_transition`, `comment`, `assignment`, `system`. Agent mutations show **Qefro Agent** as the actor. Secrets are stripped from metadata.

The generic Detail **Activity** tab renders the shared Timeline. Comments are Activity rows — there is no separate messaging product.

Audit records actor, tenant, entity, record, operation, old/new JSON, request id. Passwords, tokens, and `storage_key` are never stored. Staff never see `/settings/audit`.

See [Activity](activity.md), [Timeline](timeline.md), [Audit](audit.md).

---

## 23. Tasks

Task is a normal Qefro business object (title, assignee, due, workflow status). Applications opt a record in with `EntityDef::with_tasks()`. Do not add a `TaskService` or a custom Task page.

```
Customer / Order / Lead / Ticket
        └── Task (title, assignee, due, Open → In Progress → Completed / Cancelled)
```

Platform `Task` is at `/api/v1/tasks`. `assigned_to` defaults to the current user. `entity_type` / `entity_id` point at the related record. Overdue is derived, not stored. Permissions use the same matrix (`Task` Create/Read/Update/Delete).

See [Tasks](tasks.md).

---

## 24. Workspaces and navigation

A workspace is navigation plus a default dashboard, derived from `app.toml` / `AppModule` — not hardcoded chrome.

```toml
[[navigation]]
label = "Tickets"
entity = "Ticket"

[[navigation]]
label = "Board"
entity = "Ticket"
query = "status=Open"
view = "kanban"
section = "Operations"
```

```rust
AppModule::new("restaurant")
    .nav(NavItem::new("Orders", "Order").section("Operations"))
    .nav(NavItem::new("Kitchen", "Order").query("status=Preparing").view("kanban").section("Operations"))
    .dashboard(dashboard::ops())
```

`GET /api/v1/meta/workspace` (also nested in `/meta/ui`) returns navigation, default dashboard, reports, and permission-filtered shortcuts. `TenantUiConfig.navigation` can override the slug list. The homepage loads `GET /api/v1/dashboards/{name}`.

See [Workspaces](workspaces.md).

---

## 25. Dashboards

Dashboard definitions stay in application metadata. The frontend does not embed SQL.

```yaml
# apps/helpdesk/dashboards/helpdesk.yaml
name: helpdesk
label: Helpdesk
cards:
  - title: Open tickets
    entity: Ticket
    metric: count
    kind: metric
    filters:
      - field: status
        value: Open
```

```rust
DashboardDef::new("restaurant-ops", "Floor operations")
    .card(DashboardCard::kpi("Today's reservations", "Reservation").filter("reservation_date", "today"))
    .card(DashboardCard::sum("Today's sales", "Payment", "amount").filter("status", "captured").roles(&["Admin", "Manager"]))
    .card(DashboardCard::workflow("Kitchen status", "Order"))
    .card(DashboardCard::chart("Sales trend", "Order", "area", "order_date").metric_name("sum").measure_field("grand_total"))
    .card(DashboardCard::activity("Recent order events", "Order", 8))
    .card(DashboardCard::audit("Changes today").roles(&["Admin"]))
```

| kind | Payload |
| --- | --- |
| `metric` / `kpi` | count, sum, avg, min, max |
| `chart` / `workflow` / `status_breakdown` | series + chart type (`bar`, `line`, `area`, `pie`, `donut`) |
| `list` / `table` / `saved_view` | items from the entity list API |
| `activity` | `qefro_activity` |
| `report` | `run_report` |
| `audit` | Admin only |

Unauthorized widgets are **skipped**, not 403 for the whole dashboard. Use `roles` on a card instead of duplicating dashboards. Metric cards drill into the generic list using the card's filters.

See [Dashboards](dashboards.md), [Dashboard drill-down](dashboard-drilldown.md).

---

## 26. Reports

Metadata-driven reports reuse the filter engine. Arbitrary SQL is rejected.

```yaml
# apps/inventory/reports/stock_by_warehouse.yaml
name: stock_by_warehouse
label: Stock by warehouse
entity: StockBalance
fields: [warehouse_id, product_id, qty]
group_by: [warehouse_id]
aggregations:
  qty: sum
chart: bar
```

Aggregations: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`. `SUM`/`AVG` require a numeric field. Execution is server-side (`LIMIT 500` groups). Filters: `equals`, `not equals`, `contains`, `starts with`, `between`, `in`, `not in`, `empty`, `not empty`, `greater than`, `less than`. Payloads with `sql` or `query` keys are rejected.

```http
GET  /api/v1/{slug}/aggregates?group_by=status&metric=sum&field=amount
POST /api/v1/reports/{name}/run
GET  /api/v1/meta/reports
```

Reports honor tenant isolation, entitlements, RBAC list permission, and hidden fields. Agents use `EntityOps::run_report`.

See [Reports](reports.md).

---

## 27. Saved views and search

Users save the current list combination of filters, sort, columns, view type, and search:

```http
GET    /api/v1/saved-views?entity=Customer
POST   /api/v1/saved-views
DELETE /api/v1/saved-views/{id}
```

Rows are tenant-scoped **and** user-scoped. A user cannot reopen another user's view.

**Global search** (`GET /api/v1/search?q=Ahmed`) is PostgreSQL `ILIKE` over fields marked `searchable: true`. Secret fields are never searched. Modes: exact (`search_exact` or quoted query), prefix (`Ahmed*`), contains. Each entity is skipped unless the caller has `list` permission. Hits go through `EntityService` so field permissions strip snippets.

```yaml
- name: name
  type: string
  searchable: true
  search_weight: 10
- name: code
  type: string
  searchable: true
  search_exact: true
```

The command palette (`⌘K`) calls the same search API. List `?search=` reuses searchable metadata.

See [Saved views](views.md), [Search](search.md).

---

## 28. Theming

Tenant branding (logo, favicon, accent / primary / secondary) comes from tenant settings, then the app's `[branding]` in `app.toml`, then the tenant name. The renderer sets CSS variables. **Arbitrary tenant CSS or JavaScript is rejected.**

User appearance (this device, scoped to tenant + user): theme `light` | `dark` | `system`, density `comfortable` | `compact`, sidebar collapsed.

See [Theming](theming.md), [Tenants](tenants.md).

---

## 29. Public forms

Tenant-scoped forms that do not require an internal login. Example: `/p/{tenant-slug}/ticket`.

```yaml
public_form:
  enabled: true
  slug: ticket
  title: Submit a ticket
  fields: [subject, description, email]
  success_message: Ticket received
```

Only listed fields are accepted or returned. `tenant_id` and internal fields are stripped. Tenant is resolved from the **route slug**, never from the body. Submission uses `OpContext::public` with role `Public`, then `EntityService::create`. Rate limiting applies. After success the UI shows the configured message and a reference id, not the full internal record.

Grant Public only the actions it needs (typically `create`).

See [Public forms](public-forms.md). Helpdesk ships this pattern.

---

## 30. CSV import

Generic import for any entity the user can create. Preview writes nothing. Import calls `EntityService::create` per row (batches of 100). Validation, RBAC, formulas, workflows, audit, and tenant isolation all apply. There is no `COPY` bypass.

```http
POST /api/v1/{slug}/import/preview
POST /api/v1/{slug}/import
```

Mapping: `ignore`, map column → field, or a default value. Partial success is explicit (`imported` / `failed` / `errors`).

See [CSV import](imports.md).

---

## 31. Events, jobs, automation

### Events

Business facts emitted **after COMMIT**. A rolled-back operation does not emit a successful event.

Framework names: `{entity}.created|updated|deleted`, `entity.created` / `updated` / `deleted`, `workflow.transitioned`, `comment.created`, `attachment.created`, `user.disabled`, plus names on `OperationDef::event`. Delivery is **at-least-once** via `qefro_outbox`. Consumers should deduplicate on `id` + `tenant_id`.

See [Events](events.md).

### Jobs

PostgreSQL-backed queue. No Kafka, RabbitMQ, or Redis.

```rust
ctx.enqueue_job("notify_reservation_confirmed", json!({ "entity_id": id }));
app.job("notify_reservation_confirmed", LogNotificationJob)
```

Jobs listed on `OperationDef::job` enqueue inside the operation transaction. Workers claim with `FOR UPDATE SKIP LOCKED`. Workers do **not** run as Admin — they use an explicit `WorkerPolicy`. Production: `qefro serve` with `QEFRO_EMBED_WORKER=false` and a separate `qefro worker`.

See [Jobs](jobs.md).

### Automation

`AutomationDef` is a declarative rule layer on the existing event path. It is not a second EntityService, bus, or queue.

```yaml
name: order_ready_notification
trigger:
  event: workflow.transitioned
conditions:
  all:
    - field: entity
      equals: Order
    - field: to_state
      equals: Ready
actions:
  - notify:
      notification: order_ready
      role: Staff
```

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

Triggers: `entity.created` / `updated` / `deleted`, `workflow.transitioned`, `scheduled` (cron). Conditions: `equals`, `not_equals`, `contains`, `in`, `gt`/`lt`/`gte`/`lte`, `is_empty`, compose with `all` / `any`. No Rust, JavaScript, SQL, or shell. Actions call EntityService / NotificationDef / WebhookDef / JobQueue.

See [Automation](automation.md).

---

## 32. Notifications

Notification rules are metadata, not entity methods.

```yaml
notification:
  name: reservation_confirmed
  event: reservation.confirmed
  channels: [in_app, email]
  recipients: [Staff, Manager]
```

Nothing is sent before COMMIT. Channel failures do not roll back the business transaction. Recipients are filtered by role; users without entity access are not notified. In-app rows live in `qefro_notifications`. The generic shell shows a bell.

```http
GET  /api/v1/notifications
POST /api/v1/notifications/{id}/read
```

Channels: `in_app` (Postgres) and `email` (job). `webhook` reuses the webhook dispatcher.

See [Notifications](notifications.md).

---

## 33. Webhooks

Event-driven HTTP callbacks after COMMIT, using the job queue.

```yaml
webhook:
  name: order_created
  event: order.created
  target: https://example.com/webhook
```

Deliveries include `X-Qefro-Event`, `X-Qefro-Event-ID`, `X-Qefro-Timestamp`, `X-Qefro-Signature` (`sha256=HMAC(secret, "{timestamp}.{event_id}.{body}")`). The secret comes from `secret_env` or `QEFRO_WEBHOOK_SECRET`. Studio never returns secrets.

**At-least-once.** Verify HMAC and ignore duplicate event ids. Failed HTTP re-enters the queue with backoff.

```http
GET  /api/v1/webhooks
GET  /api/v1/webhooks/{name}/deliveries
POST /api/v1/webhooks/{name}/test
```

Admin only.

See [Webhooks](webhooks.md).

---

## 34. Realtime

Post-commit events fan out over **SSE** (`GET /api/v1/realtime`). WebSockets are not implemented.

Query filters: `entity`, `record_id`. Tenant comes from the session. A record subscription requires Read **and** a successful `EntityService::get`.

```json
{ "event": "order.updated", "entity": "Order", "record_id": "...", "changed_fields": ["status"] }
```

The generic list, detail, dashboard, and notification UI refresh on events. Authorization is never delegated to the client. Heartbeat every 15 seconds; slow clients are disconnected.

See [Realtime](realtime.md).

---

## 35. Agents

Qefro generates a tool per entity operation. Tools **never** receive a database connection.

```
Agent → Tool Registry → Authentication → Tenant Context
      → Permission Check → Validation → Workflow → Business Operation → Audit/Event
```

```http
GET  /api/v1/tools
GET  /api/v1/agent/tools
POST /api/v1/agent/tools/{name}/invoke
```

Generated operations: `create`, `get`, `find`, `update`, `delete`, `transition` (if workflow), `list_activity` / `comment`, `list_attachments`, workspace `search` / `run_report` / `get_dashboard`, and every registered business operation. The list is filtered by CRUD permission **and** `OperationDef.roles`. Invoke still re-checks. Knowing a tool name does not make it executable.

See [Agents](agents.md).

---

## 36. Studio

Qefro Studio (`/studio`) is the developer/admin console for application metadata. It is **not** a visual no-code builder. It reads the same registries the runtime already uses.

| Capability | Typical roles |
| --- | --- |
| `studio.view` | Admin, StudioViewer, PlatformAdmin |
| `studio.edit` | Admin, StudioEditor, PlatformAdmin |
| `studio.publish` | StudioPublisher, PlatformAdmin; Admin **only in development** |
| `studio.manage_apps` | StudioAppManager, PlatformAdmin |
| `studio.manage_permissions` | Admin, StudioPermissionManager, PlatformAdmin |
| `studio.manage_workflows` | Admin, StudioWorkflowManager, PlatformAdmin |

Staff and Customer have no Studio access.

**Development:** Admin can draft, validate, and publish overlays, including additive `ADD COLUMN` after confirmation.

**Production:** Admin can inspect and change **tenant** configuration (branding, navigation, terminology, locale). Publishing application metadata requires `StudioPublisher` / `PlatformAdmin`. Destructive changes (type change, field delete, entity rename) are rejected.

YAML apps: Studio can write the changed entity file after validation. Rust apps: inspect and overlay presentation; it does **not** rewrite `OperationHandler`.

See [Studio](studio.md), [Studio publishing](studio-publishing.md), [Studio entities](studio-entities.md), [Studio workflows](studio-workflows.md), [Studio permissions](studio-permissions.md).

---

## 37. Seeds

Seed batches ship with the application package. They are tenant-aware and idempotent (`unique_by`).

| Kind | When it runs |
| --- | --- |
| `system` | Non-tenant platform rows |
| `install` | When a tenant first enables the app |
| `tenant` | Explicit `qefro app seed myshop --tenant demo` |
| `development` | Only when `QEFRO_ENV=development` |

```yaml
kind: tenant
entity: Warehouse
unique_by: [code]
records:
  - code: MAIN
    name: Main warehouse
```

See [Applications](apps.md) (seeds section of the package layout).

---

## 38. Packaging, lifecycle, tenants

```
Build → Validate → Package → Install → Migrate → Enable for tenant → Configure → Update → Disable / Uninstall
```

```bash
qefro app validate myshop
qefro app package myshop          # writes myshop-0.1.0.qefro
qefro app install myshop-0.1.0.qefro
qefro migrate --app myshop
qefro tenant app enable demo myshop
qefro app update myshop
qefro app disable myshop          # global; data kept
qefro app uninstall myshop        # unregistered; data kept
```

A `.qefro` file is a ZIP of definitions plus `qefro-package.json`. It does not contain PostgreSQL data, JWT secrets, or `.env` files. Treat packages as untrusted input.

Installed globally ≠ enabled for every tenant. Entitlements: `installed ∩ tenant.enabled_apps ∩ plan.apps`. The client cannot enable an app by editing a request.

`tenant_id` is taken from the authenticated session, never from the body, query string, or `X-Tenant-ID`. Supplying `tenant_id` in a payload returns 400. Tenant A cannot read Tenant B (**404**).

New fields are added by schema apply. Fields removed from metadata are **reported** and left in PostgreSQL. There is no automatic destructive uninstall.

See [App packaging](app-packaging.md), [App lifecycle](app-lifecycle.md), [Multi-tenancy](multitenancy.md), [Licenses](licenses.md).

---

## 39. API and SDK

Base URL: `/api/v1`. Auth: `POST /auth/register`, `/auth/login`; Bearer token thereafter.

Generated per entity slug:

```
GET/POST    /{slug}
GET/PATCH/DELETE /{slug}/{id}
POST        /{slug}/{id}/transition
POST        /{slug}/{id}/actions/{name}
GET         /{slug}/{id}/activity
POST        /{slug}/{id}/comments
GET/POST    /{slug}/{id}/attachments
GET         /{slug}/{id}/print
GET         /{slug}/{id}/print.pdf
POST        /{slug}/import
```

Metadata: `/meta/ui`, `/meta/entities`, `/meta/workflows`, `/meta/permissions`, `/meta/dashboards`, `/meta/reports`, `/meta/workspace`.

**Browser SDK:** `QefroClient` in `@qefro/js` (see [QefroClient](sdk.md) and [qefro.js](qefro-js.md)). Methods: `ui()`, `list` / `get` / `create` / `update` / `remove`, `action` / `transition`, `search`, `getDashboard`, `runReport`, `activity`, `attachments`, `notifications`, saved views. There is no `IdentityClient` or `WorkflowClient`.

**Agents:** `EntityOps` in-process through `EntityService`.

External connectors call REST or tool invoke. They must never receive a SQLx pool or raw SQL.

See [API](api.md), [QefroClient](sdk.md), [Connectors](connectors.md).

---

## 40. CLI

```bash
qefro new my-app                      # Rust project
qefro app new myshop                  # YAML app under apps/
qefro app validate|package|install|update|info|list
qefro app enable|disable|uninstall myshop
qefro app seed myshop --tenant demo
qefro entity list|show|create
qefro inspect Customer
qefro migrate --app myshop
qefro dev --app myshop
qefro serve --app myshop              # production HTTP
qefro worker                          # production jobs
qefro routes|permissions|workflows|operations|tools
qefro action Reservation <id> confirm
qefro tenant app enable demo myshop
qefro doctor
qefro validate myshop
```

`qefro app remove` is an alias of `uninstall`.

---

## 41. Configuration and deployment

Qefro reads process configuration from the environment. Copy `.env.example` to `.env` locally. Never commit secrets.

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | PostgreSQL (required) |
| `JWT_SECRET` | Session signing (must not be the development default in production) |
| `QEFRO_BIND` | Listen address (default `127.0.0.1:8080`) |
| `QEFRO_ENV` | `development` or `production` |
| `QEFRO_EMBED_WORKER` | HTTP process also polls jobs (default on in development) |
| `QEFRO_STORAGE_PATH` | Local blob root |

Architecture: one HTTP process, one worker, one PostgreSQL, one generic frontend. Redis is not required.

```
qefro migrate && qefro serve     # + qefro worker when QEFRO_EMBED_WORKER=false
```

Modes: shared SaaS (many tenants, one runtime), dedicated tenant VM, or enterprise self-hosted. Same binary.

See [Configuration](configuration.md), [Deployment](deployment.md).

---

## 42. Security rules for app developers

- Never bypass `EntityService`. No raw SQL in handlers for tenant data.
- Never take `tenant_id` from the client.
- Never treat UI hiding as authorization.
- Never give agents or connectors a database connection.
- Never log or return secrets (`password`, tokens, `storage_key`, webhook secrets).
- Never execute tenant CSS/JS, formula SQL, or report SQL.
- Public forms: allowlist fields and Public RBAC; rate-limit.
- Webhooks: verify HMAC; expect at-least-once delivery.
- Workers: `Worker` role only, `worker_safe` operations.
- Studio publish in production is not a tenant Admin default.

See [Security](security.md) and [Threat model](threat-model.md).

---

## Next

| If you want to… | Read |
| --- | --- |
| Scaffold and click through the generic UI | [Create an application](creating-an-app.md) |
| Build customers / products / orders | [Fullstack tutorial](fullstack.md) |
| Understand the runtime | [Architecture](architecture.md), [Business object runtime](business-object-runtime.md) |
| Browse every doc | [Documentation index](index.md) |
| Copy a working app | [Examples](examples.md), `apps/helpdesk`, `apps/inventory` |
