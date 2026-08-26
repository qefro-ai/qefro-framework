# App development

Two ways to ship a Qefro app. Both use the same generic UI (`frontend/`). Do not create a React app per Qefro application or entity.

Walkthrough from `qefro app new` through running the UI: [Create an application](creating-an-app.md). Full shop example: [Build a fullstack application](fullstack.md).

## YAML application

Best for CRUD, relations, child tables, formulas, workflows, permissions, numbering, print formats, basic reports, and basic dashboards.

```bash
qefro app new myshop
# edit apps/myshop/entities, workflows, permissions, reports, dashboards
qefro app validate myshop
qefro app package myshop
qefro app install myshop-0.1.0.qefro
qefro migrate --app myshop
qefro tenant app enable demo myshop
qefro dev --app myshop
```

The generated skeleton includes a `Customer` entity and a Staff grant so `validate` succeeds immediately.

## Rust application

Required when you need `OperationHandler`, multi-record transactions, jobs, or behavior that is not expressible in metadata.

```bash
qefro new my-app
```

Register `AppModule`, `InstalledApp`, workflows, permissions, operations, dashboards, and reports in Rust. You can still load extra YAML entities with `EntityDef::from_yaml`.

Builtin restaurant and CRM follow this path. Packaging their `app.toml` records version and compatibility; handlers stay in the compiled crate.

## Shared runtime

HTTP, the generic UI, the CLI, and agent tools all go through `EntityService`. Tenant isolation, RBAC, and entitlements are server-side. Custom widgets belong in the frontend widget registry, not in a forked EntityForm.
