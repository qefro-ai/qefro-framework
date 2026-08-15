use anyhow::{bail, Context, Result};
use qefro_api::InstalledApp;
use qefro_core::{
    disable_app, discover_apps, enable_app, extract_package, find_app_root, inspect_package,
    load_installed, mark_installed, parse_app_toml, remove_app, store_dir, validate_bundle,
    write_package, AppBundle, AppFileManifest, FRAMEWORK_VERSION,
};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{builtin_manifests, known_app_names, runtime_for};

pub fn cmd_app_new(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid app name '{name}' (use letters, numbers, '-' or '_')");
    }
    if matches!(name, "restaurant" | "crm") && crate::is_framework_root() {
        let catalog = PathBuf::from("apps").join(name);
        if !catalog.exists() {
            crate::write_catalog_stub(&catalog, name)?;
        }
        println!(
            "'{name}' is a built-in application. Catalog: {}\nInstall it with: qefro app install {name}\nRun it with:     qefro dev --app {name}",
            catalog.display()
        );
        return Ok(());
    }
    let root = if crate::is_framework_root() {
        PathBuf::from("apps").join(name)
    } else {
        PathBuf::from(name)
    };
    if root.exists() {
        bail!("app '{name}' already exists at {}", root.display());
    }
    write_app_skeleton(&root, name)?;
    println!("created {}", root.display());
    println!("next:");
    println!("  qefro app validate {name}");
    println!("  qefro app package {name}");
    println!("  qefro app install {name}");
    println!("  qefro migrate --app {name}");
    println!("  qefro dev --app {name}");
    Ok(())
}

fn write_app_skeleton(root: &Path, name: &str) -> Result<()> {
    for dir in [
        "entities",
        "workflows",
        "permissions",
        "reports",
        "dashboards",
        "print_formats",
        "seeds",
        "hooks",
        "migrations",
        "assets",
        "tools",
    ] {
        fs::create_dir_all(root.join(dir))?;
    }
    let label = humanize(name);
    fs::write(
        root.join("app.toml"),
        format!(
            r#"name = "{name}"
version = "0.1.0"
label = "{label}"
description = ""
author = ""
license = "MIT"
api_version = "1"
framework_version = ">=0.7"

[dependencies]

[[navigation]]
label = "Customers"
entity = "Customer"
"#
        ),
    )?;
    fs::write(
        root.join("entities/customer.yaml"),
        "name: Customer\nlabel: Customer\nlabel_plural: Customers\nfields:\n  - name: name\n    type: string\n    required: true\n    searchable: true\n",
    )?;
    fs::write(
        root.join("permissions/staff.yaml"),
        "- role: Staff\n  entity: Customer\n  actions: [create, read, update, delete, list]\n",
    )?;
    fs::write(
        root.join("README.md"),
        generated_readme(name, "0.1.0", &label),
    )?;
    Ok(())
}

fn humanize(name: &str) -> String {
    let mut out = String::new();
    for (i, part) in name.split(|c| c == '-' || c == '_').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
            out.extend(chars);
        }
    }
    if out.is_empty() {
        name.to_string()
    } else {
        out
    }
}

fn generated_readme(name: &str, version: &str, label: &str) -> String {
    format!(
        "# {label}\n\nVersion: {version}\n\nYAML Qefro application. Custom business operations require a Rust `AppModule`.\n\n## Entities\n\n- Customer\n\n## Install\n\n```bash\nqefro app validate {name}\nqefro app package {name}\nqefro app install {name}-{version}.qefro\nqefro migrate --app {name}\nqefro tenant app enable <tenant-slug> {name}\nqefro dev --app {name}\n```\n"
    )
}

pub fn is_package_path(name: &str) -> bool {
    name.ends_with(".qefro") && Path::new(name).is_file()
}

pub fn load_named_bundle(name: &str) -> Result<AppBundle> {
    let root = find_app_root(name).ok_or_else(|| {
        let hint = qefro_core::suggest_similar(name, known_app_names().iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        anyhow::anyhow!("unknown app '{name}'.{hint} Use `qefro app list`.")
    })?;
    AppBundle::load(&root).map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn installed_from_bundle(bundle: AppBundle) -> Result<InstalledApp> {
    let workflows = bundle.workflows.clone();
    let permissions = bundle.permissions.clone();
    let module = bundle.into_module();
    let mut app = InstalledApp::new(module);
    for wf in workflows {
        let def: WorkflowDef = serde_json::from_value(wf).context("invalid workflow yaml")?;
        app = app.workflow(def);
    }
    for grant in permissions {
        let g: PermissionGrant = serde_json::from_value(grant).context("invalid permission yaml")?;
        app = app.permission(g);
    }
    Ok(app)
}

pub fn cmd_app_validate(name: &str) -> Result<()> {
    let bundle = load_named_bundle(name)?;
    let report = validate_bundle(&bundle, &load_installed().refs());
    for w in &report.warnings {
        println!("warning: {w}");
    }
    if report.ok() {
        println!("ok  {} {}", bundle.manifest.name, bundle.manifest.version);
        Ok(())
    } else {
        for e in &report.errors {
            eprintln!("error: {e}");
        }
        bail!("validation failed");
    }
}

pub fn cmd_app_list() -> Result<()> {
    let installed = load_installed();
    let apps = discover_apps(&builtin_manifests());
    if apps.is_empty() {
        println!("(no applications discovered)");
        return Ok(());
    }
    println!("{:<16} {:<10} {:<12} {}", "NAME", "VERSION", "STATUS", "SOURCE");
    for app in apps {
        let name = &app.manifest.name;
        let status = if installed.is_disabled(name) {
            "disabled"
        } else if installed.is_installed(name) {
            "installed"
        } else {
            "available"
        };
        let version = installed
            .records
            .get(name)
            .map(|r| r.version.as_str())
            .unwrap_or(app.manifest.version.as_str());
        let source = if app.builtin {
            "builtin"
        } else if app.root.starts_with(store_dir()) {
            "package"
        } else {
            "catalog"
        };
        println!("{name:<16} {version:<10} {status:<12} {source}");
    }
    Ok(())
}

pub async fn cmd_app_info(name: &str) -> Result<()> {
    if is_package_path(name) {
        let (meta, files) = inspect_package(Path::new(name)).map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("{}", meta.name);
        println!("Version: {}", meta.version);
        println!("Package: {name}");
        println!("Checksum: {}", meta.sha256);
        println!("Files: {}", files.len());
        return Ok(());
    }
    let installed = load_installed();
    let status = if installed.is_disabled(name) {
        "Disabled"
    } else if installed.is_installed(name) {
        "Installed"
    } else {
        "Available"
    };
    if let Ok(bundle) = load_named_bundle(name) {
        let m = &bundle.manifest;
        println!("{}", if m.label.is_empty() { &m.name } else { &m.label });
        println!("Version: {}", m.version);
        println!("Description: {}", m.description);
        println!(
            "Framework compatibility: {}",
            if m.framework_version.is_empty() {
                "(any)"
            } else {
                m.framework_version.as_str()
            }
        );
        println!("Runtime: {FRAMEWORK_VERSION}");
        println!();
        println!("Entities: {}", bundle.entities.len());
        println!("Workflows: {}", bundle.workflows.len());
        println!("Reports: {}", bundle.reports.len());
        println!("Dashboards: {}", bundle.dashboards.len());
        println!("Print formats: {}", bundle.print_formats.len());
        println!("Permissions: {}", bundle.permissions.len());
        let deps = if m.dependencies.is_empty() {
            "(none)".into()
        } else {
            m.dependencies
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!("Dependencies: {deps}");
        println!("Status: {status}");
        if let Ok(url) = std::env::var("DATABASE_URL") {
            if let Ok(pool) = qefro_db::connect(&url).await {
                if let Ok(Some(row)) = qefro_db::app_registry::get_app(&pool, name).await {
                    println!("Registry version: {}", row.version);
                    println!("Registry status: {}", row.status);
                }
                if let Ok(n) = qefro_db::app_registry::enabled_tenant_count(&pool, name).await {
                    println!("Enabled tenants: {n}");
                }
                if let Ok(hist) = qefro_db::app_registry::version_history(&pool, name).await {
                    if !hist.is_empty() {
                        println!("Version history:");
                        for (v, at) in hist {
                            println!("  {v}  {at}");
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    let runtime = runtime_for(name)?;
    println!("{name}");
    println!("Entities: {}", runtime.entity_names().len());
    println!("Workflows: {}", runtime.workflows().len());
    println!("Operations: {}", runtime.operation_defs().len());
    println!("Status: {status}");
    Ok(())
}

pub fn cmd_app_package(name: &str, output: Option<&Path>) -> Result<()> {
    let root = find_app_root(name).ok_or_else(|| anyhow::anyhow!("unknown app '{name}'"))?;
    let text = fs::read_to_string(root.join("app.toml")).context("app.toml")?;
    let manifest: AppFileManifest = parse_app_toml(&text).map_err(|e| anyhow::anyhow!("{e}"))?;
    if manifest.source != "builtin" {
        let bundle = AppBundle::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
        let report = validate_bundle(&bundle, &load_installed().refs());
        report.fail().map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let dest = output
        .map(PathBuf::from)
        .unwrap_or_else(|| qefro_core::package::default_package_name(&manifest.name, &manifest.version));
    let meta = write_package(&root, &dest, &manifest.name, &manifest.version)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("wrote {} ({})", dest.display(), meta.sha256);
    Ok(())
}

pub async fn cmd_app_install(name: &str) -> Result<()> {
    if is_package_path(name) {
        return install_package(Path::new(name)).await;
    }
    if !known_app_names().iter().any(|n| n == name) && find_app_root(name).is_none() {
        let hint = qefro_core::suggest_similar(name, known_app_names().iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        bail!("unknown app '{name}'.{hint}");
    }
    if let Ok(bundle) = load_named_bundle(name) {
        let report = validate_bundle(&bundle, &load_installed().refs());
        if !report.ok() {
            for e in &report.errors {
                eprintln!("error: {e}");
            }
            bail!("validation failed; not installed");
        }
        let source = if bundle.manifest.source.is_empty() {
            "catalog"
        } else {
            bundle.manifest.source.as_str()
        };
        let set = mark_installed(&bundle.manifest.name, &bundle.manifest.version, source, None)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        persist_registry(&bundle.manifest.clone().into(), "installed", None).await?;
        record_lifecycle(None, &bundle.manifest.name, Some(&bundle.manifest.version), "install").await;
        println!("installed: {}", set.installed.join(", "));
        Ok(())
    } else {
        let set = mark_installed(name, "1.0.0", "builtin", None).map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("installed: {}", set.installed.join(", "));
        Ok(())
    }
}

async fn install_package(path: &Path) -> Result<()> {
    let (meta, _) = inspect_package(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dest = store_dir().join(&meta.name);
    if dest.exists() {
        fs::remove_dir_all(&dest).ok();
    }
    fs::create_dir_all(&dest)?;
    if let Err(e) = extract_package(path, &dest) {
        let _ = fs::remove_dir_all(&dest);
        bail!("{e}");
    }
    let bundle = match AppBundle::load(&dest) {
        Ok(b) => b,
        Err(e) => {
            let _ = fs::remove_dir_all(&dest);
            bail!("{e}");
        }
    };
    let report = validate_bundle(&bundle, &load_installed().refs());
    if !report.ok() {
        let _ = fs::remove_dir_all(&dest);
        for e in &report.errors {
            eprintln!("error: {e}");
        }
        bail!("validation failed; package not installed");
    }
    let set = mark_installed(
        &meta.name,
        &meta.version,
        "package",
        Some(meta.sha256.clone()),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    persist_registry(
        &bundle.manifest.clone().into(),
        "installed",
        Some(&meta.sha256),
    )
    .await?;
    record_lifecycle(None, &meta.name, Some(&meta.version), "install").await;
    println!(
        "installed {} {} from {}",
        meta.name,
        meta.version,
        path.display()
    );
    println!("installed set: {}", set.installed.join(", "));
    Ok(())
}

pub async fn cmd_app_update(name: &str, yes: bool) -> Result<()> {
    if is_package_path(name) {
        return update_from_package(Path::new(name), yes).await;
    }
    let bundle = load_named_bundle(name)?;
    mark_installed(
        &bundle.manifest.name,
        &bundle.manifest.version,
        "catalog",
        None,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    persist_registry(&bundle.manifest.clone().into(), "installed", None).await?;
    apply_pending_migrations(&bundle, yes).await?;
    record_lifecycle(None, name, Some(&bundle.manifest.version), "upgrade").await;
    println!("updated {} to {}", name, bundle.manifest.version);
    Ok(())
}

async fn update_from_package(path: &Path, yes: bool) -> Result<()> {
    let (meta, _) = inspect_package(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let current = load_installed();
    if let Some(rec) = current.records.get(&meta.name) {
        if !qefro_core::version::is_upgrade(&rec.version, &meta.version).unwrap_or(true) && !yes {
            bail!(
                "installed {} is {}; package is {}. pass --yes to replace",
                meta.name,
                rec.version,
                meta.version
            );
        }
        if let Ok(old) = load_named_bundle(&meta.name) {
            let tmp = store_dir().join(format!(".tmp-{}", meta.name));
            let _ = fs::remove_dir_all(&tmp);
            fs::create_dir_all(&tmp)?;
            extract_package(path, &tmp).map_err(|e| anyhow::anyhow!("{e}"))?;
            let new_bundle = AppBundle::load(&tmp).map_err(|e| anyhow::anyhow!("{e}"))?;
            let dropped = qefro_core::destructive_field_removals(&old, &new_bundle);
            if !dropped.is_empty() {
                eprintln!("metadata removed (columns will NOT be dropped):");
                for d in &dropped {
                    eprintln!("  {d}");
                }
                if !yes && !confirm("continue update?")? {
                    let _ = fs::remove_dir_all(&tmp);
                    bail!("update cancelled");
                }
            }
            let dest = store_dir().join(&meta.name);
            let _ = fs::remove_dir_all(&dest);
            fs::rename(&tmp, &dest)?;
            mark_installed(
                &meta.name,
                &meta.version,
                "package",
                Some(meta.sha256.clone()),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
            persist_registry(
                &new_bundle.manifest.clone().into(),
                "installed",
                Some(&meta.sha256),
            )
            .await?;
            apply_pending_migrations(&new_bundle, yes).await?;
            record_lifecycle(None, &meta.name, Some(&meta.version), "upgrade").await;
            println!("updated {} → {}", rec.version, meta.version);
            return Ok(());
        }
    }
    install_package(path).await
}

async fn apply_pending_migrations(bundle: &AppBundle, yes: bool) -> Result<()> {
    let Some(url) = std::env::var("DATABASE_URL").ok() else {
        return Ok(());
    };
    let pool = qefro_db::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let pending =
        qefro_db::app_registry::pending_migrations(&pool, &bundle.manifest.name, &bundle.migrations)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    for mig in pending {
        if mig.looks_destructive() && !yes {
            eprintln!(
                "skipping destructive migration {} ({}); pass --yes",
                mig.id, mig.description
            );
            continue;
        }
        qefro_db::app_registry::apply_migration(&pool, &bundle.manifest.name, &mig, yes)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("applied migration {}", mig.id);
    }
    Ok(())
}

pub fn cmd_app_disable(name: &str) -> Result<()> {
    let set = disable_app(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("disabled {name} globally (tenant data kept)");
    println!("installed: {}", set.installed.join(", "));
    Ok(())
}

pub fn cmd_app_enable(name: &str) -> Result<()> {
    let set = enable_app(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("enabled {name} globally. Tenants still enable the app separately.");
    println!("active: {}", set.active().join(", "));
    Ok(())
}

pub async fn cmd_app_uninstall(name: &str) -> Result<()> {
    let set = remove_app(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if let Ok(pool) = qefro_db::connect(&url).await {
            let _ = qefro_db::app_registry::uninstall(&pool, name).await;
            let _ = qefro_db::app_registry::record_lifecycle(&pool, None, name, None, "uninstall")
                .await;
        }
    }
    println!("uninstalled {name} (business data and schema were kept)");
    if set.installed.is_empty() {
        println!("installed: (none — `qefro dev` will load restaurant + crm)");
    } else {
        println!("installed: {}", set.installed.join(", "));
    }
    Ok(())
}

pub async fn cmd_tenant_app(enable: bool, tenant: &str, app: &str) -> Result<()> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://qefro:qefro@127.0.0.1:5432/qefro".into());
    let pool = qefro_db::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let installed = load_installed();
    if !installed.is_installed(app) {
        bail!("app '{app}' is not installed globally");
    }
    if enable && installed.is_disabled(app) {
        bail!("app '{app}' is globally disabled");
    }
    let tenants = qefro_tenant::TenantService::new(pool.clone());
    let tenant_row = tenants
        .get_by_slug(tenant)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut config = tenants
        .get_config(tenant_row.id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let entitlements = qefro_core::Entitlements::new();
    if enable {
        if !entitlements.can_enable(app, &installed.active(), config.plan.as_deref()) {
            bail!("plan does not allow '{app}' for this tenant");
        }
        if config.enabled_apps.is_empty() {
            config.enabled_apps = installed.active();
        }
        if !config.enabled_apps.iter().any(|a| a == app) {
            config.enabled_apps.push(app.to_string());
        }
    } else if config.enabled_apps.is_empty() {
        config.enabled_apps = installed.active().into_iter().filter(|a| a != app).collect();
    } else {
        config.enabled_apps.retain(|a| a != app);
    }
    tenants
        .upsert_config(tenant_row.id, &config)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let on = if enable {
        "tenant_enable"
    } else {
        "tenant_disable"
    };
    let _ = qefro_db::app_registry::record_lifecycle(
        &pool,
        Some(tenant_row.id),
        app,
        installed.records.get(app).map(|r| r.version.as_str()),
        on,
    )
    .await;
    if enable {
        println!("enabled {app} for tenant {tenant}");
    } else {
        println!("disabled {app} for tenant {tenant}");
    }
    Ok(())
}

pub async fn cmd_app_seed(name: &str, tenant: &str, kind: Option<&str>) -> Result<()> {
    let bundle = load_named_bundle(name)?;
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://qefro:qefro@127.0.0.1:5432/qefro".into());
    let pool = qefro_db::connect(&url)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let tenants = qefro_tenant::TenantService::new(pool.clone());
    let tenant_row = tenants
        .get_by_slug(tenant)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let env = std::env::var("QEFRO_ENV").unwrap_or_else(|_| "development".into());
    let kinds: Vec<&str> = match kind {
        Some(k) => vec![k],
        None => vec!["install", "tenant"],
    };
    let (_router, state) = runtime_for(name)?.build().await?;
    let user_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT user_id FROM user_tenants WHERE tenant_id = $1 LIMIT 1",
    )
    .bind(tenant_row.id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(tenant_row.id);
    let mut ctx = qefro_core::OpContext::new(tenant_row.id, user_id, vec!["Admin".into()]);
    ctx.enabled_apps = vec![name.to_string()];
    let mut created = 0u32;
    for batch in &bundle.seeds {
        if !kinds.iter().any(|k| *k == batch.kind) {
            continue;
        }
        if batch.kind == "development" && env != "development" {
            continue;
        }
        created += qefro_db::apply_seed_batch(&state.entities, &ctx, batch)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let _ =
            qefro_db::app_registry::mark_seed_applied(&pool, tenant_row.id, name, &batch.kind).await;
    }
    println!("seeded {created} row(s) for {name} / {tenant}");
    Ok(())
}

pub async fn cmd_doctor() -> Result<()> {
    println!("qefro doctor");
    println!("framework: {FRAMEWORK_VERSION}");
    match std::process::Command::new("rustc").arg("--version").output() {
        Ok(out) => println!("rustc: {}", String::from_utf8_lossy(&out.stdout).trim()),
        Err(_) => println!("rustc: missing"),
    }
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://qefro:qefro@127.0.0.1:5432/qefro".into());
    println!("DATABASE_URL: {url}");
    let pool = match qefro_db::connect(&url).await {
        Ok(pool) => {
            match qefro_db::pool::ping(&pool).await {
                Ok(()) => println!("postgres: ok"),
                Err(e) => println!("postgres: {e}"),
            }
            Some(pool)
        }
        Err(e) => {
            println!("postgres: {e}");
            None
        }
    };
    let installed = load_installed();
    let apps = discover_apps(&builtin_manifests());
    for app in &apps {
        let name = &app.manifest.name;
        if app.builtin && AppBundle::load(&app.root).is_err() {
            println!("✓ {name} builtin runtime");
            continue;
        }
        match AppBundle::load(&app.root) {
            Ok(bundle) => {
                let report = validate_bundle(&bundle, &installed.refs());
                if !report.ok() {
                    println!("✗ {name} {}", report.errors.join("; "));
                    continue;
                }
                print!("✓ {name} manifest");
                if let Some(pool) = &pool {
                    match qefro_db::app_registry::pending_migrations(pool, name, &bundle.migrations)
                        .await
                    {
                        Ok(pending) if pending.is_empty() => print!("  ✓ migrations"),
                        Ok(pending) => print!("  ⚠ migration pending: {}", pending[0].version),
                        Err(_) => print!("  · registry not ready"),
                    }
                }
                println!();
                for w in report.warnings {
                    println!("  warning: {w}");
                }
            }
            Err(e) => println!("✗ {name} {e}"),
        }
    }
    if installed.installed.is_empty() {
        println!("installed: (default restaurant, crm)");
    } else {
        println!("installed: {}", installed.installed.join(", "));
        if !installed.disabled.is_empty() {
            println!("disabled: {}", installed.disabled.join(", "));
        }
    }
    Ok(())
}

async fn persist_registry(
    manifest: &qefro_core::AppManifest,
    status: &str,
    sha: Option<&str>,
) -> Result<()> {
    let Some(url) = std::env::var("DATABASE_URL").ok() else {
        return Ok(());
    };
    let Ok(pool) = qefro_db::connect(&url).await else {
        return Ok(());
    };
    qefro_db::app_registry::upsert_app(&pool, manifest, status, sha)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

async fn record_lifecycle(tenant: Option<uuid::Uuid>, app: &str, version: Option<&str>, on: &str) {
    let Some(url) = std::env::var("DATABASE_URL").ok() else {
        return;
    };
    let Ok(pool) = qefro_db::connect(&url).await else {
        return;
    };
    let _ = qefro_db::app_registry::record_lifecycle(&pool, tenant, app, version, on).await;
}

fn confirm(prompt: &str) -> Result<bool> {
    eprint!("{prompt} [y/N] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}
