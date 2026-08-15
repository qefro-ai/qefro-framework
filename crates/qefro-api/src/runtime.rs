use crate::routes;
use crate::state::AppState;
use anyhow::Context;
use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{AppManifest, AppModule, EntityRegistry, HookRegistry, OperationDef};
use qefro_db::{
    apply_schema, connect, EntityService, JobHandler, JobQueue, JobRegistry, LogNotificationJob,
    OperationHandler, OperationRegistry,
};
use qefro_events::InProcessEventBus;
use qefro_permissions::{PermissionGrant, PermissionRegistry};
use qefro_tenant::TenantService;
use qefro_workflow::{WorkflowDef, WorkflowRegistry};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub bind: String,
    pub env: String,
    pub public_url: String,
    pub log_level: String,
    pub storage_path: String,
    pub auto_migrate: bool,
    /// When true, the HTTP process also polls jobs. Production compose runs
    /// `qefro worker` separately and sets this false.
    pub embed_worker: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgres://qefro:qefro@127.0.0.1:5432/qefro".into(),
            jwt_secret: "dev-only-change-me".into(),
            bind: "127.0.0.1:8080".into(),
            env: "development".into(),
            public_url: "http://127.0.0.1:8080".into(),
            log_level: "info".into(),
            storage_path: "./var/qefro-storage".into(),
            auto_migrate: true,
            embed_worker: true,
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let env = std::env::var("QEFRO_ENV").unwrap_or_else(|_| "development".into());
        let auto_migrate = match std::env::var("QEFRO_AUTO_MIGRATE") {
            Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"),
            Err(_) => env != "production",
        };
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://qefro:qefro@127.0.0.1:5432/qefro".into()),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-only-change-me".into()),
            bind: std::env::var("QEFRO_BIND")
                .or_else(|_| std::env::var("QEFRO_BIND_ADDRESS"))
                .unwrap_or_else(|_| "127.0.0.1:8080".into()),
            public_url: std::env::var("QEFRO_PUBLIC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            log_level: std::env::var("QEFRO_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            storage_path: std::env::var("QEFRO_STORAGE_PATH")
                .unwrap_or_else(|_| "./var/qefro-storage".into()),
            auto_migrate,
            embed_worker: match std::env::var("QEFRO_EMBED_WORKER") {
                Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"),
                Err(_) => env != "production",
            },
            env,
        })
    }
}

pub struct InstalledApp {
    pub module: AppModule,
    pub workflows: Vec<WorkflowDef>,
    pub permissions: Vec<PermissionGrant>,
    pub operations: Vec<(OperationDef, Arc<dyn OperationHandler>)>,
    pub jobs: Vec<(String, Arc<dyn JobHandler>)>,
}

impl InstalledApp {
    pub fn new(module: AppModule) -> Self {
        Self {
            module,
            workflows: Vec::new(),
            permissions: Vec::new(),
            operations: Vec::new(),
            jobs: Vec::new(),
        }
    }

    pub fn workflow(mut self, wf: WorkflowDef) -> Self {
        self.workflows.push(wf);
        self
    }

    pub fn permission(mut self, grant: PermissionGrant) -> Self {
        self.permissions.push(grant);
        self
    }

    pub fn operation(mut self, def: OperationDef, handler: impl OperationHandler + 'static) -> Self {
        self.operations.push((def, Arc::new(handler)));
        self
    }

    pub fn job(mut self, name: impl Into<String>, handler: impl JobHandler + 'static) -> Self {
        self.jobs.push((name.into(), Arc::new(handler)));
        self
    }
}

pub struct QefroRuntime {
    config: Config,
    apps: Vec<InstalledApp>,
}

impl QefroRuntime {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            apps: Vec::new(),
        }
    }

    pub fn install(&mut self, app: InstalledApp) -> &mut Self {
        self.apps.push(app);
        self
    }

    /// `qefro migrate` always applies schema, including production.
    pub fn with_auto_migrate(mut self, auto_migrate: bool) -> Self {
        self.config.auto_migrate = auto_migrate;
        self
    }

    pub fn entity_names(&self) -> Vec<String> {
        self.apps
            .iter()
            .flat_map(|a| a.module.entities.iter().map(|e| e.name.clone()))
            .collect()
    }

    pub fn permission_grants(&self) -> Vec<PermissionGrant> {
        self.apps
            .iter()
            .flat_map(|a| a.permissions.clone())
            .collect()
    }

    pub fn workflows(&self) -> Vec<WorkflowDef> {
        self.apps.iter().flat_map(|a| a.workflows.clone()).collect()
    }

    pub fn tool_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for entity in self.entities() {
            let snake = qefro_core::ident::snake_case(&entity.name);
            names.push(format!("create_{snake}"));
            names.push(format!("get_{snake}"));
            names.push(format!("find_{}", entity.table));
            names.push(format!("update_{snake}"));
            names.push(format!("delete_{snake}"));
            if entity.workflow.is_some() {
                names.push(format!("transition_{snake}"));
            }
        }
        for app in &self.apps {
            for (def, _) in &app.operations {
                names.push(def.tool_name.clone());
            }
        }
        names.sort();
        names
    }

    pub fn operation_defs(&self) -> Vec<OperationDef> {
        let mut defs = Vec::new();
        for entity in self.entities() {
            defs.extend(qefro_db::crud_operation_defs(entity));
        }
        for app in &self.apps {
            for (def, _) in &app.operations {
                defs.push(def.clone());
            }
        }
        defs
    }

    pub fn installed_apps(&self) -> Vec<String> {
        self.apps.iter().map(|a| a.module.name.clone()).collect()
    }

    pub fn entities(&self) -> Vec<&qefro_core::EntityDef> {
        self.apps
            .iter()
            .flat_map(|a| a.module.entities.iter())
            .collect()
    }

    pub fn entity(&self, name: &str) -> Option<&qefro_core::EntityDef> {
        self.entities().into_iter().find(|e| {
            e.name.eq_ignore_ascii_case(name) || e.slug.eq_ignore_ascii_case(name)
        })
    }

    pub fn routes_summary(&self) -> Vec<String> {
        let mut routes = vec![
            "GET /health".into(),
            "GET /ready".into(),
            "POST /api/v1/auth/register".into(),
            "POST /api/v1/auth/login".into(),
            "POST /api/v1/auth/logout".into(),
            "GET /api/v1/auth/me".into(),
            "POST /api/v1/users".into(),
            "GET /api/v1/meta/ui".into(),
            "GET /api/v1/meta/dashboards".into(),
            "GET /api/v1/tools".into(),
            "GET /api/v1/operations".into(),
            "GET /api/v1/agent/tools".into(),
            "GET /api/v1/tenant".into(),
            "GET /api/v1/tenants/me/config".into(),
            "GET /docs".into(),
        ];
        for app in &self.apps {
            for entity in &app.module.entities {
                routes.push(format!("GET/POST /api/v1/{}", entity.slug));
                routes.push(format!("GET/PATCH/DELETE /api/v1/{}/:id", entity.slug));
                if entity.workflow.is_some() {
                    routes.push(format!("GET /api/v1/{}/:id/workflow", entity.slug));
                    routes.push(format!("POST /api/v1/{}/:id/transition", entity.slug));
                }
                routes.push(format!("GET /api/v1/{}/:id/actions", entity.slug));
                routes.push(format!("POST /api/v1/{}/:id/actions/:name", entity.slug));
            }
        }
        routes
    }

    pub async fn build(self) -> anyhow::Result<(axum::Router, AppState)> {
        let pool = connect(&self.config.database_url)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let mut registry = EntityRegistry::new();
        let mut permissions = PermissionRegistry::new();
        let mut workflows = WorkflowRegistry::new();
        let mut hooks = HookRegistry::new();
        let mut manifests = Vec::new();
        let mut dashboards = Vec::new();

        for app in &self.apps {
            app.module.install_entities(&mut registry)?;
            for grant in &app.permissions {
                permissions.grant(grant.clone());
            }
            for wf in &app.workflows {
                workflows.register(wf.clone());
            }
            for entity in &app.module.entities {
                permissions.ensure_admin(&entity.name);
                for hook in app.module.hooks.for_entity(&entity.name) {
                    hooks.register(hook.clone());
                }
            }
            manifests.push(AppManifest {
                name: app.module.name.clone(),
                version: app.module.version.clone(),
                label: app.module.label.clone(),
                description: app.module.description.clone(),
                depends_on: vec!["qefro-framework".into()],
            });
            dashboards.extend(app.module.dashboards.clone());
        }

        registry
            .validate_relations()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if self.config.auto_migrate {
            apply_schema(&pool, &registry)
                .await
                .map_err(|e| anyhow::anyhow!("schema apply failed: {e}"))?;
        } else {
            qefro_db::pool::ping(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("database not ready (run `qefro migrate`): {e}"))?;
        }

        let registry = Arc::new(registry);
        let permissions = Arc::new(permissions);
        let workflows = Arc::new(workflows);
        let hooks = Arc::new(hooks);
        let events = InProcessEventBus::new();

        let mut operations = OperationRegistry::new();
        let mut job_handlers = JobRegistry::new();
        job_handlers.register("notify", Arc::new(LogNotificationJob));
        for app in &self.apps {
            for (def, handler) in &app.operations {
                operations.register(def.clone(), handler.clone());
            }
            for (name, handler) in &app.jobs {
                job_handlers.register(name.clone(), handler.clone());
            }
        }
        let operations = Arc::new(operations);
        let job_handlers = Arc::new(job_handlers);
        let jobs = Arc::new(JobQueue::new(pool.clone()));

        let entities = Arc::new(
            EntityService::new(
                pool.clone(),
                registry.clone(),
                permissions.clone(),
                workflows,
                hooks,
                events,
            )
            .with_operations(operations.clone())
            .with_jobs(jobs, job_handlers),
        );
        let mut tools = ToolRegistry::from_registry(&registry, &permissions);
        for binding in operations.all() {
            tools.register_operation(&binding.def);
        }
        let tools = Arc::new(tools);
        let auth = Arc::new(AuthService::new(
            pool.clone(),
            self.config.jwt_secret.clone(),
        ));
        let tenants = Arc::new(TenantService::new(pool));
        let installed_apps: Vec<String> = manifests.iter().map(|m| m.name.clone()).collect();

        let state = AppState {
            entities,
            auth,
            tenants,
            tools,
            modules: manifests,
            dashboards,
            entitlements: qefro_core::Entitlements::new(),
            rate_limiter: Arc::new(qefro_core::MemoryRateLimiter::default()),
            installed_apps,
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let router = routes::router(state.clone())
            .layer(cors)
            .layer(TraceLayer::new_for_http());

        Ok((router, state))
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let bind: SocketAddr = self.config.bind.parse().context("invalid QEFRO_BIND")?;
        let config_bind = self.config.bind.clone();
        let embed_worker = self.config.embed_worker;
        let (router, state) = self.build().await?;
        if embed_worker {
            let jobs = state.entities.job_queue();
            let handlers = state.entities.job_handlers();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    match jobs.process_one(&handlers).await {
                        Ok(true) => {}
                        Ok(false) => {}
                        Err(err) => tracing::warn!(error = %err, "job worker"),
                    }
                }
            });
        }
        tracing::info!(%config_bind, embed_worker, "qefro listening");
        let listener = tokio::net::TcpListener::bind(bind).await?;
        axum::serve(listener, router).await?;
        Ok(())
    }

    pub async fn run_worker(self) -> anyhow::Result<()> {
        let (_router, state) = self.build().await?;
        let jobs = state.entities.job_queue();
        let handlers = state.entities.job_handlers();
        tracing::info!("qefro worker polling jobs");
        loop {
            match jobs.process_one(&handlers).await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(Duration::from_secs(2)).await,
                Err(err) => {
                    tracing::warn!(error = %err, "job worker");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}
