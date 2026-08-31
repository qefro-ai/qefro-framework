# Getting started

Qefro is a Rust-native, metadata-driven framework for building secure, multi-tenant business applications. One entity definition produces PostgreSQL schema, REST, validation, a generic UI, workflows, reports, documents, automation, realtime, integrations, and agent tools.

```
Define once
     ↓
Database · API · UI · Workflow · Reports · Documents · Automation · Realtime · Integrations · Agent tools
```

## Install

```bash
docker compose up -d postgres
# or, if Docker is unavailable and a local Postgres is already on 5432:
# ./scripts/setup-postgres.sh
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
cargo install --path crates/qefro-cli --locked --force
qefro --help
qefro doctor
```

## Create an application

Step-by-step (scaffold, entities, permissions, views, run the generic UI): **[Create an application](creating-an-app.md)**. Every feature: **[App Developer Guide](developer-guide.md)**.

```bash
qefro app new myshop
# edit apps/myshop/entities, workflows, permissions, reports
qefro app validate myshop
qefro app package myshop
qefro app install myshop
qefro migrate --app myshop
qefro dev --app myshop
```

You do not need to read `EntityService` to ship CRUD, workflows, permissions, reports, print, public forms, webhooks, or realtime.

## Restaurant tutorial

The complete walkthrough (entities through production deploy) is [Build a Restaurant app](fullstack.md) plus the restaurant example in the README.

```bash
qefro app install restaurant
qefro migrate --app restaurant
qefro dev --app restaurant
cd frontend && npm install && npm run dev
```

## Documentation map

The full catalog (every feature with a one-line description) is the [documentation index](index.md). How to use each feature while building an app: [App Developer Guide](developer-guide.md).

| Topic | Doc |
| --- | --- |
| App developer handbook | [developer-guide.md](developer-guide.md) |
| Feature catalog | [index.md](index.md) |
| Architecture | [architecture.md](architecture.md) |
| Compatibility | [v1-compatibility.md](v1-compatibility.md) |
| Create an app | [creating-an-app.md](creating-an-app.md) |
| YAML vs Rust | [app-development.md](app-development.md) |
| Entities | [entities.md](entities.md) |
| Fields / relations | [entities.md](entities.md), [child-tables.md](child-tables.md) |
| Documents | [documents.md](documents.md) |
| Workflows | [workflows.md](workflows.md) |
| Permissions | [permissions.md](permissions.md) |
| Identity | [identity.md](identity.md) |
| Tasks | [tasks.md](tasks.md) |
| Business object runtime | [business-object-runtime.md](business-object-runtime.md) |
| UI | [ui.md](ui.md), [ui-2.md](ui-2.md), [views.md](views.md) |
| Studio | [studio.md](studio.md) |
| Reports / dashboards | [reports.md](reports.md), [dashboards.md](dashboards.md) |
| Events / jobs | [events.md](events.md), [jobs.md](jobs.md) |
| Webhooks / realtime | [webhooks.md](webhooks.md), [realtime.md](realtime.md) |
| Public forms | [public-forms.md](public-forms.md) |
| Deployment | [deployment.md](deployment.md) |
| Security | [security.md](security.md), [threat-model.md](threat-model.md) |
| API | [api.md](api.md) |
| SDK / connectors | [connectors.md](connectors.md) |
