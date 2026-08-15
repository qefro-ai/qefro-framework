# Qefro Framework

Rust-native, metadata-driven framework for multi-tenant business applications.

Define entities, workflows, permissions, and **business operations**. The runtime generates PostgreSQL schema, REST APIs, validation, audit logs, a generic UI, agent tools, events, and a Postgres job queue. Authorization always runs on the server. Agents never get a database connection.

## Quick start

```bash
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
cargo run -p qefro-cli -- dev --app restaurant
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

The UI reads `/api/v1/meta/ui`. There is no per-entity React page to write.

## Create an application

```bash
qefro app new myshop
cd apps/myshop
qefro entity create Customer
qefro entity create Reservation
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

1. Open the UI and sign in (or register a tenant).
2. Create a Restaurant, Branch, Table, and Customer.
3. Create a Reservation. The customer and table fields are relation pickers, not raw UUIDs.
4. Use **Confirm → Seat Customer → Complete** (each button is a business operation: table occupancy is updated atomically with reservation status).
5. The dashboard cards (today's reservations, table occupancy, orders, sales) come from application metadata.
6. Discover tools: `GET /api/v1/tools` (already permission-filtered), including `confirm_reservation`.
7. Invoke `confirm_reservation` through `POST /api/v1/agent/tools/confirm_reservation/invoke`. Same `EntityService` path as REST.

## CLI

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
```

`qefro dev --app restaurant` loads the restaurant example. `--app crm` loads CRM. Default is both, or whatever is listed in `.qefro/installed.json`.

## Workspace

```
apps/restaurant            catalog manifest (runtime: examples/restaurant)
apps/crm                   catalog manifest (runtime: examples/basic-crm)
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
EntityDef::new("Customer")
    .field(FieldDef::string("name").required().searchable())
    .field(FieldDef::string("email").required().email().unique())
    .field(FieldDef::string("phone").nullable())
    .build()
```

That definition produces `/customers`, `/customers/new`, `/customers/:id`, `/customers/:id/edit`, REST, validation, audit, and agent tools.

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
cargo test --workspace
DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro cargo test --workspace -- --test-threads=1
```

Integration tests that need PostgreSQL skip when `DATABASE_URL` is unset.

## Docs

- [Architecture](docs/architecture.md)
- [Applications](docs/apps.md)
- [Entities](docs/entities.md)
- [Operations](docs/operations.md)
- [Workflows](docs/workflows.md)
- [Events](docs/events.md)
- [Jobs](docs/jobs.md)
- [UI](docs/ui.md)
- [Agents](docs/agents.md)
- [Multi-tenancy](docs/multitenancy.md)
- [API](docs/api.md)
- [Examples](docs/examples.md)
