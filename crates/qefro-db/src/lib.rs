//! PostgreSQL access for Qefro Framework.
//!
//! SQL is constructed from entity metadata. Identifiers are validated; values
//! are always bound as parameters. Tenant predicates are injected by the
//! repository and cannot be omitted for tenant-owned entities.

pub mod audit;
pub mod jobs;
pub mod operation;
pub mod pool;
pub mod query;
pub mod repository;
pub mod schema;
pub mod service;

pub use audit::AuditLogger;
pub use jobs::{JobHandler, JobQueue, JobRecord, JobRegistry, LogNotificationJob};
pub use operation::{
    available_for_record, crud_operation_defs, execute_operation, operation_allowed,
    NoopOperationHandler, OperationBinding, OperationCtx, OperationHandler, OperationRegistry,
};
pub use pool::{connect, DbPool};
pub use repository::{EntityRepository, Page};
pub use schema::{apply_schema, entity_ddl};
pub use service::EntityService;
