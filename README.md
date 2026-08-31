# Qefro Framework

Rust-native, metadata-driven framework for building secure, multi-tenant business applications with a shared database, API, UI, workflow, automation, and agent runtime.

Define entities, workflows, permissions, and **business operations**. The runtime generates PostgreSQL schema, REST APIs, validation, audit logs, a generic UI, agent tools, events, and a Postgres job queue. Authorization always runs on the server. Agents never get a database connection.

**V1.3** adds search, reports, dashboards, saved views, declarative validation, computed strings, and `AutomationDef` on the same EntityService path. App developers: [App Developer Guide](docs/developer-guide.md) (every feature) and the [documentation index](docs/README.md). Also [Getting started](docs/getting-started.md), [Business object runtime](docs/business-object-runtime.md), [Automation](docs/automation.md), [Validation](docs/validation.md), [Identity](docs/identity.md), and [V1 compatibility](docs/v1-compatibility.md).

## Install

Install the `qefro` binary onto your PATH (`~/.cargo/bin`):

```bash
cargo install qefro-cli
qefro --help
```

On macOS 26/27, if install fails with `mis-aligned LINKEDIT string pool` while compiling `sqlx`, skip stripping proc-macro dylibs:

```bash
CARGO_PROFILE_RELEASE_STRIP=none cargo install qefro-cli
```

From this repo without crates.io:

```bash
cargo install --path crates/qefro-cli --locked --force
# or: make install
```

From a git checkout you can also run without installing:

```bash
cargo qefro --help
```

## Quick start

```bash
docker compose up -d postgres
# or: ./scripts/setup-postgres.sh
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro migrate --app restaurant
qefro dev --app restaurant
```

Register a tenant:

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"name":"Ada","email":"ada@example.com","password":"password123","tenant_name":"Demo","tenant_slug":"demo"}'
```

Open the generic UI:

```bash
cd frontend && npm install && npm run dev
```

The UI reads `/api/v1/meta/ui`. Branding, navigation, terminology, widgets, form layouts, filters, and dashboards come from the authenticated tenant. There is no per-entity React page and no per-tenant frontend build. Define the entity once; Qefro generates schema, REST, validation, and the business UI.

Authorized developers open **Qefro Studio** (`/studio`) to inspect and publish metadata through the same registries. See [Qefro Studio](docs/studio.md).

V1.0 hardens the V0.9 platform (settings, field permissions, attachments, notifications, webhooks, CSV import, global search, realtime, public forms) on the same `EntityService` path. The generic UI is **UI 2.1**: a professional metadata-driven shell with List, Kanban, and Calendar views on that API. See [UI 2.0 / 2.1](docs/ui-2.md), [Views](docs/views.md), and [Architecture](docs/architecture.md).

## Build a fullstack app

One Axum process, PostgreSQL, and the generic UI in `frontend/`. Define entities (YAML or Rust); Qefro generates schema, REST, and screens. You do not write a React page per entity.

Step-by-step (customers, products, orders with line items): **[Build a fullstack application](docs/fullstack.md)**.

## Create an application

Step-by-step: **[Create an application](docs/creating-an-app.md)**. Shop tutorial: [Build a fullstack application](docs/fullstack.md).

```bash
qefro app new myshop
cd apps/myshop
qefro app validate myshop
qefro app package myshop
qefro app install myshop
qefro migrate --app myshop
qefro dev --app myshop
```

Inspect what the framework generated:

```bash
qefro entity list
qefro entity show Customer
qefro routes
qefro permissions
qefro workflows
qefro operations
qefro tools
qefro doctor
```

## Restaurant walkthrough

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
```

1. Open the UI and sign in (or register a tenant). Ops nav is Reservations, Tables, Orders, Customers, plus Settings.
2. Create a Restaurant, Branch, and Table (setup lives under **Settings**). People and Users are also under Settings, not the floor menu.
3. Create a Customer. Walk-in: fill name, email, and phone and leave **Person** empty — no User row. Linked guest: create a Person in Settings → People, then set Customer → Person. Customer still stores its own name/email/phone for unlinked and legacy rows.
4. Create a Reservation. The customer and table fields are relation pickers, not raw UUIDs.
5. Use **Confirm → Seat Customer → Complete** (each button is a business operation: table occupancy is updated atomically with reservation status).
6. The dashboard cards (today's reservations, table occupancy, orders, sales) come from application metadata.
7. Discover tools: `GET /api/v1/tools` (already permission-filtered), including `confirm_reservation`.
8. Invoke `confirm_reservation` through `POST /api/v1/agent/tools/confirm_reservation/invoke`. Same `EntityService` path as REST.

## CLI

After `cargo install --path crates/qefro-cli --locked --force` (or `make install`), these commands run from any directory:

```bash
qefro new my-app
qefro app new restaurant
qefro app list
qefro app install restaurant
qefro app info restaurant
qefro app remove restaurant
qefro entity list
qefro entity show Customer
qefro entity create Customer
qefro migrate
qefro dev
qefro routes
qefro permissions
qefro workflows
qefro operations
qefro operations Reservation
qefro action Reservation <id> confirm
qefro tools
qefro doctor
qefro serve
qefro worker
```

`qefro dev --app restaurant` loads the restaurant example. `--app crm` loads CRM. Default is both, or whatever is listed in `.qefro/installed.json`. Production: `qefro migrate` then `qefro serve` and `qefro worker` with `QEFRO_ENV=production`.

## Workspace

```
apps/restaurant            catalog manifest (runtime: examples/restaurant)
apps/crm                   catalog manifest (runtime: examples/basic-crm)
apps/inventory             V1.0 YAML benchmark (stock documents)
apps/helpdesk              V1.0 YAML benchmark (tickets, public form)
crates/qefro-core          metadata, validation, hooks, app catalog
crates/qefro-db            PostgreSQL, SQL generation, entity service
crates/qefro-auth          users, passwords, JWT sessions
crates/qefro-tenant        tenant records, branding/config
crates/qefro-permissions   RBAC
crates/qefro-workflow      state machines
crates/qefro-events        in-process domain events
crates/qefro-search        safe query parsing
crates/qefro-agent         tool registry (no DB access)
crates/qefro-api           Axum runtime
crates/qefro-cli           qefro binary
examples/restaurant
examples/basic-crm
frontend                   generic metadata UI
```

## Defining an entity

```rust
EntityDef::new("Reservation")
    .field(FieldDef::relation("customer", "Customer").required())
    .field(FieldDef::date("reservation_date").required().ui(UiConfig::date()))
    .field(FieldDef::time("reservation_time").required())
    .field(FieldDef::integer("guests").required())
    .field(FieldDef::enum_("status", vec!["Pending", "Confirmed", "Cancelled"]))
    .field(FieldDef::datetime("created_at").ui(UiConfig::datetime().tenant_timezone()))
    .build();
```

A business document adds child tables, formulas, numbering, and workflow on the same `EntityDef`:

```rust
EntityDef::new("Order")
    .field(FieldDef::relation("customer_id", "Customer").required())
    .field(FieldDef::date("order_date").required())
    .child_table(ChildTableDef::new("items", "OrderItem").parent_field("order_id"))
    .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
    .field(FieldDef::currency("discount"))
    .field(FieldDef::currency("grand_total").computed("subtotal - discount"))
    .workflow("order")
    .build();
```

That definition produces PostgreSQL, REST, validation, nested child tables, computed fields, generic list/form/detail UI, print, reports, and workflow actions — without a custom React page.

YAML is supported via `EntityDef::from_yaml` / `EntityRegistry::load_dir`.

## Agent tools

Every entity exposes tools such as `create_reservation` and `find_reservations`. Invocation path:

```
Agent → Tool Registry → Authentication → Tenant Context
      → Permission Check → Validation → Workflow → Business Operation → Audit/Event
```

`GET /api/v1/tools` returns only tools the current user may invoke. Invoke still re-checks permissions.

## Tests

```bash
./scripts/setup-postgres.sh
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
cargo test --workspace -- --test-threads=1
cd frontend && npm test
```

Or `make check`. Integration tests require `DATABASE_URL` (they fail closed if it is unset). See [Benchmarks](docs/benchmarks.md).

## Docs

**Start here:** [Documentation index](docs/README.md) · [App Developer Guide](docs/developer-guide.md) (every feature, with YAML/Rust examples)

| Path | Doc |
| --- | --- |
| Install and first run | [Getting started](docs/getting-started.md) |
| Scaffold a YAML app | [Create an application](docs/creating-an-app.md) |
| Shop tutorial | [Build a fullstack application](docs/fullstack.md) |
| YAML vs Rust | [App development](docs/app-development.md) |
| Architecture | [Architecture](docs/architecture.md) |
| Feature catalog | [docs/README.md](docs/README.md) |
| Examples | [Examples](docs/examples.md) |
