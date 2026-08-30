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
use qefro_search::Query;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) struct EntityServiceOps<'a>(pub &'a AppState);

impl EntityOps for EntityServiceOps<'_> {
    async fn list(&self, ctx: &OpContext, entity: &str, query: Query) -> QefroResult<Value> {
        let page = self.0.entities.list(ctx, entity, query).await?;
        serde_json::to_value(page).map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn get(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        self.0.entities.get(ctx, entity, id).await
    }

    async fn create(&self, ctx: &OpContext, entity: &str, data: Value) -> QefroResult<Value> {
        self.0.entities.create(ctx, entity, data).await
    }

    async fn update(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        data: Value,
    ) -> QefroResult<Value> {
        self.0.entities.update(ctx, entity, id, data).await
    }

    async fn delete(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        self.0.entities.delete(ctx, entity, id).await
    }

    async fn transition(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        transition: &str,
    ) -> QefroResult<Value> {
        self.0.entities.transition(ctx, entity, id, transition).await
    }

    async fn execute(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
        name: &str,
        input: Value,
    ) -> QefroResult<Value> {
        self.0.entities.execute(ctx, entity, id, name, input).await
    }

    async fn list_activity(&self, ctx: &OpContext, entity: &str, id: Uuid) -> QefroResult<Value> {
        let items = self.0.entities.list_activity(ctx, entity, id, 50).await?;
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
        let row = self.0.entities.add_comment(ctx, entity, id, message).await?;
        serde_json::to_value(row).map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn list_attachments(
        &self,
        ctx: &OpContext,
        entity: &str,
        id: Uuid,
    ) -> QefroResult<Value> {
        let items = self.0.entities.list_record_attachments(ctx, entity, id).await?;
        serde_json::to_value(json!({ "items": items }))
            .map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn search(&self, ctx: &OpContext, q: &str, limit: usize) -> QefroResult<Value> {
        let results = self.0.entities.global_search_grouped(ctx, q, limit).await?;
        serde_json::to_value(results).map_err(|e| qefro_core::QefroError::internal(e.to_string()))
    }

    async fn run_report(&self, ctx: &OpContext, name: &str, filters: Value) -> QefroResult<Value> {
        let report = self
            .0
            .reports_live()
            .into_iter()
            .find(|r| r.name == name)
            .ok_or_else(|| qefro_core::QefroError::not_found(format!("report '{name}' not found")))?;
        if !ctx.allows_app(report.module.as_deref()) {
            return Err(qefro_core::QefroError::not_found(format!(
                "report '{name}' not found"
            )));
        }
        self.0.entities.run_report(ctx, &report, filters).await
    }

    async fn get_dashboard(&self, ctx: &OpContext, name: &str) -> QefroResult<Value> {
        let dash = self
            .0
            .dashboards_live()
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| {
                qefro_core::QefroError::not_found(format!("dashboard '{name}' not found"))
            })?;
        if !ctx.allows_app(dash.module.as_deref()) {
            return Err(qefro_core::QefroError::not_found(format!(
                "dashboard '{name}' not found"
            )));
        }
        let mut cards = Vec::new();
        for card in &dash.cards {
            match self.0.entities.dashboard_card_value(ctx, card).await {
                Ok(value) => cards.push(value),
                Err(err)
                    if matches!(
                        err,
                        qefro_core::QefroError::Forbidden { .. }
                            | qefro_core::QefroError::NotFound { .. }
                            | qefro_core::QefroError::AppNotEnabled { .. }
                    ) =>
                {
                    continue
                }
                Err(err) => return Err(err),
            }
        }
        Ok(json!({
            "name": dash.name,
            "label": dash.label,
            "module": dash.module,
            "cards": cards,
        }))
    }
}

pub fn app_router(state: AppState) -> axum::Router {
    routes::router(state)
}
