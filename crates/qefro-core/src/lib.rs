//! Qefro Framework core: metadata, entities, validation, and shared types.
//!
//! This crate is intentionally free of HTTP and database drivers so it can be
//! used by the CLI, agent layer, and application modules independently of the
//! runtime.

pub mod accounting;
pub mod app;
pub mod automation;
pub mod bundle;
pub mod catalog;
pub mod commerce;
pub mod communication;
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
pub mod money;
pub mod operation;
pub mod package;
pub mod page;
pub mod platform;
pub mod rate_limit;
pub mod registry;
pub mod sanitize;
pub mod schedule;
pub mod scheduling;
pub mod seed;
pub mod storage;
pub mod studio;
pub mod task;
pub mod template;
pub mod timezone;
pub mod ui;
pub mod validate;
pub mod validation;
pub mod version;

pub use accounting::{
    account_entity, accounting_automations, accounting_dashboard, accounting_entities,
    accounting_nav_items, accounting_notifications, accounting_reports, fiscal_period_entity,
    journal_entry_entity, journal_line_entity, tenant_account_code, LedgerPosting, ACCOUNT_ENTITY,
    ACCOUNT_KEY_CASH, ACCOUNT_KEY_COGS, ACCOUNT_KEY_INVENTORY, ACCOUNT_KEY_PAYABLE,
    ACCOUNT_KEY_RECEIVABLE, ACCOUNT_KEY_SALES, ACCOUNT_SLUG, ACCOUNT_TYPE_ASSET,
    ACCOUNT_TYPE_EQUITY, ACCOUNT_TYPE_EXPENSE, ACCOUNT_TYPE_LIABILITY, ACCOUNT_TYPE_REVENUE,
    JOURNAL_DRAFT, JOURNAL_ENTITY, JOURNAL_LINE_ENTITY, JOURNAL_LINE_SLUG, JOURNAL_POSTED,
    JOURNAL_REVERSED, JOURNAL_SLUG, JOURNAL_WORKFLOW, PERIOD_CLOSED, PERIOD_ENTITY, PERIOD_OPEN,
    PERIOD_SLUG, PERIOD_WORKFLOW,
};
pub use app::{AppManifest, AppModule, AppModuleBuilder, NavItem};
pub use automation::{
    ActivityAction, AssignAction, AutomationAction, AutomationDef, AutomationTrigger,
    CommentAction, CommunicationAction, CreateEntityAction, NotifyAction, TransitionAction,
    UpdateEntityAction, WebhookAction,
};
pub use bundle::AppBundle;
pub use catalog::{
    app_root_candidates, disable_app, discover_apps, enable_app, find_app_root, install_app,
    load_installed, load_yaml_docs, load_yaml_entities, mark_installed, parse_app_toml, remove_app,
    store_dir, AppFileManifest, DiscoveredApp, InstalledRecord, InstalledSet,
};
pub use commerce::{
    apply_commerce_links, commerce_automations, commerce_child_slugs, commerce_communications,
    commerce_dashboard, commerce_entities, commerce_nav_items, commerce_notifications,
    commerce_reports, invoice_entity, is_commerce_entity, product_entity, quote_entity,
    sales_order_entity, sales_payment_entity, sales_return_entity, shipment_entity,
    CUSTOMER_ID_FIELD, CUSTOMER_TYPE_FIELD, FULFILL_FULFILLED, FULFILL_PARTIAL,
    FULFILL_UNFULFILLED, INVOICE_ENTITY, INVOICE_ITEM_ENTITY, INVOICE_PAID, INVOICE_SLUG,
    INVOICE_WORKFLOW, ORDER_COMPLETED, ORDER_CONFIRMED, ORDER_FULFILLED, PAYMENT_ALLOCATION_ENTITY,
    PAYMENT_WORKFLOW, PAY_RECEIVED, PRODUCT_ENTITY, PRODUCT_SLUG, QUOTE_ENTITY, QUOTE_ITEM_ENTITY,
    QUOTE_SLUG, QUOTE_WORKFLOW, RETURN_WORKFLOW, SALES_ORDER_ENTITY, SALES_ORDER_ITEM_ENTITY,
    SALES_ORDER_SLUG, SALES_ORDER_WORKFLOW, SALES_PAYMENT_ENTITY, SALES_PAYMENT_SLUG,
    SALES_RETURN_ENTITY, SALES_RETURN_ITEM_ENTITY, SALES_RETURN_SLUG, SHIPMENT_ENTITY,
    SHIPMENT_ITEM_ENTITY, SHIPMENT_SLUG, SHIPMENT_WORKFLOW,
};
pub use communication::{
    reject_unsafe_communication_payload, select_channels, validate_communication, CommunicationDef,
    RecipientAddress, CHANNELS, CHANNEL_EMAIL, CHANNEL_IN_APP, CHANNEL_SMS, CHANNEL_WHATSAPP,
    COMM_DEAD_LETTER, COMM_DELIVERED, COMM_FAILED, COMM_PENDING, COMM_QUEUED, COMM_SENDING,
    COMM_SENT, COMM_SKIPPED, PURPOSE_MARKETING, PURPOSE_TRANSACTIONAL,
};
pub use condition::Condition;
pub use context::{OpContext, ROLE_PUBLIC, ROLE_WORKER};
pub use document::{
    resolve_print_format, validate_print_format, DocumentConfig, NamingConfig, PrintFormat,
    PrintSection, ReportDef, PRINT_SECTION_KINDS, PRINT_VARIANTS,
};
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
pub use money::{
    assert_balanced, money_mul_qty, parse_money, round_money, sum_debit_credit, MONEY_SCALE,
};
pub use operation::{operation, OperationDef};
pub use package::{extract_package, inspect_package, write_package, PackageMeta};
pub use page::{
    normalize_layout, reject_unsafe_page_payload, validate_page, PageActionRef, PageDef,
    PageSection, PageTab, PAGE_LAYOUTS, PAGE_SECTION_KINDS, PAGE_TEMPLATES, PAGE_VIEWS,
};
pub use platform::{
    webhook_secret, webhook_signature, ConfirmationDef, EntityActionDef, LinkDef, LinkFilter,
    NotificationDef, PublicFormDef, WebhookDef,
};
pub use rate_limit::{MemoryRateLimiter, RateLimiter};
pub use registry::EntityRegistry;
pub use sanitize::sanitize_html;
pub use schedule::{next_run_after, parse_cron, parse_timezone, schedule_slot_key, CronExpr};
pub use scheduling::{
    apply_default_end, conflict_message, generate_slots, intervals_overlap, is_blackout, lock_key,
    parse_date, parse_window, validate_scheduling, window_within_working_hours, AvailabilitySlot,
    SchedulingConfig, SchedulingSummary, TimeWindow, WorkingHours,
};
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
pub use template::{
    display_value, reject_unsafe_print_payload, reject_unsafe_template, render_template,
    template_paths, validate_template_paths, wrap_record, FormatOpts,
};
pub use timezone::{canonicalize_datetime, local_to_utc, utc_to_local};
pub use ui::{
    CalendarViewSpec, CardViewSpec, ChartMeasureSpec, ChartViewSpec, CommunicationSummary,
    DashboardCard, DashboardDef, DetailViewSpec, EntityCapabilities, EntityPermissions,
    EntityViews, FormViewSpec, KanbanCardSpec, KanbanViewSpec, ListViewSpec, PrintFormatSummary,
    TenantBranding, TenantBusinessConfig, TenantConfig, TenantFeatures, TenantUiConfig, UiConfig,
    UiEntityMeta, UiFieldMeta, UiWhen, UiWidget, ViewColumnSpec, ViewSectionSpec, WidgetOptions,
    WorkspaceNavItem, UI_SCHEMA_VERSION,
};
pub use validate::{
    destructive_field_removals, validate_bundle, InstalledAppRef, ValidationReport,
};
pub use validation::{
    apply_entity_rules, apply_field_rules, compare_rule_line, existence_rules, field_is_readonly,
    field_rule_lines, reject_readonly_writes, strip_computed_fields, strip_server_managed_fields,
    validate_record, CompareClause, ValidationRule, ValidationRules, WhenClause,
};
pub use version::{
    is_framework_dep, API_VERSION, APP_API_VERSION, FRAMEWORK_COMPAT_REQ, FRAMEWORK_VERSION,
    METADATA_SCHEMA_VERSION, MIGRATION_FORMAT_VERSION,
};

pub mod prelude {
    pub use crate::{
        AppModule, AppModuleBuilder, AutomationDef, ChildTableDef, DocumentConfig, EntityActionDef,
        EntityDef, EntityRegistry, FieldDef, FieldType, LinkDef, NamingConfig, NotificationDef,
        OnDelete, OpContext, PageDef, PrintFormat, PublicFormDef, QefroError, QefroResult,
        RecordLifecycle, RelationDef, RelationKind, ReportDef, RowPolicy, SchedulingConfig,
        UiConfig, ValidationRule, ValidationRules, WebhookDef,
    };
}
