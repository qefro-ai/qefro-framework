# Applications

Qefro applications are first-class modules. Framework core does not hardcode restaurant or CRM behavior.

## Layout

```
apps/
    restaurant/
        app.toml
    crm/
        app.toml
    myshop/
        app.toml
        entities/
        workflows/
        permissions/
        hooks/
        tools/
        seeds/
```

Built-in examples are implemented in Rust (`examples/restaurant`, `examples/basic-crm`) and advertised through catalog manifests in `apps/`. New apps can be YAML-only.

## Manifest

```toml
name = "restaurant"
version = "0.2.0"
description = "Restaurant management application"
depends_on = ["qefro-framework"]
```

Dependencies are recorded, not resolved as a marketplace. Keep them simple.

## What an app contributes

- entities
- workflows
- permissions
- **business operations** (Rust handlers) and optional jobs
- hooks
- tools (generated from entities and operations)
- seed data (directory reserved)
- UI configuration / dashboards

## CLI

```bash
qefro app new myshop
qefro app list
qefro app install myshop
qefro app remove myshop
qefro app info myshop
```

`qefro app install` writes `.qefro/installed.json`. That marks an application as **installed globally**. Each tenant then **enables** a subset via `/api/v1/tenant/apps`. Enabling is constrained by `Entitlements` (plan). UI, REST, and agent tools all honor the resolved list. Frontend visibility is not security.

`qefro dev` with `--app all` loads the installed set. If the file is empty, restaurant and CRM load as the demo default.

## Registering from Rust

```rust
AppModule::new("restaurant")
    .entity(customer())
    .dashboard(ops())
    .build()
```

`QefroRuntime::install(InstalledApp::new(module).workflow(...).permission(...))` is the only registration path the HTTP and agent layers see.
