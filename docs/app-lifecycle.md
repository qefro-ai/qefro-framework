# App lifecycle

```
Build → Validate → Package → Install → Migrate → Enable for tenant → Configure → Update → Disable / Uninstall
```

## Install

`qefro app install restaurant` or `qefro app install restaurant-1.0.0.qefro`:

1. Inspect / load definitions
2. Validate manifest, entities, relations, workflows, reports, dashboards, print formats, permissions, framework version, dependencies
3. Extract a package into `.qefro/store/<name>/` (skipped for catalog names)
4. Record `.qefro/installed.json`
5. Upsert `qefro_apps` and `qefro_app_versions` when `DATABASE_URL` is set
6. Emit `app.installed`

Schema is applied by `qefro migrate` / `qefro dev` (additive `CREATE TABLE` / `ADD COLUMN`). Installation does not drop columns.

## Update

`qefro app update myshop` reloads catalog metadata. `qefro app update myshop-1.1.0.qefro` replaces the store copy.

- New fields are added by schema apply.
- Fields removed from metadata are **reported** and left in PostgreSQL. Pass `--yes` to continue when the operator has reviewed the list.
- Explicit files in `migrations/` are recorded in `qefro_app_migrations`. SQL containing `DROP` / `TRUNCATE` / `DELETE FROM` is skipped unless `--yes`.
- Version history is appended. Full rollback is not implemented in V0.7.

Never silently delete business data.

## Disable vs uninstall vs remove

| Command | Definitions | Tenant use | Data |
| --- | --- | --- | --- |
| `qefro app disable` | kept | tenants cannot enable | kept |
| `qefro tenant app disable` | kept | that tenant only | kept |
| `qefro app uninstall` / `remove` | unregistered | no | **kept** |

There is no automatic destructive uninstall in V0.7.

## Tenant enablement

```
Installed globally → tenant enabled → tenant configured → tenant disabled
```

```bash
qefro tenant app enable demo restaurant
qefro tenant app disable demo restaurant
```

Empty `enabled_apps` still means every globally installed app the plan allows. Disable materializes the list minus that app. Branding, terminology, navigation, and feature flags stay tenant configuration — they do not fork the app package.

Installing or updating Restaurant does not change another tenant's CRM data, branding, or enabled apps.

## Seeds

```bash
qefro app seed restaurant --tenant demo
qefro app seed restaurant --tenant demo --kind development
```

| Kind | When |
| --- | --- |
| `system` | skipped for tenant-owned entities |
| `install` | intended for first tenant enable; safe to rerun |
| `tenant` | explicit seed command |
| `development` | only if `QEFRO_ENV=development` |

Rows matching `unique_by` are left alone. Seeds never overwrite user-modified data.

## Lifecycle hooks

YAML under `hooks/`:

```yaml
on: tenant_enable
seed_kinds: [install]
```

Allowed events: `install`, `upgrade`, `uninstall`, `tenant_enable`, `tenant_disable`. Apps cannot run shell. Matching events are also written to `qefro_app_events` (`app.installed`, `app.updated`, `app.enabled`, `app.disabled`, `app.uninstalled`).

## Registry

PostgreSQL `qefro_apps` is the source of truth when the database is available. `.qefro/installed.json` remains the local index for `qefro dev` without requiring every command to open Postgres. `qefro doctor` checks manifests, dependencies, and pending migrations.
