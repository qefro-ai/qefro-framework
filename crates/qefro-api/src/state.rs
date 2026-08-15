use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{AppManifest, BlobStore, DashboardDef, Entitlements, MemoryRateLimiter};
use qefro_db::{BlobMetaStore, EntityService, SavedFilterStore};
use qefro_tenant::TenantService;
use std::sync::Arc;

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
    pub installed_apps: Vec<String>,
    pub default_navigation: Vec<String>,
    pub blob_store: Arc<dyn BlobStore>,
    pub blobs: Arc<BlobMetaStore>,
    pub saved_filters: Arc<SavedFilterStore>,
}
