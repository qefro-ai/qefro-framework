use crate::platform::WebhookDeliverJob;
use crate::realtime::{RealtimeFanout, RealtimeHub};
use crate::routes;
use crate::state::AppState;
use anyhow::Context;
use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{
    AppModule, EntityRegistry, HookRegistry, LocalBlobStore, OperationDef, StudioCatalog,
};
use qefro_db::{
    apply_schema, connect, AttachmentPurgeJob, AttachmentStore, AutomationEngine, BlobMetaStore,
    DueReminderJob, EmailNotifyJob, EntityService, JobHandler, JobQueue, JobRegistry,
    LogNotificationJob, MetadataChangeService, NotificationStore, OperationExecuteJob,
    OperationHandler, OperationRegistry, PlatformDispatcher, SavedFilterStore, WebhookLog,
    ATTACHMENT_PURGE_JOB, DUE_REMINDER_JOB, OPERATION_EXECUTE_JOB,
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

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.env.eq_ignore_ascii_case("production") {
            if self.jwt_secret == "dev-only-change-me" || self.jwt_secret.len() < 16 {
                anyhow::bail!("JWT_SECRET must be a non-default value of at least 16 characters in production");
            }
            if self.database_url.is_empty() {
                anyhow::bail!("DATABASE_URL is required");
            }
        }
        if self.bind.parse::<std::net::SocketAddr>().is_err() {
            anyhow::bail!("invalid QEFRO_BIND '{}'", self.bind);
        }
        Ok(())
    }
}

pub struct InstalledApp {
    pub module: AppModule,
    pub workflows: Vec<WorkflowDef>,
    pub permissions: Vec<PermissionGrant>,
    pub field_levels: Vec<qefro_permissions::FieldLevelGrant>,
    pub operations: Vec<(OperationDef, Arc<dyn OperationHandler>)>,
    pub jobs: Vec<(String, Arc<dyn JobHandler>)>,
}

impl InstalledApp {
    pub fn new(module: AppModule) -> Self {
        Self {
            module,
            workflows: Vec::new(),
            permissions: Vec::new(),
            field_levels: Vec::new(),
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

    pub fn field_level(mut self, grant: qefro_permissions::FieldLevelGrant) -> Self {
        self.field_levels.push(grant);
        self
    }

    pub fn operation(
        mut self,
        def: OperationDef,
        handler: impl OperationHandler + 'static,
    ) -> Self {
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
        let mut names: Vec<String> = qefro_core::platform_entities()
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.extend(
            self.apps
                .iter()
                .flat_map(|a| a.module.entities.iter().map(|e| e.name.clone())),
        );
        names
    }

    pub fn permission_grants(&self) -> Vec<PermissionGrant> {
        let mut grants = qefro_permissions::identity_grants();
        grants.extend(qefro_permissions::task_grants());
        grants.extend(qefro_permissions::accounting_grants());
        grants.extend(qefro_permissions::commerce_grants());
        grants.extend(self.apps.iter().flat_map(|a| a.permissions.clone()));
        grants
    }

    pub fn workflows(&self) -> Vec<WorkflowDef> {
        let mut wfs = vec![
            qefro_workflow::task_workflow(),
            qefro_workflow::journal_workflow(),
            qefro_workflow::period_workflow(),
            qefro_workflow::quote_workflow(),
            qefro_workflow::sales_order_workflow(),
            qefro_workflow::shipment_workflow(),
            qefro_workflow::invoice_workflow(),
            qefro_workflow::sales_payment_workflow(),
            qefro_workflow::sales_return_workflow(),
        ];
        wfs.extend(self.apps.iter().flat_map(|a| a.workflows.clone()));
        wfs
    }

    pub fn automations(&self) -> Vec<qefro_core::AutomationDef> {
        let mut autos = qefro_core::task_automations();
        autos.extend(qefro_core::accounting_automations());
        autos.extend(qefro_core::commerce_automations());
        autos.extend(self.apps.iter().flat_map(|a| a.module.automations.clone()));
        autos
    }

    pub fn reports(&self) -> Vec<qefro_core::ReportDef> {
        let mut reports = qefro_core::accounting_reports();
        reports.extend(qefro_core::commerce_reports());
        reports.extend(self.apps.iter().flat_map(|a| a.module.reports.clone()));
        reports
    }

    pub fn dashboards(&self) -> Vec<qefro_core::DashboardDef> {
        let mut cards: Vec<qefro_core::DashboardDef> = self
            .apps
            .iter()
            .flat_map(|a| a.module.dashboards.clone())
            .collect();
        cards.push(qefro_core::task_dashboard());
        cards.push(qefro_core::accounting_dashboard());
        cards.push(qefro_core::commerce_dashboard());
        cards
    }

    pub fn tool_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut push = |entity: &qefro_core::EntityDef| {
            let snake = qefro_core::ident::snake_case(&entity.name);
            names.push(format!("create_{snake}"));
            names.push(format!("get_{snake}"));
            names.push(format!("find_{}", entity.table));
            names.push(format!("update_{snake}"));
            names.push(format!("delete_{snake}"));
            if entity.workflow.is_some() {
                names.push(format!("transition_{snake}"));
            }
        };
        for entity in qefro_core::platform_entities() {
            push(&entity);
        }
        for entity in self.entities() {
            push(entity);
        }
        for def in qefro_db::accounting_operation_defs() {
            names.push(def.tool_name.clone());
        }
        for def in qefro_db::commerce_operation_defs() {
            names.push(def.tool_name.clone());
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
        for entity in qefro_core::platform_entities() {
            defs.extend(qefro_db::crud_operation_defs(&entity));
        }
        defs.extend(qefro_db::accounting_operation_defs());
        defs.extend(qefro_db::commerce_operation_defs());
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

    pub fn entity(&self, name: &str) -> Option<qefro_core::EntityDef> {
        if let Some(entity) = qefro_core::platform_entities()
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(name) || e.slug.eq_ignore_ascii_case(name))
        {
            return Some(entity);
        }
        self.apps
            .iter()
            .flat_map(|a| a.module.entities.iter())
            .find(|e| e.name.eq_ignore_ascii_case(name) || e.slug.eq_ignore_ascii_case(name))
            .cloned()
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
            "GET/PATCH /api/v1/users/:id".into(),
            "GET/POST /api/v1/people".into(),
            "GET/POST /api/v1/organizations".into(),
            "GET/POST /api/v1/tasks".into(),
            "GET /api/v1/meta/ui".into(),
            "GET /api/v1/meta/dashboards".into(),
            "GET /api/v1/meta/reports".into(),
            "POST /api/v1/reports/:name/run".into(),
            "GET /api/v1/tools".into(),
            "GET /api/v1/operations".into(),
            "GET /api/v1/agent/tools".into(),
            "GET /api/v1/tenant".into(),
            "GET /api/v1/tenants/me/config".into(),
            "GET /api/v1/studio/apps".into(),
            "POST /api/v1/studio/drafts".into(),
            "POST /api/v1/studio/validate".into(),
            "GET /api/v1/studio/publish".into(),
            "GET /api/v1/meta/workspace".into(),
            "GET /api/v1/saved-views".into(),
            "GET /api/v1/search".into(),
            "GET /api/v1/settings/:slug".into(),
            "GET /api/v1/notifications".into(),
            "GET /api/v1/realtime".into(),
        ];
        for app in &self.apps {
            for entity in &app.module.entities {
                routes.push(format!("GET/POST /api/v1/{}", entity.slug));
                routes.push(format!("POST /api/v1/{}/bulk", entity.slug));
                routes.push(format!("GET /api/v1/{}/export", entity.slug));
                routes.push(format!("GET/PATCH/DELETE /api/v1/{}/:id", entity.slug));
                if entity.workflow.is_some() {
                    routes.push(format!("GET /api/v1/{}/:id/workflow", entity.slug));
                    routes.push(format!("POST /api/v1/{}/:id/transition", entity.slug));
                }
                routes.push(format!("GET /api/v1/{}/:id/actions", entity.slug));
                routes.push(format!("POST /api/v1/{}/:id/actions/:name", entity.slug));
                if !entity.print_formats.is_empty() || entity.document.is_some() {
                    routes.push(format!("GET /api/v1/{}/:id/print", entity.slug));
                }
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
        let mut reports = Vec::new();
        let mut print_formats = Vec::new();

        for entity in qefro_core::platform_entities() {
            let name = entity.name.clone();
            registry.register(entity)?;
            permissions.ensure_admin(&name);
        }
        for grant in qefro_permissions::identity_grants() {
            permissions.grant(grant);
        }
        for grant in qefro_permissions::task_grants() {
            permissions.grant(grant);
        }
        for grant in qefro_permissions::accounting_grants() {
            permissions.grant(grant);
        }
        for grant in qefro_permissions::commerce_grants() {
            permissions.grant(grant);
        }
        workflows.register(qefro_workflow::task_workflow());
        workflows.register(qefro_workflow::journal_workflow());
        workflows.register(qefro_workflow::period_workflow());
        workflows.register(qefro_workflow::quote_workflow());
        workflows.register(qefro_workflow::sales_order_workflow());
        workflows.register(qefro_workflow::shipment_workflow());
        workflows.register(qefro_workflow::invoice_workflow());
        workflows.register(qefro_workflow::sales_payment_workflow());
        workflows.register(qefro_workflow::sales_return_workflow());

        for app in &self.apps {
            app.module.install_entities(&mut registry)?;
            for grant in &app.permissions {
                permissions.grant(grant.clone());
            }
            for grant in &app.field_levels {
                permissions.grant_field_level(grant.clone());
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
            manifests.push(qefro_core::AppManifest::from_module(&app.module));
            dashboards.extend(app.module.dashboards.clone());
            reports.extend(app.module.reports.clone());
            print_formats.extend(app.module.print_formats.clone());
            for entity in &app.module.entities {
                print_formats.extend(entity.print_formats.clone());
            }
        }
        dashboards.push(qefro_core::task_dashboard());
        dashboards.push(qefro_core::accounting_dashboard());
        dashboards.push(qefro_core::commerce_dashboard());
        reports.extend(qefro_core::accounting_reports());
        reports.extend(qefro_core::commerce_reports());

        registry.wire_identity_inverses()?;

        registry
            .validate_relations()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if self.config.auto_migrate {
            apply_schema(&pool, &registry)
                .await
                .map_err(|e| anyhow::anyhow!("schema apply failed: {e}"))?;
            for app in &self.apps {
                let status = if qefro_core::load_installed().is_disabled(&app.module.name) {
                    "disabled"
                } else {
                    "installed"
                };
                let _ = qefro_db::app_registry::upsert_app(
                    &pool,
                    &qefro_core::AppManifest::from_module(&app.module),
                    status,
                    None,
                )
                .await;
            }
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
        let catalog = Arc::new(StudioCatalog::default());
        let studio = Arc::new(MetadataChangeService::new(
            pool.clone(),
            registry.clone(),
            workflows.clone(),
            permissions.clone(),
            catalog.clone(),
            self.config.env.clone(),
        ));

        let mut operations = OperationRegistry::new();
        let mut job_handlers = JobRegistry::new();
        let webhook_log = WebhookLog::new(pool.clone());
        let jobs = Arc::new(JobQueue::new(pool.clone()));
        job_handlers.register("notify", Arc::new(LogNotificationJob));
        job_handlers.register("notify.email", Arc::new(EmailNotifyJob));
        let due_reminder = DueReminderJob::new();
        job_handlers.register(DUE_REMINDER_JOB, due_reminder.clone());
        let operation_execute = OperationExecuteJob::new();
        job_handlers.register(OPERATION_EXECUTE_JOB, operation_execute.clone());
        let attachment_purge = AttachmentPurgeJob::new();
        job_handlers.register(ATTACHMENT_PURGE_JOB, attachment_purge.clone());
        job_handlers.register(
            "webhook.deliver",
            Arc::new(WebhookDeliverJob {
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new()),
                log: webhook_log.clone(),
            }),
        );
        for app in &self.apps {
            for (def, handler) in &app.operations {
                operations.register(def.clone(), handler.clone());
            }
            for (name, handler) in &app.jobs {
                job_handlers.register(name.clone(), handler.clone());
            }
        }

        let mut notification_defs = qefro_core::task_notifications();
        notification_defs.extend(qefro_core::accounting_notifications());
        notification_defs.extend(qefro_core::commerce_notifications());
        let mut webhook_defs = Vec::new();
        let mut automation_defs = qefro_core::task_automations();
        automation_defs.extend(qefro_core::accounting_automations());
        automation_defs.extend(qefro_core::commerce_automations());
        for app in &self.apps {
            notification_defs.extend(app.module.notifications.clone());
            webhook_defs.extend(app.module.webhooks.clone());
            automation_defs.extend(app.module.automations.clone());
        }
        let automation = Arc::new(AutomationEngine::new(
            pool.clone(),
            jobs.clone(),
            automation_defs,
            notification_defs.clone(),
            webhook_defs.clone(),
        ));
        job_handlers.register("automation.run", automation.clone());
        job_handlers.register("automation.schedule", automation.clone());
        qefro_db::register_document_operations(&mut operations, &registry);
        qefro_db::register_accounting_operations(&mut operations);
        qefro_db::register_commerce_operations(&mut operations);
        let operations = Arc::new(operations);
        let job_handlers = Arc::new(job_handlers);
        let auth = Arc::new(AuthService::new(
            pool.clone(),
            self.config.jwt_secret.clone(),
        ));

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
            .with_jobs(jobs.clone(), job_handlers.clone())
            .with_identity(auth.clone()),
        );
        automation.bind(entities.clone());
        due_reminder.bind(entities.clone());
        operation_execute.bind(entities.clone());
        entities
            .events()
            .subscribe_async(
                "*",
                Arc::new(PlatformDispatcher::new(
                    pool.clone(),
                    jobs.clone(),
                    notification_defs.clone(),
                    webhook_defs.clone(),
                )),
            )
            .await;
        entities
            .events()
            .subscribe_async("*", automation.clone())
            .await;
        let realtime = Arc::new(RealtimeHub::new());
        entities
            .events()
            .subscribe_async("*", Arc::new(RealtimeFanout(realtime.clone())))
            .await;
        let mut tools = ToolRegistry::from_registry(&registry, &permissions);
        for binding in operations.all() {
            tools.register_operation(&binding.def);
        }
        let tools = Arc::new(tools);
        let tenants = Arc::new(TenantService::new(pool.clone()));
        let installed_set = qefro_core::load_installed();
        let installed_apps: Vec<String> = manifests
            .iter()
            .map(|m| m.name.clone())
            .filter(|n| !installed_set.is_disabled(n))
            .collect();
        let mut default_navigation: Vec<String> = self
            .apps
            .iter()
            .flat_map(|a| a.module.default_nav_slugs())
            .collect();
        default_navigation.push(qefro_core::TASK_SLUG.into());
        default_navigation.push(qefro_core::ACCOUNT_SLUG.into());
        default_navigation.push(qefro_core::JOURNAL_SLUG.into());
        default_navigation.push(qefro_core::PERIOD_SLUG.into());
        default_navigation.push(qefro_core::PRODUCT_SLUG.into());
        default_navigation.push(qefro_core::QUOTE_SLUG.into());
        default_navigation.push(qefro_core::SALES_ORDER_SLUG.into());
        default_navigation.push(qefro_core::SHIPMENT_SLUG.into());
        default_navigation.push(qefro_core::INVOICE_SLUG.into());
        default_navigation.push(qefro_core::SALES_PAYMENT_SLUG.into());
        default_navigation.push(qefro_core::SALES_RETURN_SLUG.into());
        let mut default_nav_items: Vec<qefro_core::WorkspaceNavItem> = self
            .apps
            .iter()
            .flat_map(|a| {
                a.module.navigation.iter().filter_map(|item| {
                    let entity = a
                        .module
                        .entities
                        .iter()
                        .find(|e| e.name == item.entity || e.slug == item.entity)?;
                    Some(qefro_core::WorkspaceNavItem {
                        label: item.label.clone(),
                        entity: entity.name.clone(),
                        slug: entity.slug.clone(),
                        query: item.query.clone(),
                        view: item.view.clone(),
                        module: Some(a.module.name.clone()),
                        section: item.section.clone(),
                    })
                })
            })
            .collect();
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Tasks".into(),
            entity: qefro_core::TASK_ENTITY.into(),
            slug: qefro_core::TASK_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Work".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Accounts".into(),
            entity: qefro_core::ACCOUNT_ENTITY.into(),
            slug: qefro_core::ACCOUNT_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Finance".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Journal Entries".into(),
            entity: qefro_core::JOURNAL_ENTITY.into(),
            slug: qefro_core::JOURNAL_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Finance".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Fiscal Periods".into(),
            entity: qefro_core::PERIOD_ENTITY.into(),
            slug: qefro_core::PERIOD_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Finance".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Products".into(),
            entity: qefro_core::PRODUCT_ENTITY.into(),
            slug: qefro_core::PRODUCT_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Quotes".into(),
            entity: qefro_core::QUOTE_ENTITY.into(),
            slug: qefro_core::QUOTE_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Sales Orders".into(),
            entity: qefro_core::SALES_ORDER_ENTITY.into(),
            slug: qefro_core::SALES_ORDER_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Shipments".into(),
            entity: qefro_core::SHIPMENT_ENTITY.into(),
            slug: qefro_core::SHIPMENT_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Invoices".into(),
            entity: qefro_core::INVOICE_ENTITY.into(),
            slug: qefro_core::INVOICE_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Payments".into(),
            entity: qefro_core::SALES_PAYMENT_ENTITY.into(),
            slug: qefro_core::SALES_PAYMENT_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        default_nav_items.push(qefro_core::WorkspaceNavItem {
            label: "Returns".into(),
            entity: qefro_core::SALES_RETURN_ENTITY.into(),
            slug: qefro_core::SALES_RETURN_SLUG.into(),
            query: None,
            view: None,
            module: None,
            section: Some("Sales".into()),
        });
        let mut default_hidden_entities: Vec<String> = vec![
            qefro_core::PERSON_SLUG.into(),
            qefro_core::ORGANIZATION_SLUG.into(),
            qefro_core::USER_SLUG.into(),
            qefro_core::JOURNAL_LINE_SLUG.into(),
        ];
        default_hidden_entities.extend(
            qefro_core::commerce_child_slugs()
                .into_iter()
                .map(|s| s.to_string()),
        );
        default_hidden_entities.extend(
            self.apps
                .iter()
                .flat_map(|a| a.module.default_hidden_slugs()),
        );
        let blob_store: Arc<dyn qefro_core::BlobStore> =
            Arc::new(LocalBlobStore::new(&self.config.storage_path));
        attachment_purge.bind(blob_store.clone());
        let blobs = Arc::new(BlobMetaStore::new(pool.clone()));
        let saved_filters = Arc::new(SavedFilterStore::new(pool.clone()));
        let notifications = Arc::new(NotificationStore::new(pool.clone()));
        let attachments = Arc::new(AttachmentStore::new(pool));

        let state = AppState {
            entities,
            auth,
            tenants,
            tools,
            modules: manifests,
            dashboards,
            reports,
            print_formats,
            entitlements: qefro_core::Entitlements::new(),
            rate_limiter: Arc::new(qefro_core::MemoryRateLimiter::default()),
            public_limiter: Arc::new(qefro_core::MemoryRateLimiter::new(
                30,
                std::time::Duration::from_secs(60),
            )),
            search_limiter: Arc::new(qefro_core::MemoryRateLimiter::new(
                60,
                std::time::Duration::from_secs(60),
            )),
            login_limiter: Arc::new(qefro_core::MemoryRateLimiter::new(
                20,
                std::time::Duration::from_secs(60),
            )),
            installed_apps,
            default_navigation,
            default_nav_items,
            default_hidden_entities,
            blob_store,
            blobs,
            saved_filters,
            env: self.config.env.clone(),
            catalog,
            studio,
            realtime,
            notifications,
            webhook_log: Arc::new(webhook_log),
            attachments,
            notification_defs,
            webhooks: webhook_defs,
            automation,
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let router = routes::router(state.clone())
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            .layer(axum::middleware::from_fn(crate::metrics::track))
            .layer(axum::extract::DefaultBodyLimit::max(12 * 1024 * 1024));

        Ok((router, state))
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        self.config.validate()?;
        let bind: SocketAddr = self.config.bind.parse().context("invalid QEFRO_BIND")?;
        let config_bind = self.config.bind.clone();
        let embed_worker = self.config.embed_worker;
        let (router, state) = self.build().await?;
        if embed_worker {
            let jobs = state.entities.job_queue();
            let handlers = state.entities.job_handlers();
            let entities = state.entities.clone();
            let automation = state.automation.clone();
            let _ = jobs.reclaim_running().await;
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    if let Err(err) = automation.enqueue_scheduled().await {
                        tracing::warn!(error = %err, "automation scheduler");
                    }
                    match jobs.process_one(&handlers).await {
                        Ok(true) => {}
                        Ok(false) => {}
                        Err(err) => tracing::warn!(error = %err, "job worker"),
                    }
                    if let Err(err) = entities.dispatch_outbox().await {
                        tracing::warn!(error = %err, "outbox dispatch");
                    }
                }
            });
        }
        tracing::info!(%config_bind, embed_worker, "qefro listening");
        let listener = tokio::net::TcpListener::bind(bind).await?;
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }

    pub async fn run_worker(self) -> anyhow::Result<()> {
        self.config.validate()?;
        let (_router, state) = self.build().await?;
        let jobs = state.entities.job_queue();
        let handlers = state.entities.job_handlers();
        let entities = state.entities.clone();
        let automation = state.automation.clone();
        let reclaimed = jobs.reclaim_running().await.unwrap_or(0);
        tracing::info!(reclaimed, "qefro worker polling jobs");
        loop {
            tokio::select! {
                _ = shutdown_signal() => {
                    tracing::info!("worker shutting down; in-flight job will finish");
                    break;
                }
                _ = async {
                    if let Err(err) = automation.enqueue_scheduled().await {
                        tracing::warn!(error = %err, "automation scheduler");
                    }
                    match jobs.process_one(&handlers).await {
                        Ok(true) => {}
                        Ok(false) => tokio::time::sleep(Duration::from_secs(2)).await,
                        Err(err) => {
                            tracing::warn!(error = %err, "job worker");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                    if let Err(err) = entities.dispatch_outbox().await {
                        tracing::warn!(error = %err, "outbox dispatch");
                    }
                } => {}
            }
        }
        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
