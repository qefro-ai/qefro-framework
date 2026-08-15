//! Qefro Framework core: metadata, entities, validation, and shared types.
//!
//! This crate is intentionally free of HTTP and database drivers so it can be
//! used by the CLI, agent layer, and application modules independently of the
//! runtime.

pub mod app;
pub mod catalog;
pub mod context;
pub mod entitlement;
pub mod entity;
pub mod error;
pub mod field;
pub mod hook;
pub mod ident;
pub mod metering;
pub mod operation;
pub mod rate_limit;
pub mod registry;
pub mod storage;
pub mod ui;
pub mod validation;

pub use app::{AppManifest, AppModule, AppModuleBuilder};
pub use catalog::{
    discover_apps, install_app, load_installed, load_yaml_entities, parse_app_toml, remove_app,
    AppFileManifest, DiscoveredApp, InstalledSet,
};
pub use context::{OpContext, ROLE_WORKER};
pub use entitlement::{Entitlements, Plan};
pub use entity::EntityDef;
pub use error::{FieldError, QefroError, QefroResult};
pub use metering::MeteringEvent;
pub use rate_limit::{MemoryRateLimiter, RateLimiter};
pub use storage::{BlobStore, LocalBlobStore};
pub use field::{FieldDef, FieldType, RelationDef, RelationKind};
pub use hook::{EntityHook, HookRegistry, NoopHook};
pub use ident::{quote_ident, slugify, snake_case, suggest_similar, to_plural_slug};
pub use operation::{operation, OperationDef};
pub use registry::EntityRegistry;
pub use ui::{
    DashboardCard, DashboardDef, TenantBranding, TenantBusinessConfig, TenantConfig,
    TenantFeatures, TenantUiConfig, UiEntityMeta, UiFieldMeta, UiWidget,
};
pub use validation::{validate_record, ValidationRules};

pub mod prelude {
    pub use crate::{
        AppModule, AppModuleBuilder, EntityDef, EntityRegistry, FieldDef, FieldType, OpContext,
        QefroError, QefroResult, RelationDef, RelationKind, ValidationRules,
    };
}
