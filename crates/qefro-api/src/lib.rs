//! HTTP API and runtime composition for Qefro Framework.

pub mod error;
pub mod extract;
pub mod metrics;
pub mod openapi;
pub mod platform;
pub mod realtime;
pub mod routes;
pub mod runtime;
pub mod state;
pub mod studio;

pub use qefro_core::{operation, OperationDef};
pub use qefro_db::{
    JobHandler, JobQueue, JobRegistry, LogNotificationJob, NoopOperationHandler, OperationCtx,
    OperationHandler, OperationRegistry,
};
pub use runtime::{Config, InstalledApp, QefroRuntime};
pub use state::AppState;

use qefro_agent::EntityOps;
use qefro_core::{OpContext, QefroResult};
use qefro_db::EntityService;
use qefro_search::Query;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) struct EntityServiceOps<'a>(pub &'a EntityService);

impl EntityOps for EntityServiceOps<'_> {
    async fn list(&self, ctx: &OpContext, entity: &str, query: Query) -> QefroResult<Value> {
        let page = self.0.list(ctx, entity, query).await?;
        serde_json::to_value(page).map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn get(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        self.0.get(ctx, entity, id).await
    }

    async fn create(&self, ctx: &OpContext, entity: &str, data: Value) -> QefroResult<Value> {
        self.0.create(ctx, entity, data).await
    }

    async fn update(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        data: Value,
    ) -> QefroResult<Value> {
        self.0.update(ctx, entity, id, data).await
    }

    async fn delete(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        self.0.delete(ctx, entity, id).await
    }

    async fn transition(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        transition: &str,
    ) -> QefroResult<Value> {
        self.0.transition(ctx, entity, id, transition).await
    }

    async fn execute(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        name: &str,
        input: Value,
    ) -> QefroResult<Value> {
        self.0.execute(ctx, entity, id, name, input).await
    }

    async fn list_activity(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        let items = self.0.list_activity(ctx, entity, id, 50).await?;
        serde_json::to_value(json!({ "items": items }))
            .map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn add_comment(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        message: &str,
    ) -> QefroResult<Value> {
        let row = self.0.add_comment(ctx, entity, id, message).await?;
        serde_json::to_value(row).map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn list_attachments(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
    ) -> QefroResult<Value> {
        let items = self.0.list_record_attachments(ctx, entity, id).await?;
        serde_json::to_value(json!({ "items": items }))
            .map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }
}

pub fn app_router(state: AppState) -> axum::Router {
    routes::router(state)
}
