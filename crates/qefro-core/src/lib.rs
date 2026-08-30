//! Qefro Framework core: metadata, entities, validation, and shared types.
//!
//! This crate is intentionally free of HTTP and database drivers so it can be
//! used by the CLI, agent layer, and application modules independently of the
//! runtime.

pub mod app;
pub mod automation;
pub mod bundle;
pub mod catalog;
pub mod condition;
pub mod context;
pub mod document;
pub mod entitlement;
pub mod entity;
pub mod error;
pub mod field;
pub mod formula;
pub mod hook;
pub mod ident;
pub mod identity;
pub mod lifecycle;
pub mod metering;
pub mod migration;
pub mod operation;
pub mod package;
pub mod platform;
pub mod rate_limit;
pub mod registry;
pub mod sanitize;
pub mod schedule;
pub mod seed;
pub mod storage;
pub mod studio;
pub mod task;
pub mod timezone;
pub mod ui;
pub mod validate;
pub mod validation;
pub mod version;

pub use app::{AppManifest, AppModule, AppModuleBuilder, NavItem};
pub use automation::{
    ActivityAction, AssignAction, AutomationAction, AutomationDef, AutomationTrigger,
    CommentAction, CreateEntityAction, NotifyAction, TransitionAction, UpdateEntityAction,
    WebhookAction,
};
pub use bundle::AppBundle;
pub use catalog::{
    app_root_candidates, disable_app, discover_apps, enable_app, find_app_root, install_app,
    load_installed, load_yaml_docs, load_yaml_entities, mark_installed, parse_app_toml, remove_app,
    store_dir, AppFileManifest, DiscoveredApp, InstalledRecord, InstalledSet,
};
pub use condition::Condition;
pub use context::{OpContext, ROLE_PUBLIC, ROLE_WORKER};
pub use document::{DocumentConfig, NamingConfig, PrintFormat, ReportDef};
pub use entitlement::{Entitlements, Plan};
pub use entity::{EntityDef, RecordLifecycle, RowPolicy};
pub use error::{FieldError, QefroError, QefroResult};
pub use field::{ChildTableDef, FieldDef, FieldType, OnDelete, RelationDef, RelationKind};
pub use formula::{
    apply_computed_fields, detect_cycles, eval_formula, eval_value, parse_formula, FormulaContext,
    FormulaValue,
};
pub use hook::{EntityHook, HookRegistry, NoopHook};
pub use ident::{quote_ident, slugify, snake_case, suggest_similar, to_plural_slug};
pub use identity::{
    apply_organization_backrefs, apply_party_fields, apply_person_backrefs, contains_secret_key,
    field_changes, identity_entities, is_organization_link_field, is_person_link_field,
    is_secret_key, organization_backref_field, organization_backref_name, organization_backrefs,
    organization_entity, person_backref_field, person_backref_name, person_backrefs, person_entity,
    strip_secrets, user_entity, validate_party, ORGANIZATION_ENTITY, ORGANIZATION_LINK_FIELD,
    ORGANIZATION_SLUG, PARTY_TYPE_FIELD, PARTY_TYPE_ORGANIZATION, PARTY_TYPE_PERSON, PERSON_ENTITY,
    PERSON_LINK_FIELD, PERSON_SLUG, SECRET_KEYS, USER_ENTITY, USER_SLUG,
};
pub use lifecycle::{lifecycle_event_name, LifecycleHookDef};
pub use metering::MeteringEvent;
pub use migration::{sql_is_destructive, AppMigration};
pub use operation::{operation, OperationDef};
pub use package::{extract_package, inspect_package, write_package, PackageMeta};
pub use platform::{
    webhook_secret, webhook_signature, ConfirmationDef, EntityActionDef, LinkDef, LinkFilter,
    NotificationDef, PublicFormDef, WebhookDef,
};
pub use rate_limit::{MemoryRateLimiter, RateLimiter};
pub use registry::EntityRegistry;
pub use sanitize::sanitize_html;
pub use schedule::{next_run_after, parse_cron, parse_timezone, schedule_slot_key, CronExpr};
pub use seed::SeedBatch;
pub use storage::{BlobStore, LocalBlobStore};
pub use studio::{
    capabilities as studio_capabilities, classify_entity_change, entity_referrers, preview_formula,
    require_cap as require_studio_cap, ChangeAnalysis, FieldUiPatch, SchemaImpact, StudioCatalog,
    CAP_EDIT, CAP_MANAGE_APPS, CAP_MANAGE_PERMISSIONS, CAP_MANAGE_WORKFLOWS, CAP_PUBLISH, CAP_VIEW,
    FORMULA_FUNCTIONS,
};
pub use task::{
    apply_task_link, platform_entities, task_automations, task_dashboard, task_entity,
    task_nav_item, task_notifications, task_priorities, task_statuses, RELATED_ID_FIELD,
    RELATED_TYPE_FIELD, STATUS_CANCELLED, STATUS_COMPLETED, STATUS_IN_PROGRESS, STATUS_OPEN,
    TASK_ENTITY, TASK_SLUG, TASK_WORKFLOW,
};
pub use timezone::{canonicalize_datetime, local_to_utc, utc_to_local};
pub use ui::{
    CalendarViewSpec, CardViewSpec, ChartMeasureSpec, ChartViewSpec, DashboardCard, DashboardDef,
    DetailViewSpec, EntityCapabilities, EntityPermissions, EntityViews, FormViewSpec,
    KanbanCardSpec, KanbanViewSpec, ListViewSpec, TenantBranding, TenantBusinessConfig,
    TenantConfig, TenantFeatures, TenantUiConfig, UiConfig, UiEntityMeta, UiFieldMeta, UiWhen,
    UiWidget, ViewColumnSpec, ViewSectionSpec, WidgetOptions, WorkspaceNavItem, UI_SCHEMA_VERSION,
};
pub use validate::{
    destructive_field_removals, validate_bundle, InstalledAppRef, ValidationReport,
};
pub use validation::{
    apply_entity_rules, apply_field_rules, compare_rule_line, existence_rules, field_is_readonly,
    field_rule_lines, reject_readonly_writes, strip_computed_fields, validate_record,
    CompareClause, ValidationRule, ValidationRules, WhenClause,
};
pub use version::{
    is_framework_dep, API_VERSION, APP_API_VERSION, FRAMEWORK_COMPAT_REQ, FRAMEWORK_VERSION,
    METADATA_SCHEMA_VERSION, MIGRATION_FORMAT_VERSION,
};

pub mod prelude {
    pub use crate::{
        AppModule, AppModuleBuilder, AutomationDef, ChildTableDef, DocumentConfig, EntityActionDef,
        EntityDef, EntityRegistry, FieldDef, FieldType, LinkDef, NamingConfig, NotificationDef,
        OnDelete, OpContext, PrintFormat, PublicFormDef, QefroError, QefroResult, RecordLifecycle,
        RelationDef, RelationKind, ReportDef, RowPolicy, UiConfig, ValidationRule, ValidationRules,
        WebhookDef,
    };
}
