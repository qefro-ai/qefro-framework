use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{
    AppManifest, BlobStore, DashboardDef, Entitlements, MemoryRateLimiter, NotificationDef,
    OpContext, PageDef, PrintFormat, ReportDef, StudioCatalog, TenantBranding, TenantConfig,
    WebhookDef,
};
use qefro_db::{
    AttachmentStore, AutomationEngine, BlobMetaStore, CommunicationHub, CommunicationStore,
    EntityService, MetadataChangeService, NotificationStore, SavedFilterStore, WebhookLog,
};
use qefro_tenant::TenantService;
use std::sync::Arc;

use crate::realtime::RealtimeHub;

#[derive(Clone)]
pub struct AppState {
    pub entities: Arc<EntityService>,
    pub auth: Arc<AuthService>,
    pub tenants: Arc<TenantService>,
    pub tools: Arc<ToolRegistry>,
    pub modules: Vec<AppManifest>,
    pub dashboards: Vec<DashboardDef>,
    pub pages: Vec<PageDef>,
    pub reports: Vec<qefro_core::ReportDef>,
    pub print_formats: Vec<qefro_core::PrintFormat>,
    pub entitlements: Entitlements,
    pub rate_limiter: Arc<MemoryRateLimiter>,
    pub public_limiter: Arc<MemoryRateLimiter>,
    pub search_limiter: Arc<MemoryRateLimiter>,
    pub login_limiter: Arc<MemoryRateLimiter>,
    pub installed_apps: Vec<String>,
    pub default_navigation: Vec<String>,
    pub default_nav_items: Vec<qefro_core::WorkspaceNavItem>,
    pub default_hidden_entities: Vec<String>,
    pub blob_store: Arc<dyn BlobStore>,
    pub blobs: Arc<BlobMetaStore>,
    pub saved_filters: Arc<SavedFilterStore>,
    pub env: String,
    pub catalog: Arc<StudioCatalog>,
    pub studio: Arc<MetadataChangeService>,
    pub realtime: Arc<RealtimeHub>,
    pub notifications: Arc<NotificationStore>,
    pub webhook_log: Arc<WebhookLog>,
    pub attachments: Arc<AttachmentStore>,
    pub notification_defs: Vec<NotificationDef>,
    pub webhooks: Vec<WebhookDef>,
    pub automation: Arc<AutomationEngine>,
    pub communications: Arc<CommunicationStore>,
    pub communication_defs: Vec<qefro_core::CommunicationDef>,
    pub communication_hub: Arc<CommunicationHub>,
}

impl AppState {
    pub fn reports_live(&self) -> Vec<ReportDef> {
        self.catalog.merge_reports(&self.reports)
    }

    pub fn dashboards_live(&self) -> Vec<DashboardDef> {
        self.catalog.merge_dashboards(&self.dashboards)
    }

    pub fn pages_live(&self) -> Vec<PageDef> {
        self.catalog.merge_pages(&self.pages)
    }

    /// Application dashboards win over platform ones (e.g. Task) when the tenant
    /// has not set `default_dashboard`.
    pub fn default_dashboard_name(&self, ctx: &OpContext) -> Option<String> {
        let live = self.dashboards_live();
        live.iter()
            .find(|d| d.module.is_some() && ctx.allows_app(d.module.as_deref()))
            .or_else(|| live.iter().find(|d| ctx.allows_app(d.module.as_deref())))
            .map(|d| d.name.clone())
    }

    pub fn print_formats_live(&self) -> Vec<PrintFormat> {
        self.catalog.merge_print_formats(&self.print_formats)
    }

    pub fn communications_live(&self) -> Vec<qefro_core::CommunicationDef> {
        self.catalog.merge_communications(&self.communication_defs)
    }

    /// Tenant branding wins; empty fields take enabled-app defaults, then tenant name.
    pub fn resolve_branding(
        &self,
        ctx: &OpContext,
        config: &TenantConfig,
        tenant_name: Option<&str>,
    ) -> TenantBranding {
        let defaults = self.modules.iter().filter_map(|module| {
            if ctx.allows_app(Some(module.name.as_str())) && !module.branding.is_empty() {
                Some(module.branding.clone())
            } else {
                None
            }
        });
        TenantBranding::resolve(&config.branding, defaults, tenant_name)
    }
}
