use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{discover_apps, load_installed, suggest_similar, AppManifest};
use std::fs;
use std::path::{Path, PathBuf};

mod app_cmd;

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
    /// Application catalog (new, validate, package, install)
    #[command(
        after_help = "Schema and the HTTP server are top-level commands:\n  qefro migrate --app <name>\n  qefro dev --app <name>\nThere is no `qefro app migrate` or `qefro app run`."
    )]
    App {
        #[command(subcommand)]
        command: AppCommands,
    },
    /// Entity helpers
    Entity {
        #[command(subcommand)]
        command: EntityCommands,
    },
    /// Apply PostgreSQL schema from registered modules (`qefro migrate`, not `qefro app migrate`)
    Migrate {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Run the development server (`qefro dev`, not `qefro app run`)
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
    /// Run the HTTP server (production). Set QEFRO_EMBED_WORKER=false and run `qefro worker` separately.
    Serve {
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Poll and run worker-safe background jobs
    Worker,
    /// Check local development prerequisites and installed apps
    Doctor,
    /// Validate application metadata (entities, relations, workflows, views)
    Validate {
        #[arg(default_value = "all")]
        app: String,
    },
    /// Inspect an entity, composed page, or automation
    /// (`qefro inspect automation order_confirmation`)
    Inspect {
        name: String,
        target: Option<String>,
        #[arg(long, default_value = "all")]
        app: String,
    },
    /// Tenant helpers
    Tenant {
        #[command(subcommand)]
        command: TenantCommands,
    },
}

#[derive(Subcommand)]
#[command(
    after_help = "Schema: `qefro migrate --app <name>`. Server: `qefro dev --app <name>`.\nThere is no `qefro app migrate` or `qefro app run`."
)]
enum AppCommands {
    /// Create an application module skeleton under apps/<name>
    New { name: String },
    /// List discovered applications
    List,
    /// Validate an application package or catalog directory
    Validate { name: String },
    /// Mark an application as installed, or install a .qefro package
    Install { name: String },
    /// Reload catalog metadata or install a newer .qefro package
    Update {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Write a .qefro package
    Package {
        name: String,
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Globally disable an installed application (data kept)
    Disable { name: String },
    /// Globally re-enable a disabled application
    Enable { name: String },
    /// Remove application registration (data kept)
    Uninstall { name: String },
    /// Remove an application from the installed set (alias of uninstall)
    Remove { name: String },
    /// Show application metadata
    Info { name: String },
    /// Apply seed data for a tenant
    Seed {
        name: String,
        #[arg(long)]
        tenant: String,
        #[arg(long)]
        kind: Option<String>,
    },
}

#[derive(Subcommand)]
enum TenantCommands {
    /// Enable or disable an application for one tenant
    App {
        #[command(subcommand)]
        command: TenantAppCommands,
    },
}

#[derive(Subcommand)]
enum TenantAppCommands {
    Enable { tenant: String, app: String },
    Disable { tenant: String, app: String },
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
    #[command(visible_alias = "new")]
    Create {
        name: String,
        #[arg(long)]
        app: Option<String>,
    },
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = std::env::var("QEFRO_LOG_LEVEL").unwrap_or_else(|_| "info".into());
        tracing_subscriber::EnvFilter::new(format!("{level},sqlx=warn,tower_http=info"))
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Dev { .. }
        | Commands::Serve { .. }
        | Commands::Worker
        | Commands::Migrate { .. } => {
            init_tracing();
            let env = std::env::var("QEFRO_ENV").unwrap_or_else(|_| "development".into());
            tracing::info!(env, "qefro starting");
        }
        _ => {}
    }

    match cli.command {
        Commands::New { name, path } => cmd_new(&name, path.as_deref())?,
        Commands::App { command } => match command {
            AppCommands::New { name } => app_cmd::cmd_app_new(&name)?,
            AppCommands::List => app_cmd::cmd_app_list()?,
            AppCommands::Validate { name } => app_cmd::cmd_app_validate(&name)?,
            AppCommands::Install { name } => app_cmd::cmd_app_install(&name).await?,
            AppCommands::Update { name, yes } => app_cmd::cmd_app_update(&name, yes).await?,
            AppCommands::Package { name, output } => {
                app_cmd::cmd_app_package(&name, output.as_deref())?
            }
            AppCommands::Disable { name } => app_cmd::cmd_app_disable(&name)?,
            AppCommands::Enable { name } => app_cmd::cmd_app_enable(&name)?,
            AppCommands::Uninstall { name } | AppCommands::Remove { name } => {
                app_cmd::cmd_app_uninstall(&name).await?
            }
            AppCommands::Info { name } => app_cmd::cmd_app_info(&name).await?,
            AppCommands::Seed { name, tenant, kind } => {
                app_cmd::cmd_app_seed(&name, &tenant, kind.as_deref()).await?
            }
        },
        Commands::Tenant { command } => match command {
            TenantCommands::App { command } => match command {
                TenantAppCommands::Enable { tenant, app } => {
                    app_cmd::cmd_tenant_app(true, &tenant, &app).await?
                }
                TenantAppCommands::Disable { tenant, app } => {
                    app_cmd::cmd_tenant_app(false, &tenant, &app).await?
                }
            },
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
            let _ = runtime_for(&app)?.with_auto_migrate(true).build().await?;
            println!("schema applied");
        }
        Commands::Dev { app } => {
            runtime_for(&app)?.serve().await?;
        }
        Commands::Serve { app } => {
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
        } => {
            cmd_action(
                &entity,
                &id,
                &name,
                input.as_deref(),
                &url,
                token.as_deref(),
            )
            .await?
        }
        Commands::Worker => runtime_for("all")?.run_worker().await?,
        Commands::Doctor => app_cmd::cmd_doctor().await?,
        Commands::Validate { app } => cmd_validate(&app)?,
        Commands::Inspect { name, target, app } => cmd_inspect(&app, &name, target.as_deref())?,
    }
    Ok(())
}

pub(crate) fn runtime_for(app: &str) -> Result<QefroRuntime> {
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

fn load_fs_app(name: &str) -> Result<InstalledApp> {
    let bundle = app_cmd::load_named_bundle(name)?;
    app_cmd::installed_from_bundle(bundle)
}

pub(crate) fn known_app_names() -> Vec<String> {
    let mut names: Vec<String> = discover_apps(&builtin_manifests())
        .into_iter()
        .map(|a| a.manifest.name)
        .collect();
    names.extend(["restaurant".into(), "crm".into()]);
    names.sort();
    names.dedup();
    names
}

pub(crate) fn builtin_manifests() -> Vec<AppManifest> {
    vec![
        manifest_of(&qefro_restaurant::installed()),
        manifest_of(&qefro_crm::installed()),
    ]
}

fn manifest_of(app: &InstalledApp) -> AppManifest {
    AppManifest::from_module(&app.module)
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
            "name = \"{name}\"\nversion = \"0.1.0\"\nlabel = \"{name}\"\ndescription = \"\"\napi_version = \"1\"\nframework_version = \">=1.0,<2.0\"\n"
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

pub(crate) fn is_framework_root() -> bool {
    Path::new("Cargo.toml").exists() && Path::new("crates").is_dir()
}

pub(crate) fn write_catalog_stub(root: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(root)?;
    let (version, label, description) = match name {
        "restaurant" => (
            "1.0.0",
            "Restaurant Management",
            "Tables, reservations, menus, orders, and payments",
        ),
        "crm" => (
            "1.0.0",
            "CRM",
            "Leads, contacts, opportunities, and activities",
        ),
        _ => ("0.1.0", name, ""),
    };
    fs::write(
        root.join("app.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\nlabel = \"{label}\"\ndescription = \"{description}\"\napi_version = \"1\"\nframework_version = \">=1.0,<2.0\"\nsource = \"builtin\"\n"
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

fn cmd_inspect(app: &str, name: &str, target: Option<&str>) -> Result<()> {
    if name.eq_ignore_ascii_case("page") {
        let page_name =
            target.ok_or_else(|| anyhow::anyhow!("usage: qefro inspect page <name>"))?;
        return cmd_page_show(app, page_name);
    }
    if name.eq_ignore_ascii_case("automation") {
        let auto_name =
            target.ok_or_else(|| anyhow::anyhow!("usage: qefro inspect automation <name>"))?;
        return cmd_automation_show(app, auto_name);
    }
    let runtime = runtime_for(app)?;
    if runtime
        .automations()
        .iter()
        .any(|a| a.name.eq_ignore_ascii_case(name))
    {
        return cmd_automation_show(app, name);
    }
    if runtime.page(name).is_some() {
        return cmd_page_show(app, name);
    }
    cmd_entity_show(app, name)
}

fn cmd_automation_show(app: &str, name: &str) -> Result<()> {
    let runtime = runtime_for(app)?;
    let Some(def) = runtime
        .automations()
        .into_iter()
        .find(|a| a.name.eq_ignore_ascii_case(name) || a.id_key().eq_ignore_ascii_case(name))
    else {
        let known: Vec<String> = runtime.automations().into_iter().map(|a| a.name).collect();
        let hint = suggest_similar(name, known.iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        bail!("automation '{name}' not found.{hint}");
    };
    println!("name:           {}", def.name);
    println!(
        "status:         {}",
        if def.enabled { "published" } else { "disabled" }
    );
    println!("version:        {}", def.version);
    println!("module:         {}", def.module.clone().unwrap_or_default());
    println!("description:    {}", def.description);
    println!(
        "trigger:        {}",
        def.trigger
            .event
            .clone()
            .or(def.trigger.schedule.clone())
            .unwrap_or_else(|| def.trigger.kind.clone())
    );
    if let Some(cond) = &def.conditions {
        println!(
            "conditions:     {}",
            serde_json::to_string(cond).unwrap_or_default()
        );
    }
    println!("steps:");
    for step in def.effective_steps() {
        println!("  {}  {}", step.kind(), step.label());
    }
    if !def.actions.is_empty() && def.steps.is_empty() {
        println!("actions:");
        for action in &def.actions {
            println!("  {}", action.kind());
        }
    }
    println!("max_depth:      {}", def.depth_limit());
    println!("max_attempts:   {}", def.attempt_limit());
    Ok(())
}

fn cmd_page_show(app: &str, name: &str) -> Result<()> {
    let runtime = runtime_for(app)?;
    let Some(mut page) = runtime.page(name) else {
        let known: Vec<String> = runtime.pages().into_iter().map(|p| p.name).collect();
        let hint = suggest_similar(name, known.iter().map(|s| s.as_str()))
            .map(|s| format!(" Did you mean '{s}'?"))
            .unwrap_or_default();
        bail!("page '{name}' not found.{hint}");
    };
    page.normalize();
    println!("name:           {}", page.name);
    println!("label:          {}", page.label);
    println!("slug:           {}", page.slug());
    println!("route:          {}", page.route());
    println!(
        "module:         {}",
        page.module.clone().unwrap_or_default()
    );
    println!("layout:         {}", page.layout);
    println!(
        "template:       {}",
        page.template.clone().unwrap_or_else(|| "(none)".into())
    );
    println!(
        "permissions:    page roles={}",
        if page.roles.is_empty() {
            "(inherit section)".into()
        } else {
            page.roles.join(",")
        }
    );
    if let Some(entity) = &page.context_entity {
        println!(
            "context:        {} via {}",
            entity,
            page.context_param.as_deref().unwrap_or("id")
        );
    }
    if !page.tabs.is_empty() {
        println!("tabs:");
        for tab in &page.tabs {
            println!("  {}  {}", tab.name, tab.label);
        }
    }
    println!("components:");
    for section in &page.sections {
        println!(
            "  {:<22} {:<12} entity={} view={} report={} widget={}",
            section.title,
            section.resolved_kind(),
            section.entity.clone().unwrap_or_default(),
            section.view.clone().unwrap_or_default(),
            section.report.clone().unwrap_or_default(),
            section
                .widget
                .clone()
                .or(section.card.as_ref().map(|c| c.title.clone()))
                .unwrap_or_default()
        );
        if !section.roles.is_empty() {
            println!("                     roles={}", section.roles.join(","));
        }
    }
    if !page.actions.is_empty() {
        println!("actions:");
        for action in &page.actions {
            println!(
                "  {} {}.{}",
                action.label.clone().unwrap_or_default(),
                action.entity,
                action.action
            );
        }
    }
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
    println!(
        "module:         {}",
        entity.module.clone().unwrap_or_default()
    );
    println!(
        "workflow:       {}",
        entity.workflow.clone().unwrap_or_else(|| "(none)".into())
    );
    println!("display_field:  {}", entity.display_field);
    println!("lifecycle:      archive={}", entity.archives());
    println!(
        "capabilities:   attachments={} activity={} comments={} audit={} workflow={}",
        entity.attachments,
        entity.activity,
        entity.comments,
        entity.audit,
        entity.workflow.is_some()
    );
    if qefro_core::is_commerce_entity(&entity.name) {
        println!(
            "commerce:       Quote → Sales Order → Fulfillment → Invoice → Payment → Return (EntityService operations; no commerce API)"
        );
    }
    println!(
        "row_policy:     {}",
        entity
            .row_policy
            .as_ref()
            .map(|p| format!("{p:?}"))
            .unwrap_or_else(|| "(none)".into())
    );
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
                qefro_core::RelationKind::ChildTable => "child-table",
            });
        }
        println!(
            "  {:<20} {:<12} {}",
            field.name,
            field.field_type.as_str(),
            flags.join(", ")
        );
        if let Some(rel) = &field.relation {
            println!(
                "                     → {}  on_delete={:?}",
                rel.target_entity, rel.on_delete
            );
        }
    }
    let mut any_rules = false;
    for field in &entity.fields {
        if field.system {
            continue;
        }
        let lines = qefro_core::field_rule_lines(field);
        if lines.is_empty() {
            continue;
        }
        if !any_rules {
            println!("Rules");
            any_rules = true;
        }
        println!("  {}", field.name);
        for line in lines {
            println!("    {line}");
        }
    }
    if !entity.validation.is_empty() {
        if !any_rules {
            println!("Rules");
        }
        println!("  Validation");
        for rule in &entity.validation {
            if let Some(line) = qefro_core::compare_rule_line(rule) {
                println!("    {line}");
            } else if !rule.require.is_empty() {
                println!("    require {}", rule.require.join(", "));
            } else if let Some(field) = &rule.field {
                println!("    {field} {:?}", rule.rule);
            }
        }
    }
    if !entity.actions.is_empty() {
        println!("actions:");
        for action in &entity.actions {
            println!("  {}  {}", action.name, action.label);
        }
    }
    let ops: Vec<_> = runtime
        .operation_defs()
        .into_iter()
        .filter(|d| d.entity.eq_ignore_ascii_case(&entity.name))
        .collect();
    if !ops.is_empty() {
        println!("Operations");
        for def in ops {
            println!("  {}", def.label);
        }
    }
    println!("permissions:");
    for grant in runtime.permission_grants() {
        if grant.entity.eq_ignore_ascii_case(&entity.name) {
            let actions: Vec<_> = grant.actions.iter().map(|a| a.as_str()).collect();
            println!("  {}  {}", grant.role, actions.join(","));
        }
    }
    if let Some(wf_name) = &entity.workflow {
        if let Some(wf) = runtime.workflows().into_iter().find(|w| {
            w.name.eq_ignore_ascii_case(wf_name) || w.entity.eq_ignore_ascii_case(&entity.name)
        }) {
            println!("workflow {}:", wf.name);
            for t in &wf.transitions {
                let guard = t.guard.as_ref().map(|_| "  guard").unwrap_or_default();
                println!("  {}  {} → {}{guard}", t.name, t.from, t.to);
            }
        }
    }
    if let Some(views) = &entity.views {
        println!(
            "views:          default={}",
            views.default.clone().unwrap_or_else(|| "list".into())
        );
        if let Some(list) = &views.list {
            if let Some(group) = &list.group_by {
                println!("  list group_by={group}");
            }
        }
        if let Some(kanban) = &views.kanban {
            if let Some(group) = &kanban.group_by {
                println!("  kanban group_by={group}");
            }
        }
    }
    let autos: Vec<_> = runtime
        .automations()
        .into_iter()
        .filter(|a| a.module.as_deref() == entity.module.as_deref())
        .collect();
    if !autos.is_empty() {
        println!("automations:");
        for auto in autos {
            println!("  {}  {:?}", auto.name, auto.trigger.event);
        }
    }
    let reports: Vec<_> = runtime
        .reports()
        .into_iter()
        .filter(|r| r.entity.eq_ignore_ascii_case(&entity.name))
        .collect();
    if !reports.is_empty() {
        println!("reports:");
        for report in reports {
            println!("  {}  {}", report.name, report.label);
        }
    }
    let docs: Vec<_> = runtime
        .print_formats()
        .into_iter()
        .filter(|f| f.entity.eq_ignore_ascii_case(&entity.name))
        .collect();
    if !docs.is_empty() {
        println!("Documents");
        for fmt in docs {
            println!("  {}  {}  {}", fmt.document_title(), fmt.variant, fmt.name);
        }
    }
    if let Some(sched) = &entity.scheduling {
        println!("Scheduling");
        println!("  Start     {}", sched.start_field);
        if let Some(time) = &sched.time_field {
            println!("  Time      {time}");
        }
        if let Some(end) = &sched.end_field {
            println!("  End       {end}");
        }
        if let Some(end_time) = &sched.end_time_field {
            println!("  End time  {end_time}");
        }
        if sched.resources.is_empty() {
            println!("  Resource  (none)");
        } else {
            println!("  Resource  {}", sched.resources.join(", "));
        }
        println!(
            "  Calendar  {}",
            if sched.calendar {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!(
            "  Conflict  {}",
            if sched.conflict {
                "enabled"
            } else {
                "disabled"
            }
        );
        if let Some(mins) = sched.duration_minutes {
            println!("  Duration  {mins} minutes");
        }
    }
    let comms: Vec<_> = runtime
        .communications()
        .into_iter()
        .filter(|c| c.entity.eq_ignore_ascii_case(&entity.name))
        .collect();
    if !comms.is_empty() {
        println!("Communication");
        for def in comms {
            println!("  {}  {}  {}", def.name, def.event, def.channels.join(","));
        }
    }
    Ok(())
}

fn cmd_validate(app: &str) -> Result<()> {
    let runtime = runtime_for(app)?;
    let mut registry = qefro_core::EntityRegistry::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for identity in qefro_core::platform_entities() {
        if let Err(e) = identity.validate_idents() {
            errors.push(format!("{}: {e}", identity.name));
        }
        if let Err(e) = registry.register(identity) {
            errors.push(e.to_string());
        }
    }
    for entity in runtime.entities() {
        if let Err(e) = entity.validate_idents() {
            errors.push(format!("{}: {e}", entity.name));
        }
        match registry.register((*entity).clone()) {
            Ok(()) => {}
            Err(e) => errors.push(format!("{}: {e}", entity.name)),
        }
    }
    if let Err(e) = registry.validate_relations() {
        errors.push(e.to_string());
    }
    for wf in runtime.workflows() {
        if registry.try_get(&wf.entity).is_none() {
            errors.push(format!(
                "workflow '{}' references unknown entity '{}'",
                wf.name, wf.entity
            ));
        }
        match wf.validate() {
            Ok(notes) => warnings.extend(notes),
            Err(e) => errors.push(format!("workflow '{}': {e}", wf.name)),
        }
    }
    for report in runtime.reports() {
        if registry.try_get(&report.entity).is_none() {
            errors.push(format!(
                "report '{}' references unknown entity '{}'",
                report.name, report.entity
            ));
        }
    }
    for dash in runtime.dashboards() {
        for card in &dash.cards {
            if !card.entity.is_empty()
                && !card.entity.starts_with('_')
                && registry.try_get(&card.entity).is_none()
            {
                errors.push(format!(
                    "dashboard '{}' card references unknown entity '{}'",
                    dash.name, card.entity
                ));
            }
        }
    }
    let entity_slugs: Vec<String> = registry
        .list()
        .into_iter()
        .map(|e| e.slug.clone())
        .collect();
    let reports = runtime.reports();
    let dashboards = runtime.dashboards();
    let mut page_slugs = std::collections::HashSet::new();
    for mut page in runtime.pages() {
        page.normalize();
        let slug = page.slug().to_string();
        if !page_slugs.insert(slug.clone()) {
            errors.push(format!("duplicate page route '/pages/{slug}'"));
        }
        for err in qefro_core::validate_page(&page, &registry, &reports, &dashboards, &entity_slugs)
        {
            errors.push(err);
        }
    }
    for fmt in runtime.print_formats() {
        for err in qefro_core::validate_print_format(&fmt, &registry) {
            errors.push(err);
        }
    }
    for def in runtime.communications() {
        for err in qefro_core::validate_communication(&def, &registry) {
            errors.push(err);
        }
    }
    for entity in runtime.entities() {
        for err in qefro_core::validate_scheduling(&entity, Some(&registry)) {
            errors.push(err);
        }
    }
    for auto in runtime.automations() {
        for err in qefro_core::validate_automation(&auto, Some(&registry)) {
            errors.push(err);
        }
    }
    for w in &warnings {
        println!("warning: {w}");
    }
    if errors.is_empty() {
        println!(
            "ok  {} entities  {} workflows",
            runtime.entity_names().len(),
            runtime.workflows().len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("error: {e}");
        }
        bail!("validation failed ({} errors)", errors.len());
    }
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
        let root =
            qefro_core::find_app_root(app).unwrap_or_else(|| PathBuf::from("apps").join(app));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_list_uses_restaurant_defs_not_a_parallel_cli_path() {
        let mut runtime = QefroRuntime::new(Config {
            database_url: "postgres://unused".into(),
            jwt_secret: "test".into(),
            bind: "127.0.0.1:0".into(),
            ..Config::default()
        });
        runtime.install(qefro_restaurant::installed());
        let names: Vec<String> = runtime
            .operation_defs()
            .into_iter()
            .map(|d| format!("{}.{}", d.entity, d.name))
            .collect();
        assert!(names.contains(&"Reservation.confirm".into()));
        assert!(names.contains(&"Reservation.cancel".into()));
        assert!(names.contains(&"Reservation.seat_customer".into()));
        assert!(names.contains(&"Reservation.complete".into()));
    }

    #[test]
    fn validate_restaurant_metadata_graph() {
        cmd_validate("restaurant").expect("restaurant metadata should validate");
    }
}
