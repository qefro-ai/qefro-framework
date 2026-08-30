use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{
    AppManifest, BlobStore, DashboardDef, Entitlements, MemoryRateLimiter, NotificationDef,
    PrintFormat, ReportDef, StudioCatalog, WebhookDef,
};
use qefro_db::{
    AttachmentStore, AutomationEngine, BlobMetaStore, EntityService, MetadataChangeService,
    NotificationStore, SavedFilterStore, WebhookLog,
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
}

impl AppState {
    pub fn reports_live(&self) -> Vec<ReportDef> {
        self.catalog.merge_reports(&self.reports)
    }

    pub fn dashboards_live(&self) -> Vec<DashboardDef> {
        self.catalog.merge_dashboards(&self.dashboards)
    }

    pub fn print_formats_live(&self) -> Vec<PrintFormat> {
        self.catalog.merge_print_formats(&self.print_formats)
    }
}
