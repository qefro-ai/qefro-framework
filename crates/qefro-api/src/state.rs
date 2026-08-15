use qefro_agent::ToolRegistry;
use qefro_auth::AuthService;
use qefro_core::{AppManifest, DashboardDef, Entitlements, MemoryRateLimiter};
use qefro_db::EntityService;
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
    pub entitlements: Entitlements,
    pub rate_limiter: Arc<MemoryRateLimiter>,
    pub installed_apps: Vec<String>,
}
