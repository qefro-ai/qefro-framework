//! PostgreSQL access for Qefro Framework.
//!
//! SQL is constructed from entity metadata. Identifiers are validated; values
//! are always bound as parameters. Tenant predicates are injected by the
//! repository and cannot be omitted for tenant-owned entities.

pub mod accounting;
pub mod activity;
pub mod app_registry;
pub mod attachments;
pub mod audit;
pub mod automation;
pub mod blobs;
pub mod bulk;
pub mod commerce;
pub mod communication;
pub mod custom_fields;
pub mod document_ops;
pub mod due;
pub mod global_search;
pub mod import;
pub mod jobs;
pub mod notifications;
pub mod numbering;
pub mod operation;
pub mod operation_run;
pub mod outbox;
pub mod pool;
pub mod print;
pub mod query;
pub mod reports;
pub mod repository;
pub mod saved_filters;
pub mod scheduling;
pub mod scheduling_ops;
pub mod schema;
pub mod seeds;
pub mod service;
pub mod studio;
pub mod webhooks;

pub use accounting::{accounting_operation_defs, post_ledger, register_accounting_operations};
pub use app_registry::AppRegistryRow;
pub use attachments::{Attachment, AttachmentPurgeJob, AttachmentStore, ATTACHMENT_PURGE_JOB};
pub use audit::AuditLogger;
pub use automation::AutomationEngine;
pub use blobs::{BlobMeta, BlobMetaStore};
pub use bulk::BulkRequest;
pub use commerce::{
    commerce_operation_defs, inventory_consume, inventory_release, inventory_reserve,
    inventory_restore, register_commerce_operations,
};
pub use communication::{
    dispatch_event_communications, enqueue_communication, CommunicationDeliverJob,
    CommunicationDispatcher, CommunicationHub, CommunicationLog, CommunicationProvider,
    CommunicationStore, LogEmailProvider, LogSmsProvider, LogWhatsAppProvider, OutboundMessage,
    RecordingProvider, COMMUNICATION_DELIVER_JOB,
};
pub use custom_fields::CustomFieldStore;
pub use document_ops::register_document_operations;
pub use due::{DueReminderJob, DUE_REMINDER_JOB};
pub use global_search::{SearchGroup, SearchHit, SearchResponse};
pub use import::{
    DuplicatePolicy, ImportFormat, ImportJobRecord, ImportMapping, ImportMode, ImportOptions,
    ImportPreview, ImportResult, ImportRunJob, IMPORT_RUN_JOB,
};
pub use jobs::{JobHandler, JobQueue, JobRecord, JobRegistry, LogNotificationJob};
pub use notifications::{EmailNotifyJob, InAppNotification, NotificationStore, PlatformDispatcher};
pub use operation::{
    available_for_record, crud_operation_defs, execute_operation, execute_operation_with,
    operation_allowed, ExecuteOpts, NoopOperationHandler, OperationBinding, OperationCtx,
    OperationExecuteJob, OperationHandler, OperationRegistry,
};
pub use operation_run::{
    OperationRun, OperationRunStore, OPERATION_EXECUTE_JOB, STATUS_COMPLETED as RUN_COMPLETED,
    STATUS_FAILED as RUN_FAILED, STATUS_QUEUED as RUN_QUEUED, STATUS_RUNNING as RUN_RUNNING,
};
pub use outbox::Outbox;
pub use pool::{connect, DbPool};
pub use repository::{EntityRepository, Page};
pub use saved_filters::{SavedFilter, SavedFilterStore};
pub use scheduling::{ScheduleReminderJob, SCHEDULE_REMINDER_JOB};
pub use scheduling_ops::register_scheduling_operations;
pub use schema::{apply_schema, entity_ddl};
pub use seeds::apply_seed_batch;
pub use service::EntityService;
pub use studio::{
    to_yaml, DraftRequest, MetadataChangeService, PublishRequest, StudioDraft, StudioVersion,
};
pub use webhooks::{signed_headers, WebhookDelivery, WebhookLog};
