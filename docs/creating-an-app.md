# Create an application with Qefro

Define the business once. Qefro generates PostgreSQL schema, REST APIs, validation, a generic UI (list, cards, kanban, calendar, form, detail), workflows, permissions, and agent tools. You do **not** write a React page or REST controller per entity.

```
Entity YAML / EntityDef
        │
        ├─ schema, REST, validation
        ├─ generic UI (List · Cards · Kanban · Form · Detail)
        ├─ workflows, permissions, reports
        └─ agent tools
                │
                ▼
         EntityService
```

This guide creates a YAML app. For a longer customers/products/orders walkthrough see [Build a fullstack application](fullstack.md). For packaging and tenant enablement see [Applications](apps.md).

## Prerequisites

- Rust (stable) and Cargo
- Node.js 20+ (generic UI)
- PostgreSQL 16

Install the CLI from crates.io:

```bash
cargo install qefro-cli
qefro --help
qefro doctor
```

On macOS 26/27, if install fails with `mis-aligned LINKEDIT string pool` while compiling `sqlx`:

```bash
CARGO_PROFILE_RELEASE_STRIP=none cargo install qefro-cli
```

From a git checkout of this repository instead:

```bash
cargo install --path crates/qefro-cli --locked --force
# or: make install
```

Start Postgres and set the database URL. Preferred: the compose service in this repo.

```bash
docker compose up -d postgres
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro doctor
```

If Docker is not installed, or port 5432 already belongs to a local Postgres that has no `qefro` role:

```bash
./scripts/setup-postgres.sh
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro doctor
```

That script creates role `qefro` / database `qefro` (password `qefro`) on `127.0.0.1:5432` and grants DDL on `public` (PostgreSQL 15+). The compose `POSTGRES_USER` is a superuser; the local fallback matches that so `qefro migrate` can apply schema. There is no `qefro app migrate` or `qefro app run` — schema is `qefro migrate`, the server is `qefro dev`.

## Two ways to start

| Path | Command | Use when |
| --- | --- | --- |
| YAML app | `qefro app new myshop` | CRUD, relations, child tables, formulas, workflows, permissions, YAML reports/dashboards/pages |
| Rust app | `qefro new my-app` | Same as YAML, plus **business operations** (`OperationHandler`) |

YAML apps cannot register `OperationHandler`s. Those live on `InstalledApp` in Rust (restaurant and CRM follow that path). Both paths share the same generic frontend. Do not scaffold a second React app.

Work from the **framework repository root** so `qefro app new` writes `apps/<name>/` and you can run `frontend/`. Outside the repo the CLI writes `./<name>/`.

## 1. Scaffold the app

```bash
qefro app new myshop
```

That creates:

```
apps/myshop/
├── app.toml
├── entities/customer.yaml
├── permissions/staff.yaml
├── workflows/
├── reports/
├── dashboards/
├── pages/
├── print_formats/
├── seeds/
└── README.md
```

The skeleton includes a `Customer` entity and a Staff grant so `validate` succeeds immediately.

`app.toml` is the package manifest (name, version, navigation). Entity, workflow, and permission definitions stay in their directories. Do not duplicate them in the manifest.

```toml
name = "myshop"
version = "0.1.0"
label = "Myshop"
api_version = "1"
framework_version = ">=1.0,<2.0"

[[navigation]]
label = "Customers"
entity = "Customer"
```

## 2. Define entities

Scaffold more files:

```bash
qefro entity create Company --app myshop
qefro entity create Contact --app myshop
```

Edit YAML. One definition drives schema, API, and UI.

`apps/myshop/entities/company.yaml`:

```yaml
name: Company
label: Company
label_plural: Companies
display_field: name
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

`apps/myshop/entities/contact.yaml`:

```yaml
name: Contact
label: Contact
label_plural: Contacts
display_field: name
fields:
  - name: name
    type: string
    required: true
    searchable: true
  - name: email
    type: string
    required: true
    searchable: true
    validation:
      email: true
    ui:
      widget: email
  - name: company_id
    type: relation
    required: true
    relation:
      target_entity: Company
      kind: many_to_one
    ui:
      widget: relation
      section: Details
  - name: notes
    type: text
    ui:
      widget: textarea
      section: Notes
```

Add a one-to-many on Company so the detail page lists related contacts:

```yaml
# on Company
  - name: contacts
    type: relation
    relation:
      target_entity: Contact
      kind: one_to_many
      inverse_field: company_id
```

Field types: `string`, `text`, `integer`, `decimal`, `boolean`, `date`, `time`, `datetime`, `uuid`, `enum`, `json`, `relation`, `child_table`. Presentation is `ui.widget`, not a second type system. See [Entities](entities.md) and [Child tables](child-tables.md).

## 3. Permissions

`apps/myshop/permissions/staff.yaml`:

```yaml
- role: Staff
  entity: Customer
  actions: [create, read, update, delete, list]
- role: Staff
  entity: Company
  actions: [create, read, update, delete, list]
- role: Staff
  entity: Contact
  actions: [create, read, update, delete, list]
```

Admin bypasses role lists after login. The generic UI may hide New/Edit/Delete from `permissions` on `/api/v1/meta/ui`; the server still authorizes every write. See [Permissions](permissions.md).

## 4. Optional: workflow

Status fields bound to a workflow cannot be PATCHed. Callers use a named transition.

`apps/myshop/workflows/company.yaml`:

```yaml
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

On the Company entity set `workflow: company`. Kanban drag uses `POST .../transition`, not `PATCH status`. See [Workflows](workflows.md).

## 5. Views (generic UI)

You do not create pages. The generic UI reads `GET /api/v1/meta/ui`.

| View | When it appears |
| --- | --- |
| List | Always |
| Cards | Only if `views.card` is set |
| Kanban | `views.kanban`, or a workflow + status/enum grouping field |
| Calendar | `views.calendar`, or a non-system date/datetime field |
| Form / Detail | Routes `/:slug/new`, `/:slug/:id`, `/:slug/:id/edit` |

Do not force Cards or Kanban on entities that omit that metadata. List columns, card title/fields, and kanban `group_by` are presentation-only. See [Views](views.md) and [View metadata](view-metadata.md).

Studio can overlay labels, order, sections, and view config without changing the business model. It is not a page builder. See [Studio](studio.md).

## 6. Validate, install, migrate

```bash
qefro app validate myshop
qefro app package myshop
qefro app install myshop
qefro migrate --app myshop
```

`validate` checks the bundle (unknown relations, bad versions). `package` writes a `.qefro` archive. `install` registers the app globally. `migrate` applies PostgreSQL schema for that app’s entities.

Inspect what was generated:

```bash
qefro entity list --app myshop
qefro entity show Company --app myshop
qefro routes --app myshop
qefro permissions --app myshop
qefro workflows --app myshop
```

## 7. Run the API and UI

```bash
qefro dev --app myshop
```

API listens on `http://127.0.0.1:8080`. Register a tenant:

```bash
curl -s http://127.0.0.1:8080/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"name":"Ada","email":"ada@example.com","password":"password123","tenant_name":"Demo","tenant_slug":"demo"}'
```

Enable the app for that tenant if you installed after the tenant already existed:

```bash
qefro tenant app enable demo myshop
```

Start the generic UI from this repository (one frontend for every app):

```bash
cd frontend && npm install && npm run dev
```

Open the UI, sign in, and use **Companies** / **Contacts**. Relation fields are searchable pickers, not raw UUIDs. Related contacts appear on the company detail page; **Add Contact** prefills `company_id`.

The UI talks to the backend through `QefroClient` (`frontend/src/sdk/client.ts`) → `/api/v1` → `EntityService`. Agents use `EntityOps` on the same path. See [QefroClient](sdk.md).

## What you get without writing UI code

For each entity:

- `GET/POST /api/v1/{slug}`, `GET/PATCH/DELETE /api/v1/{slug}/{id}`
- List with search, filters, sort, pagination
- Optional Cards / Kanban / Calendar from `views:`
- Create and edit forms from field widgets
- Detail with related records, child tables, actions
- `_expanded`, `_related`, `_links`, `_workflow`, `_actions`, `_permissions`

Do not branch on entity names in React. Use metadata (`EntityDef`, widgets, views, permissions).

## When you need Rust

Use `qefro new my-app` (or an `AppModule` in this repo) when a transition must update related records in one transaction — for example seating a reservation and occupying a table. That is an `OperationHandler`, not a YAML workflow alone. See [Operations](operations.md) and [App development](app-development.md).

```bash
qefro new my-app
cd my-app
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
cargo run
```

You can still load extra YAML with `EntityDef::from_yaml`. Do not fork `EntityForm`.

## Package and share

```bash
qefro app package myshop
qefro app install myshop-0.1.0.qefro
```

`.qefro` packages run on any Qefro 1.x runtime (`framework_version = ">=1.0,<2.0"`). See [App packaging](app-packaging.md) and [App lifecycle](app-lifecycle.md).

## Next

| Topic | Doc |
| --- | --- |
| Full shop tutorial | [fullstack.md](fullstack.md) |
| Field types and relations | [entities.md](entities.md) |
| Child tables | [child-tables.md](child-tables.md) |
| Workflows | [workflows.md](workflows.md) |
| Permissions | [permissions.md](permissions.md) |
| Generic UI | [ui.md](ui.md), [views.md](views.md) |
| Studio overlays | [studio.md](studio.md) |
| Reports / dashboards / pages | [reports.md](reports.md), [dashboards.md](dashboards.md), [pages.md](pages.md) |
| Agents | [agents.md](agents.md) |
| Deploy | [deployment.md](deployment.md) |
