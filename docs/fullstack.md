# Build a fullstack application

Qefro is one Axum process, one PostgreSQL database, and one generic React UI. You do not write a REST controller per entity, and you do not write a React page per entity.

Define the business once — as YAML files or as Rust `EntityDef`s. The runtime generates schema, CRUD APIs, validation, tenant isolation, list/form/detail screens, and (when you add them) workflows, numbering, print, and reports.

This guide builds a small shop: customers, products, and orders with line items. For a shorter path from `qefro app new` to a running generic UI, see [Create an application](creating-an-app.md). Feature-by-feature handbook: [App Developer Guide](developer-guide.md).

## What the stack is

```
Browser (frontend/, Vite :5173)
        │  /api proxied
        ▼
qefro serve / qefro dev   (Axum :8080)
        │
        ▼
PostgreSQL
```

The UI calls `GET /api/v1/meta/ui` after login. Navigation, field widgets, form layouts, and action buttons come from that payload. Branding is per tenant. There is no per-tenant frontend build.

Authorization always runs on the server. The browser never decides whether a create, PATCH, or workflow action is legal.

## Prerequisites

- Rust (stable) and Cargo
- Node.js 20+ (for the generic UI)
- PostgreSQL 16

Install the CLI:

```bash
cargo install qefro-cli
qefro --help
```

If that fails on macOS 26/27 with `mis-aligned LINKEDIT string pool` while compiling `sqlx`:

```bash
CARGO_PROFILE_RELEASE_STRIP=none cargo install qefro-cli
```

From a git checkout of this repository:

```bash
cargo install --path crates/qefro-cli --locked --force
# or: make install
```

Start Postgres. Docker Compose in this repo is enough:

```bash
docker compose up -d postgres
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
```

Or a local cluster:

```bash
export DATABASE_URL=postgres://USER@127.0.0.1:5432/qefro
```

Copy `.env.example` to `.env` if you want `JWT_SECRET` and bind address in a file. Never commit secrets.

## Two ways to start

| Path | Command | Best for |
| --- | --- | --- |
| YAML app in this repo | `qefro app new myshop` | CRUD, relations, child tables, formulas, workflows, permissions, YAML reports/dashboards |
| Standalone Rust binary | `qefro new my-app` | Same as YAML, plus **business operations** (`OperationHandler`) |

YAML apps loaded by the CLI register entities, workflows, permission grants, reports, dashboards, and print formats from directories. They cannot register `OperationHandler`s. Those live on `InstalledApp` in Rust.

Both paths share the same generic frontend. Do not scaffold a second React app. Package and distribute with `qefro app package` / `qefro app install *.qefro` — see [App packaging](app-packaging.md).

---

## Path 1 — YAML app (inside this repository)

Work from the framework root so the CLI writes `apps/<name>/`.

### 1. Create the app

```bash
qefro app new myshop
cd apps/myshop
```

Layout:

```
apps/myshop/
    app.toml
    entities/
    workflows/
    permissions/
    hooks/
    tools/
    seeds/
```

### 2. Define entities

Scaffold a file, then edit it:

```bash
qefro entity create Customer --app myshop
qefro entity create Product --app myshop
qefro entity create Order --app myshop
qefro entity create OrderItem --app myshop
```

`apps/myshop/entities/customer.yaml`:

```yaml
name: Customer
label: Customer
label_plural: Customers
fields:
  - name: name
    type: string
    required: true
    searchable: true
  - name: email
    type: string
    searchable: true
    validation:
      email: true
  - name: phone
    type: string
```

`apps/myshop/entities/product.yaml`:

```yaml
name: Product
label: Product
label_plural: Products
fields:
  - name: name
    type: string
    required: true
    searchable: true
  - name: sku
    type: string
    required: true
    unique: true
  - name: unit_price
    type: decimal
    required: true
    ui:
      widget: currency
```

`apps/myshop/entities/order.yaml`:

```yaml
name: Order
label: Order
label_plural: Orders
workflow: order
child_tables:
  - name: items
    child_entity: OrderItem
    parent_field: order_id
fields:
  - name: customer_id
    type: relation
    required: true
    relation:
      target_entity: Customer
      kind: many_to_one
  - name: order_date
    type: date
    required: true
    default_from: current_date
  - name: items
    type: child_table
  - name: subtotal
    type: decimal
    computed: true
    formula: SUM(items.amount)
    ui:
      widget: currency
  - name: discount
    type: decimal
    ui:
      widget: currency
  - name: grand_total
    type: decimal
    computed: true
    formula: subtotal - discount
    ui:
      widget: currency
  - name: status
    type: enum
    values: [Draft, Submitted, Cancelled]
document:
  submit_enabled: true
  cancel_enabled: true
  lock_states: [Submitted, Cancelled]
  number_on: submit
naming:
  pattern: "ORD-{YYYY}-{#####}"
  field: doc_no
  assign_on: submit
```

`apps/myshop/entities/order_item.yaml`:

```yaml
name: OrderItem
label: Line item
label_plural: Line items
standalone: false
child_of:
  parent_entity: Order
  parent_field: items
fields:
  - name: order_id
    type: relation
    required: true
    relation:
      target_entity: Order
      kind: many_to_one
    ui:
      hidden: true
  - name: product_id
    type: relation
    required: true
    relation:
      target_entity: Product
      kind: many_to_one
  - name: quantity
    type: integer
    required: true
    validation:
      min: 1
  - name: unit_price
    type: decimal
    required: true
    ui:
      widget: currency
  - name: amount
    type: decimal
    computed: true
    formula: quantity * unit_price
    ui:
      widget: currency
```

`child_of` keeps OrderItem out of the sidebar. Nested create/update still goes through the Order API as an `items` array. Computed fields are recalculated on the server; client-sent amounts are discarded.

### 3. Workflow

`apps/myshop/workflows/order.yaml`:

```yaml
name: order
entity: Order
field: status
initial: Draft
states:
  - name: Draft
  - name: Submitted
  - name: Cancelled
    terminal: true
transitions:
  - name: submit
    from: Draft
    to: Submitted
    label: Submit
    allowed_roles: [Admin, Manager, Staff]
  - name: cancel
    from: Draft
    to: Cancelled
    label: Cancel
    allowed_roles: [Admin, Manager]
```

Status cannot be PATCHed. The UI shows Submit / Cancel from `_actions` or `_workflow`. With `document.submit_enabled` / `cancel_enabled`, Qefro registers generic submit and cancel handlers if you did not write your own.

### 4. Permissions

The first user created via register is **Admin** and bypasses entity grants. Other roles need an explicit matrix.

`apps/myshop/permissions/staff.yaml`:

```yaml
- role: Staff
  entity: Customer
  actions: [create, read, update, delete, list]
- role: Staff
  entity: Product
  actions: [create, read, update, delete, list]
- role: Staff
  entity: Order
  actions: [create, read, update, delete, list]
- role: Staff
  entity: OrderItem
  actions: [create, read, update, delete, list]
```

A file may be one grant object or an array. Action names are `create`, `read`, `update`, `delete`, `list`, `export`.

### 5. Install, migrate, run

From the framework root:

```bash
qefro app install myshop
qefro migrate --app myshop
qefro entity list
qefro entity show Order
qefro routes
qefro workflows
qefro permissions
qefro doctor
```

Start the API (schema is applied again in development):

```bash
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro dev --app myshop
```

The process listens on `127.0.0.1:8080` by default (`QEFRO_BIND`).

### 6. Start the generic UI

In a second terminal, from the framework checkout:

```bash
cd frontend
npm install
npm run dev
```

Vite serves `http://127.0.0.1:5173` and proxies `/api` to `:8080`. Open that origin, register a tenant (name, email, password ≥ 8 characters, tenant name and slug), then use Customers, Products, and Orders.

Create an order with line items on the same form. `subtotal` and `grand_total` fill in after save. Submit locks the document; PATCH of business fields then fails until you use an allowed operation.

Inspect what the UI is rendering:

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"ada@example.com","password":"password123"}'
# then:
curl -s http://127.0.0.1:8080/api/v1/meta/ui -H "authorization: Bearer $TOKEN"
```

---

## Path 2 — Standalone Rust binary

Use this when you need named business operations (confirm + mutate related rows in one transaction), dashboards, or reports.

### Scaffold

`qefro new` writes a Cargo package. The generated `Cargo.toml` currently points at a sibling clone:

```toml
qefro-core = { path = "../qefro-framework/crates/qefro-core" }
qefro-api = { path = "../qefro-framework/crates/qefro-api" }
```

Create the app next to a checkout of this repository, or switch the deps to crates.io versions (`qefro-core = "0.6"`, `qefro-api = "0.6"`) once those crates are published.

```bash
# from the parent of qefro-framework/
qefro new myshop
cd myshop
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
cargo run
```

The stub already registers a `Customer` entity and calls `runtime.serve()`. Point the same `frontend/` at this process (still `:8080`).

### Register a module

Same metadata types as YAML. Restaurant and CRM in `examples/` are the full pattern.

```rust
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, ChildTableDef, EntityDef, FieldDef};
use qefro_permissions::PermissionGrant;
use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};

fn order() -> EntityDef {
    EntityDef::new("Order")
        .field(FieldDef::relation("customer_id", "Customer").required())
        .field(FieldDef::date("order_date").required())
        .child_table(ChildTableDef::new("items", "OrderItem").parent_field("order_id"))
        .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
        .field(FieldDef::currency("discount"))
        .field(FieldDef::currency("grand_total").computed("subtotal - discount"))
        .workflow("order")
        .build()
}

fn app() -> InstalledApp {
    let module = AppModule::new("myshop")
        .entity(/* customer, product, order, order_item */)
        .build();

    InstalledApp::new(module)
        .workflow(
            WorkflowDef::new("order", "Order", "Draft")
                .state(StateDef::new("Submitted"))
                .state(StateDef::new("Cancelled").terminal())
                .transition(TransitionDef::new("submit", "Draft", "Submitted")),
        )
        .permission(PermissionGrant::crud("Staff", "Order"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut runtime = QefroRuntime::new(Config::from_env()?);
    runtime.install(app());
    runtime.serve().await?;
    Ok(())
}
```

You can still load YAML entities from disk with `EntityDef::from_yaml` / `EntityRegistry::load_dir` and register the rest in Rust.

### Operations, dashboards, reports

CRUD is generated. Multi-record rules are not. Confirm a reservation and occupy a table in one transaction with an `OperationHandler` — see [operations.md](operations.md) and `examples/restaurant`.

```rust
app.operation(
    operation("confirm", "Reservation")
        .label("Confirm")
        .transition("confirm")
        .roles(&["Manager", "Staff"]),
    ConfirmReservation,
);
```

The generic detail page POSTs `/api/v1/{slug}/{id}/actions/confirm`. Agent tools and `qefro action` use the same `EntityService` path.

Dashboards and reports are metadata on `AppModule`, not React charts with raw SQL:

```rust
AppModule::new("myshop")
    .dashboard(/* DashboardDef */)
    .report(
        ReportDef::new("sales-by-day", "Order")
            .fields(&["order_date", "grand_total"])
            .group_by(&["order_date"])
            .sum("grand_total")
            .chart("bar"),
    )
```

See [dashboards.md](dashboards.md) and [reports.md](reports.md).

---

## What you should not build

- Per-entity React list/form/detail pages. The routes `/customers`, `/customers/new`, `/orders/:id` already exist in `frontend/`.
- A second API layer in Node or another Axum crate that talks to the same tables. Mutations go through `EntityService`.
- Client-side “authorization” (hiding a button is not security). The server re-checks RBAC, tenant, workflow, and validation.
- Arbitrary SQL in reports or dashboards. Filters reuse the same allowlisted query builder as list APIs.
- Custom widgets except by registering them in the frontend widget registry (`frontend/src/components/fields/widgets.tsx`). Data type and widget stay separate (`decimal` + `currency`).

When the generic form is not enough, extend metadata (sections, tabs, `visible_when`, child tables) or add a widget. Do not fork EntityForm for one entity.

---

## How a request is served

```
UI / curl / CLI / agent tool
        ↓
Authentication (JWT)
        ↓
Tenant from the session (not X-Tenant-ID)
        ↓
Application entitlements
        ↓
RBAC
        ↓
Validation + formulas
        ↓
Workflow (status is not a free PATCH)
        ↓
Handler + PostgreSQL transaction
        ↓
Audit + event after COMMIT
```

Every tenant-owned row has `tenant_id`. Repositories add `WHERE tenant_id = $1`. Clients cannot set it.

Generated REST for an entity named Order (`slug` = `orders`):

```
GET    /api/v1/orders
POST   /api/v1/orders
GET    /api/v1/orders/{id}
PATCH  /api/v1/orders/{id}
DELETE /api/v1/orders/{id}
POST   /api/v1/orders/{id}/actions/{name}
POST   /api/v1/orders/{id}/transition
```

Nested body on create/update:

```json
{
  "customer_id": "...",
  "order_date": "2026-08-15",
  "items": [
    { "product_id": "...", "quantity": 2, "unit_price": 199.00 }
  ]
}
```

OpenAPI: `GET /api/openapi.json` and `GET /docs`.

---

## Frontend in this repository

`frontend/` is the product UI for every Qefro app.

| Concern | Where it comes from |
| --- | --- |
| Entity list / form / detail | `GET /api/v1/meta/ui` |
| Widgets | registry keyed by `field.widget` |
| Actions | record `_actions` / `_workflow` |
| Branding, locale, nav | tenant config + `/meta/ui` |
| Dashboards | `GET /api/v1/meta/dashboards` |
| Reports | `GET /api/v1/reports` (when the app registered them) |

Development: `npm run dev` on 5173. Production compose serves the built UI on port 8081 in front of `qefro serve`. See [deployment.md](deployment.md).

Register from the login screen or:

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"name":"Ada","email":"ada@example.com","password":"password123","tenant_name":"Demo","tenant_slug":"demo"}'
```

That user is Admin for the new tenant.

---

## Learn from the examples

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```

Then open the UI: Restaurant → Branch → Table → Customer → Reservation. Confirm / Seat / Complete are operations, not status dropdowns. Order line items are a child table with formulas.

CRM (`--app crm`) shows lead convert as a Rust operation that creates a customer.

Source: `examples/restaurant`, `examples/basic-crm`. Catalog manifests: `apps/restaurant`, `apps/crm`.

---

## Production

Development embeds schema apply and the job worker in `qefro dev` / `qefro serve`. Production splits them:

```bash
export QEFRO_ENV=production
export QEFRO_AUTO_MIGRATE=false
export QEFRO_EMBED_WORKER=false
export JWT_SECRET=a-long-random-value
qefro migrate
qefro serve
# another process:
qefro worker
```

`docker compose up --build` runs Postgres, migrate, server, worker, and the generic frontend. Details: [configuration.md](configuration.md), [deployment.md](deployment.md), [security.md](security.md).

---

## Next reading

- [Architecture](architecture.md) — metadata as source of truth, security pipeline
- [Applications](apps.md) — catalog, install vs tenant enable
- [Entities](entities.md) — fields, relations, YAML shape
- [Child tables](child-tables.md) · [Formulas](formulas.md) · [Documents](documents.md)
- [Numbering](numbering.md) · [Print formats](print-formats.md) · [Reports](reports.md)
- [Workflows](workflows.md) · [Operations](operations.md)
- [UI](ui.md) · [Forms](forms.md) · [Widgets](ui-widgets.md) · [Dashboards](dashboards.md)
- [API](api.md) · [Examples](examples.md)
