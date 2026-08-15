//! Qefro Framework core: metadata, entities, validation, and shared types.
//!
//! This crate is intentionally free of HTTP and database drivers so it can be
//! used by the CLI, agent layer, and application modules independently of the
//! runtime.

pub mod app;
pub mod bundle;
pub mod catalog;
pub mod context;
pub mod document;
pub mod entitlement;
pub mod entity;
pub mod error;
pub mod field;
pub mod formula;
pub mod hook;
pub mod ident;
pub mod lifecycle;
pub mod metering;
pub mod migration;
pub mod operation;
pub mod package;
pub mod platform;
pub mod rate_limit;
pub mod registry;
pub mod sanitize;
pub mod seed;
pub mod storage;
pub mod studio;
pub mod timezone;
pub mod ui;
pub mod validate;
pub mod validation;
pub mod version;

pub use app::{AppManifest, AppModule, AppModuleBuilder, NavItem};
pub use bundle::AppBundle;
pub use catalog::{
    app_root_candidates, disable_app, discover_apps, enable_app, find_app_root, install_app,
    load_installed, load_yaml_docs, load_yaml_entities, mark_installed, parse_app_toml, remove_app,
    store_dir, AppFileManifest, DiscoveredApp, InstalledRecord, InstalledSet,
};
pub use context::{OpContext, ROLE_PUBLIC, ROLE_WORKER};
pub use document::{DocumentConfig, NamingConfig, PrintFormat, ReportDef};
pub use entitlement::{Entitlements, Plan};
pub use entity::EntityDef;
pub use error::{FieldError, QefroError, QefroResult};
pub use metering::MeteringEvent;
pub use rate_limit::{MemoryRateLimiter, RateLimiter};
pub use storage::{BlobStore, LocalBlobStore};
pub use field::{ChildTableDef, FieldDef, FieldType, RelationDef, RelationKind};
pub use formula::{
    apply_computed_fields, detect_cycles, eval_formula, parse_formula, FormulaContext,
};
pub use hook::{EntityHook, HookRegistry, NoopHook};
pub use ident::{quote_ident, slugify, snake_case, suggest_similar, to_plural_slug};
pub use lifecycle::{lifecycle_event_name, LifecycleHookDef};
pub use migration::{sql_is_destructive, AppMigration};
pub use operation::{operation, OperationDef};
pub use package::{extract_package, inspect_package, write_package, PackageMeta};
pub use platform::{
    webhook_secret, webhook_signature, ConfirmationDef, EntityActionDef, LinkDef, NotificationDef,
    PublicFormDef, WebhookDef,
};
pub use registry::EntityRegistry;
pub use sanitize::sanitize_html;
pub use studio::{
    capabilities as studio_capabilities, classify_entity_change, entity_referrers, preview_formula,
    require_cap as require_studio_cap, ChangeAnalysis, FieldUiPatch, SchemaImpact, StudioCatalog,
    CAP_EDIT, CAP_MANAGE_APPS, CAP_MANAGE_PERMISSIONS, CAP_MANAGE_WORKFLOWS, CAP_PUBLISH, CAP_VIEW,
    FORMULA_FUNCTIONS,
};
pub use seed::SeedBatch;
pub use timezone::{canonicalize_datetime, local_to_utc, utc_to_local};
pub use ui::{
    DashboardCard, DashboardDef, TenantBranding, TenantBusinessConfig, TenantConfig,
    TenantFeatures, TenantUiConfig, UiConfig, UiEntityMeta, UiFieldMeta, UiWhen, UiWidget,
    WidgetOptions, UI_SCHEMA_VERSION,
};
pub use validate::{destructive_field_removals, validate_bundle, InstalledAppRef, ValidationReport};
pub use validation::{validate_record, ValidationRules};
pub use version::{
    is_framework_dep, API_VERSION, APP_API_VERSION, FRAMEWORK_COMPAT_REQ, FRAMEWORK_VERSION,
    METADATA_SCHEMA_VERSION, MIGRATION_FORMAT_VERSION,
};

pub mod prelude {
    pub use crate::{
        AppModule, AppModuleBuilder, ChildTableDef, DocumentConfig, EntityActionDef, EntityDef,
        EntityRegistry, FieldDef, FieldType, LinkDef, NamingConfig, NotificationDef, OpContext,
        PrintFormat, PublicFormDef, QefroError, QefroResult, RelationDef, RelationKind, ReportDef,
        UiConfig, ValidationRules, WebhookDef,
    };
}
