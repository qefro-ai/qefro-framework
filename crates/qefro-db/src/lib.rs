//! PostgreSQL access for Qefro Framework.
//!
//! SQL is constructed from entity metadata. Identifiers are validated; values
//! are always bound as parameters. Tenant predicates are injected by the
//! repository and cannot be omitted for tenant-owned entities.

pub mod activity;
pub mod app_registry;
pub mod attachments;
pub mod audit;
pub mod automation;
pub mod blobs;
pub mod bulk;
pub mod document_ops;
pub mod due;
pub mod global_search;
pub mod import;
pub mod jobs;
pub mod notifications;
pub mod numbering;
pub mod operation;
pub mod outbox;
pub mod pool;
pub mod print;
pub mod query;
pub mod reports;
pub mod repository;
pub mod saved_filters;
pub mod schema;
pub mod seeds;
pub mod service;
pub mod studio;
pub mod webhooks;

pub use app_registry::AppRegistryRow;
pub use attachments::{Attachment, AttachmentStore};
pub use audit::AuditLogger;
pub use automation::AutomationEngine;
pub use blobs::{BlobMeta, BlobMetaStore};
pub use bulk::BulkRequest;
pub use document_ops::register_document_operations;
pub use due::{DueReminderJob, DUE_REMINDER_JOB};
pub use global_search::{SearchGroup, SearchHit, SearchResponse};
pub use import::{ImportMapping, ImportPreview, ImportResult};
pub use jobs::{JobHandler, JobQueue, JobRecord, JobRegistry, LogNotificationJob};
pub use notifications::{EmailNotifyJob, InAppNotification, NotificationStore, PlatformDispatcher};
pub use operation::{
    available_for_record, crud_operation_defs, execute_operation, operation_allowed,
    NoopOperationHandler, OperationBinding, OperationCtx, OperationHandler, OperationRegistry,
};
pub use outbox::Outbox;
pub use pool::{connect, DbPool};
pub use repository::{EntityRepository, Page};
pub use saved_filters::{SavedFilter, SavedFilterStore};
pub use schema::{apply_schema, entity_ddl};
pub use seeds::apply_seed_batch;
pub use service::EntityService;
pub use studio::{
    to_yaml, DraftRequest, MetadataChangeService, PublishRequest, StudioDraft, StudioVersion,
};
pub use webhooks::{signed_headers, WebhookDelivery, WebhookLog};
