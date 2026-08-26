# Applications

Qefro applications are first-class, versioned packages. Framework core does not hardcode restaurant or CRM behavior.

To create an app from scratch and run it with the generic UI, see [Create an application](creating-an-app.md). Longer shop tutorial: [Build a fullstack application](fullstack.md). Packaging and install: [App packaging](app-packaging.md). Lifecycle: [App lifecycle](app-lifecycle.md). YAML vs Rust: [App development](app-development.md).

## Layout

```
apps/myshop/
├── app.toml
├── entities/
├── workflows/
├── permissions/
├── reports/
├── dashboards/
├── print_formats/
├── seeds/
├── hooks/
├── migrations/
├── assets/
└── README.md
```

Built-in examples are implemented in Rust (`examples/restaurant`, `examples/basic-crm`) and advertised through catalog manifests in `apps/`. YAML apps can be packaged as `.qefro` files and installed onto any Qefro 1.x runtime (`framework_version = ">=1.0,<2.0"`). Benchmark apps: `apps/inventory`, `apps/helpdesk`.

## Manifest

```toml
name = "restaurant"
version = "1.0.0"
label = "Restaurant Management"
description = "Restaurant operations and ordering"
author = "Qefro"
license = "MIT"
api_version = "1"
framework_version = ">=1.0,<2.0"

[dependencies]
inventory = ">=1.0,<2.0"

[[navigation]]
label = "Reservations"
entity = "Reservation"
```

`app.toml` describes the package. Entity, workflow, permission, report, dashboard, print, and seed definitions stay in their directories (or in Rust for builtin apps). Do not duplicate those definitions in the manifest.

The legacy `depends_on = ["qefro-framework"]` list is still accepted and treated as a framework compatibility marker.

## States

| State | Meaning |
| --- | --- |
| **catalogued** / **available** | Discovered under `apps/` or `.qefro/store/`, not installed |
| **installed** | Registered globally (`.qefro/installed.json` and `qefro_apps` when Postgres is up). Tenants *may* enable it. |
| **disabled** | Still installed. Definitions and data kept. Tenants cannot enable it. |
| **enabled** | A *tenant* has the app in `enabled_apps` (or empty list meaning all installed apps the plan allows) |
| **uninstalled** | Removed from the global registry. Business tables are kept |

Installed globally ≠ enabled for every tenant. Entitlements still apply: `installed ∩ tenant.enabled_apps ∩ plan.apps`. The client cannot enable an app by editing a request.

## What an app contributes

- entities, child tables, formulas, documents
- workflows
- permissions
- reports, dashboards, print formats (YAML or Rust)
- seeds (tenant-aware, idempotent)
- default navigation
- **business operations** (Rust `OperationHandler` only)
- hooks (entity hooks in Rust; lifecycle hooks in YAML are declarative — no shell)

## CLI

```bash
qefro app new myshop
qefro app validate myshop
qefro app package myshop
qefro app install myshop
qefro app install myshop-0.1.0.qefro
qefro app update myshop
qefro app info myshop
qefro app list
qefro app disable myshop
qefro app enable myshop
qefro app uninstall myshop
qefro app seed myshop --tenant demo
qefro tenant app enable demo myshop
qefro tenant app disable demo myshop
```

`qefro app remove` is an alias of `uninstall`.

## Registering from Rust

```rust
AppModule::new("restaurant")
    .version("1.0.0")
    .entity(customer())
    .dashboard(ops())
    .build()
```

`QefroRuntime::install(InstalledApp::new(module).workflow(...).permission(...))` is the only registration path the HTTP and agent layers see.
