//! PostgreSQL access for Qefro Framework.
//!
//! SQL is constructed from entity metadata. Identifiers are validated; values
//! are always bound as parameters. Tenant predicates are injected by the
//! repository and cannot be omitted for tenant-owned entities.

pub mod app_registry;
pub mod audit;
pub mod blobs;
pub mod document_ops;
pub mod jobs;
pub mod numbering;
pub mod operation;
pub mod pool;
pub mod print;
pub mod query;
pub mod reports;
pub mod repository;
pub mod saved_filters;
pub mod schema;
pub mod seeds;
pub mod service;

pub use app_registry::AppRegistryRow;
pub use audit::AuditLogger;
pub use blobs::{BlobMeta, BlobMetaStore};
pub use document_ops::register_document_operations;
pub use jobs::{JobHandler, JobQueue, JobRecord, JobRegistry, LogNotificationJob};
pub use operation::{
    available_for_record, crud_operation_defs, execute_operation, operation_allowed,
    NoopOperationHandler, OperationBinding, OperationCtx, OperationHandler, OperationRegistry,
};
pub use pool::{connect, DbPool};
pub use repository::{EntityRepository, Page};
pub use schema::{apply_schema, entity_ddl};
pub use seeds::apply_seed_batch;
pub use saved_filters::{SavedFilter, SavedFilterStore};
pub use service::EntityService;
