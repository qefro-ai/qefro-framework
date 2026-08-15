use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    discover_apps, install_app, load_installed, load_yaml_entities, parse_app_toml, remove_app,
    suggest_similar, AppManifest, AppModule,
};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "qefro", version, about = "Qefro Framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new standalone Qefro project
    New {
        name: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Application module helpers
    App {
        #[command(subcommand)]
        command: AppCommands,
    },
    /// Entity helpers
    Entity {
        #[command(subcommand)]
        command: EntityCommands,
    },
    /// Apply PostgreSQL schema from registered modules
    Migrate {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Run the development server
    Dev {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Print generated routes
    Routes {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Print the permission matrix
    Permissions {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Print registered workflows
    Workflows {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Print generated agent tools
    Tools {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// List business operations
    Operations {
        entity: Option<String>,
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Invoke a business operation through the running API
    Action {
        entity: String,
        id: String,
        name: String,
        #[arg(long)]
        input: Option<String>,
        #[arg(long, env = "QEFRO_URL", default_value = "http://127.0.0.1:8080")]
        url: String,
        #[arg(long, env = "QEFRO_TOKEN")]
        token: Option<String>,
    },
    /// Check local development prerequisites
    Doctor,
}

#[derive(Subcommand)]
enum AppCommands {
    /// Create an application module skeleton under apps/<name>
    New { name: String },
    /// List discovered applications
    List,
    /// Mark an application as installed for `qefro dev`
    Install { name: String },
    /// Remove an application from the installed set
    Remove { name: String },
    /// Show application metadata
    Info { name: String },
}

#[derive(Subcommand)]
enum EntityCommands {
    List {
        #[arg(long, default_value = "all")]
        app: String,
    },
    Show {
        name: String,
        #[arg(long, default_value = "all")]
        app: String,
    },
    Create {
        name: String,
        #[arg(long)]
        app: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::New { name, path } => cmd_new(&name, path.as_deref())?,
        Commands::App { command } => match command {
            AppCommands::New { name } => cmd_app_new(&name)?,
            AppCommands::List => cmd_app_list(),
            AppCommands::Install { name } => cmd_app_install(&name)?,
            AppCommands::Remove { name } => cmd_app_remove(&name)?,
            AppCommands::Info { name } => cmd_app_info(&name)?,
        },
        Commands::Entity { command } => match command {
            EntityCommands::List { app } => {
                let runtime = runtime_for(&app)?;
                for name in runtime.entity_names() {
                    println!("{name}");
                }
            }
            EntityCommands::Show { name, app } => cmd_entity_show(&app, &name)?,
            EntityCommands::Create { name, app } => cmd_entity_create(&name, app.as_deref())?,
        },
        Commands::Migrate { app } => {
            let runtime = runtime_for(&app)?;
            let _ = runtime.build().await?;
            println!("schema applied");
        }
        Commands::Dev { app } => {
            runtime_for(&app)?.serve().await?;
        }
        Commands::Routes { app } => {
            for route in runtime_for(&app)?.routes_summary() {
                println!("{route}");
            }
        }
        Commands::Permissions { app } => {
            for grant in runtime_for(&app)?.permission_grants() {
                let actions: Vec<_> = grant.actions.iter().map(|a| a.as_str()).collect();
                println!("{}  {}  {}", grant.role, grant.entity, actions.join(","));
            }
        }
        Commands::Workflows { app } => {
            for wf in runtime_for(&app)?.workflows() {
                let states: Vec<_> = wf.states.iter().map(|s| s.name.as_str()).collect();
                println!("{}  entity={}  initial={}", wf.name, wf.entity, wf.initial);
                println!("  states: {}", states.join(" → "));
                for t in &wf.transitions {
                    let label = if t.label.is_empty() {
                        t.name.clone()
                    } else {
                        t.label.clone()
                    };
                    let roles = if t.allowed_roles.is_empty() {
                        "*".into()
                    } else {
                        t.allowed_roles.join(",")
                    };
                    println!(
                        "  {label} ({})  {} → {}  roles={roles}",
                        t.name, t.from, t.to
                    );
                }
            }
        }
        Commands::Tools { app } => {
            for name in runtime_for(&app)?.tool_names() {
                println!("{name}");
            }
        }
        Commands::Operations { entity, app } => {
            let runtime = runtime_for(&app)?;
            for def in runtime.operation_defs() {
                if let Some(filter) = &entity {
                    if !def.entity.eq_ignore_ascii_case(filter) {
                        continue;
                    }
                }
                println!(
                    "{}\t{}\t{}\t{}",
                    def.entity, def.name, def.label, def.permission
                );
            }
        }
        Commands::Action {
            entity,
            id,
            name,
            input,
            url,
            token,
        } => cmd_action(&entity, &id, &name, input.as_deref(), &url, token.as_deref()).await?,
        Commands::Doctor => cmd_doctor().await?,
    }
    Ok(())
}

fn runtime_for(app: &str) -> Result<QefroRuntime> {
    let mut runtime = QefroRuntime::new(Config::from_env()?);
    for name in resolve_apps(app)? {
        install_named(&mut runtime, &name)?;
    }
    Ok(runtime)
}

fn resolve_apps(selector: &str) -> Result<Vec<String>> {
    match selector {
        "all" | "" => {
            let installed = load_installed();
            if installed.installed.is_empty() {
                Ok(vec!["restaurant".into(), "crm".into()])
            } else {
                Ok(installed.installed)
            }
        }
        other => Ok(vec![other.to_string()]),
    }
}

fn install_named(runtime: &mut QefroRuntime, name: &str) -> Result<()> {
    match name {
        "restaurant" => {
            runtime.install(qefro_restaurant::installed());
        }
        "crm" => {
            runtime.install(qefro_crm::installed());
        }
        other => {
            runtime.install(load_fs_app(other)?);
        }
    }
    Ok(())
}

fn app_root_candidates(name: &str) -> Vec<PathBuf> {
    vec![PathBuf::from("apps").join(name), PathBuf::from(name)]
}

fn find_app_root(name: &str) -> Option<PathBuf> {
    app_root_candidates(name)
        .into_iter()
        .find(|p| p.join("app.toml").exists())
}

fn load_fs_app(name: &str) -> Result<InstalledApp> {
    let root = find_app_root(name).ok_or_else(|| {
        let known = known_app_names();
        let hint = suggest_similar(name, known.iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        anyhow::anyhow!("unknown app '{name}'.{hint} Use `qefro app list`.")
    })?;
    let manifest = parse_app_toml(
        &fs::read_to_string(root.join("app.toml")).with_context(|| root.join("app.toml").display().to_string())?,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut builder = AppModule::new(&manifest.name)
        .version(&manifest.version)
        .label(&manifest.label)
        .description(&manifest.description);
    for entity in load_yaml_entities(&root).map_err(|e| anyhow::anyhow!("{e}"))? {
        builder = builder.entity(entity);
    }
    let mut app = InstalledApp::new(builder.build());
    for wf in load_yaml_dir::<WorkflowDef>(&root.join("workflows"))? {
        app = app.workflow(wf);
    }
    for grant in load_permission_grants(&root.join("permissions"))? {
        app = app.permission(grant);
    }
    Ok(app)
}

fn load_yaml_dir<T: serde::de::DeserializeOwned>(dir: &Path) -> Result<Vec<T>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        if ext == "json" {
            out.push(serde_json::from_str(&text).with_context(|| path.display().to_string())?);
        } else {
            out.push(serde_yaml::from_str(&text).with_context(|| path.display().to_string())?);
        }
    }
    Ok(out)
}

fn load_permission_grants(dir: &Path) -> Result<Vec<PermissionGrant>> {
    let mut grants = Vec::new();
    if !dir.exists() {
        return Ok(grants);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let value: serde_json::Value = if ext == "json" {
            serde_json::from_str(&text)?
        } else {
            serde_yaml::from_str(&text)?
        };
        if value.is_array() {
            grants.extend(serde_json::from_value::<Vec<PermissionGrant>>(value)?);
        } else {
            grants.push(serde_json::from_value(value)?);
        }
    }
    Ok(grants)
}

fn known_app_names() -> Vec<String> {
    let mut names: Vec<String> = discover_apps(&builtin_manifests())
        .into_iter()
        .map(|a| a.manifest.name)
        .collect();
    names.extend(["restaurant".into(), "crm".into()]);
    names.sort();
    names.dedup();
    names
}

fn builtin_manifests() -> Vec<AppManifest> {
    vec![
        manifest_of(&qefro_restaurant::installed()),
        manifest_of(&qefro_crm::installed()),
    ]
}

fn manifest_of(app: &InstalledApp) -> AppManifest {
    AppManifest {
        name: app.module.name.clone(),
        version: app.module.version.clone(),
        label: app.module.label.clone(),
        description: app.module.description.clone(),
        depends_on: vec!["qefro-framework".into()],
    }
}

fn cmd_new(name: &str, path: Option<&Path>) -> Result<()> {
    let root = path
        .map(|p| p.join(name))
        .unwrap_or_else(|| PathBuf::from(name));
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("entities"))?;
    fs::create_dir_all(root.join("workflows"))?;
    fs::create_dir_all(root.join("permissions"))?;
    fs::create_dir_all(root.join("hooks"))?;
    fs::create_dir_all(root.join("tools"))?;
    fs::create_dir_all(root.join("seeds"))?;

    let ident = name.replace('-', "_");
    fs::write(
        root.join("app.toml"),
        format!(
            "name = \"{name}\"\nversion = \"0.1.0\"\nlabel = \"{name}\"\ndescription = \"\"\ndepends_on = [\"qefro-framework\"]\n"
        ),
    )?;
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{ident}"
version = "0.1.0"
edition = "2021"

[dependencies]
qefro-core = {{ path = "../qefro-framework/crates/qefro-core" }}
qefro-api = {{ path = "../qefro-framework/crates/qefro-api" }}
tokio = {{ version = "1", features = ["full"] }}
anyhow = "1"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
"#
        ),
    )?;
    fs::write(
        root.join("src/main.rs"),
        r#"use anyhow::Result;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef};

fn app() -> InstalledApp {
    let module = AppModule::new(env!("CARGO_PKG_NAME"))
        .entity(
            EntityDef::new("Customer")
                .field(FieldDef::string("name").required().searchable())
                .field(FieldDef::string("email").email().nullable())
                .build(),
        )
        .build();
    InstalledApp::new(module)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let mut runtime = QefroRuntime::new(Config::from_env()?);
    runtime.install(app());
    runtime.serve().await?;
    Ok(())
}
"#,
    )?;
    fs::write(
        root.join(".env.example"),
        "DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro\nJWT_SECRET=change-me\nQEFRO_BIND=127.0.0.1:8080\n",
    )?;
    fs::write(
        root.join("README.md"),
        format!("# {name}\n\nGenerated by `qefro new`.\n\n```bash\nexport DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro\ncargo run\n```\n"),
    )?;
    println!("created {}", root.display());
    Ok(())
}

fn is_framework_root() -> bool {
    Path::new("Cargo.toml").exists() && Path::new("crates").is_dir()
}

fn cmd_app_new(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid app name '{name}' (use letters, numbers, '-' or '_')");
    }
    if matches!(name, "restaurant" | "crm") && is_framework_root() {
        let catalog = PathBuf::from("apps").join(name);
        if !catalog.exists() {
            write_catalog_stub(&catalog, name)?;
        }
        println!(
            "'{name}' is a built-in application. Catalog: {}\nInstall it with: qefro app install {name}\nRun it with:     qefro dev --app {name}",
            catalog.display()
        );
        return Ok(());
    }
    let root = if is_framework_root() {
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
    println!("  cd {}", root.display());
    println!("  qefro entity create Customer");
    println!("  qefro app install {name}");
    println!("  qefro migrate --app {name}");
    println!("  qefro dev --app {name}");
    Ok(())
}

fn write_catalog_stub(root: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(root)?;
    let (version, label, description) = match name {
        "restaurant" => (
            "0.2.0",
            "Restaurant",
            "Tables, reservations, menus, orders, and payments",
        ),
        "crm" => (
            "0.2.0",
            "CRM",
            "Leads, contacts, opportunities, and activities",
        ),
        _ => ("0.1.0", name, ""),
    };
    fs::write(
        root.join("app.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\nlabel = \"{label}\"\ndescription = \"{description}\"\ndepends_on = [\"qefro-framework\"]\n"
        ),
    )?;
    fs::write(
        root.join("README.md"),
        format!(
            "# {label}\n\nBuilt-in Qefro application. Runtime source: `examples/{name}`.\nEntities, workflows, and permissions are registered from Rust — they are not hardcoded in framework core.\n"
        ),
    )?;
    Ok(())
}

fn write_app_skeleton(root: &Path, name: &str) -> Result<()> {
    for dir in ["entities", "workflows", "permissions", "hooks", "tools", "seeds"] {
        fs::create_dir_all(root.join(dir))?;
    }
    fs::write(
        root.join("app.toml"),
        format!(
            "name = \"{name}\"\nversion = \"0.1.0\"\nlabel = \"{name}\"\ndescription = \"\"\ndepends_on = [\"qefro-framework\"]\n"
        ),
    )?;
    fs::write(
        root.join("README.md"),
        format!(
            "# {name}\n\nGenerated by `qefro app new`.\n\n```bash\nqefro entity create Customer\nqefro app install {name}\nqefro migrate --app {name}\nqefro dev --app {name}\n```\n"
        ),
    )?;
    fs::write(
        root.join("entities/.gitkeep"),
        "",
    )?;
    Ok(())
}

fn cmd_app_list() {
    let installed = load_installed();
    let apps = discover_apps(&builtin_manifests());
    if apps.is_empty() {
        println!("(no applications discovered)");
        return;
    }
    for app in apps {
        let status = if installed.installed.iter().any(|n| n == &app.manifest.name) {
            "installed"
        } else {
            "available"
        };
        let kind = if app.builtin { "builtin" } else { "fs" };
        println!(
            "{:<16} {:<8} {:<8} {:<10} {}",
            app.manifest.name, app.manifest.version, kind, status, app.manifest.description
        );
    }
}

fn cmd_app_install(name: &str) -> Result<()> {
    if !known_app_names().iter().any(|n| n == name) && find_app_root(name).is_none() {
        let hint = suggest_similar(name, known_app_names().iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        bail!("unknown app '{name}'.{hint}");
    }
    let set = install_app(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("installed: {}", set.installed.join(", "));
    Ok(())
}

fn cmd_app_remove(name: &str) -> Result<()> {
    let set = remove_app(name).map_err(|e| anyhow::anyhow!("{e}"))?;
    if set.installed.is_empty() {
        println!("installed: (none — `qefro dev` will load restaurant + crm)");
    } else {
        println!("installed: {}", set.installed.join(", "));
    }
    Ok(())
}

fn cmd_app_info(name: &str) -> Result<()> {
    let runtime = runtime_for(name)?;
    let apps = runtime.installed_apps();
    if apps.is_empty() {
        bail!("app '{name}' has no registered module");
    }
    println!("name:        {name}");
    println!("module(s):   {}", apps.join(", "));
    println!("entities:    {}", runtime.entity_names().join(", "));
    let wfs: Vec<_> = runtime.workflows().into_iter().map(|w| w.name).collect();
    println!(
        "workflows:   {}",
        if wfs.is_empty() {
            "(none)".into()
        } else {
            wfs.join(", ")
        }
    );
    println!("tools:       {}", runtime.tool_names().len());
    Ok(())
}

fn cmd_entity_show(app: &str, name: &str) -> Result<()> {
    let runtime = runtime_for(app)?;
    let Some(entity) = runtime.entity(name) else {
        let known = runtime.entity_names();
        let hint = suggest_similar(name, known.iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        bail!("entity '{name}' not found.{hint}");
    };
    println!("name:           {}", entity.name);
    println!("label:          {}", entity.label);
    println!("slug:           {}", entity.slug);
    println!("table:          {}", entity.table);
    println!("module:         {}", entity.module.clone().unwrap_or_default());
    println!("workflow:       {}", entity.workflow.clone().unwrap_or_else(|| "(none)".into()));
    println!("display_field:  {}", entity.display_field);
    println!("fields:");
    for field in &entity.fields {
        let mut flags = Vec::new();
        if field.required {
            flags.push("required");
        }
        if field.searchable {
            flags.push("searchable");
        }
        if field.unique {
            flags.push("unique");
        }
        if let Some(rel) = &field.relation {
            flags.push(match rel.kind {
                qefro_core::RelationKind::ManyToOne => "many-to-one",
                qefro_core::RelationKind::OneToMany => "one-to-many",
                qefro_core::RelationKind::ManyToMany => "many-to-many",
            });
        }
        println!(
            "  {:<20} {:<12} {}",
            field.name,
            field.field_type.as_str(),
            flags.join(", ")
        );
        if let Some(rel) = &field.relation {
            println!("                     → {}", rel.target_entity);
        }
    }
    Ok(())
}

fn cmd_entity_create(name: &str, app: Option<&str>) -> Result<()> {
    let dir = entity_write_dir(app)?;
    fs::create_dir_all(&dir)?;
    let slug = qefro_core::ident::snake_case(name);
    let path = dir.join(format!("{slug}.yaml"));
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    fs::write(
        &path,
        format!(
            "name: {name}\nlabel: {name}\nfields:\n  - name: name\n    type: string\n    required: true\n    searchable: true\n"
        ),
    )?;
    println!("wrote {}", path.display());
    Ok(())
}

fn entity_write_dir(app: Option<&str>) -> Result<PathBuf> {
    if let Some(app) = app {
        let root = find_app_root(app).unwrap_or_else(|| PathBuf::from("apps").join(app));
        return Ok(root.join("entities"));
    }
    if Path::new("app.toml").exists() {
        return Ok(PathBuf::from("entities"));
    }
    Ok(PathBuf::from("entities"))
}

async fn cmd_action(
    entity: &str,
    id: &str,
    name: &str,
    input: Option<&str>,
    url: &str,
    token: Option<&str>,
) -> Result<()> {
    let runtime = runtime_for("all")?;
    let def = runtime
        .entity(entity)
        .with_context(|| format!("unknown entity '{entity}'"))?;
    let payload: serde_json::Value = match input {
        Some(raw) => serde_json::from_str(raw).context("invalid --input JSON")?,
        None => serde_json::json!({}),
    };
    let endpoint = format!(
        "{}/api/v1/{}/{id}/actions/{name}",
        url.trim_end_matches('/'),
        def.slug
    );
    let mut req = reqwest::Client::new().post(&endpoint).json(&payload);
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let response = req.send().await.context("request failed")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("action failed ({status}): {body}");
    }
    println!("{body}");
    Ok(())
}

async fn cmd_doctor() -> Result<()> {
    println!("qefro doctor");
    match Command::new("rustc").arg("--version").output() {
        Ok(out) => println!("rustc: {}", String::from_utf8_lossy(&out.stdout).trim()),
        Err(_) => println!("rustc: missing"),
    }
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://qefro:qefro@127.0.0.1:5432/qefro".into());
    println!("DATABASE_URL: {url}");
    match qefro_db::connect(&url).await {
        Ok(pool) => {
            qefro_db::pool::ping(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("postgres: ok");
        }
        Err(e) => println!("postgres: {e}"),
    }
    if Path::new("app.toml").exists() {
        println!("app.toml: present");
    } else {
        println!("app.toml: not in cwd (ok if running framework examples)");
    }
    if Path::new("apps").is_dir() {
        let apps = discover_apps(&builtin_manifests());
        println!(
            "apps/: {} discovered ({})",
            apps.len(),
            apps.iter()
                .map(|a| a.manifest.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let installed = load_installed();
    if installed.installed.is_empty() {
        println!("installed: (default restaurant, crm)");
    } else {
        println!("installed: {}", installed.installed.join(", "));
    }
    Ok(())
}
