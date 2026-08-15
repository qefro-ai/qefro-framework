use crate::context::OpContext;
use crate::error::QefroResult;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Lifecycle hooks run inside the entity service, never in the HTTP layer.
#[async_trait]
pub trait EntityHook: Send + Sync {
    fn entity(&self) -> &str;

    async fn before_create(&self, _ctx: &OpContext, _record: &mut Value) -> QefroResult<()> {
        Ok(())
    }
    async fn after_create(&self, _ctx: &OpContext, _record: &Value) -> QefroResult<()> {
        Ok(())
    }
    async fn before_update(
        &self,
        _ctx: &OpContext,
        _current: &Value,
        _patch: &mut Value,
    ) -> QefroResult<()> {
        Ok(())
    }
    async fn after_update(&self, _ctx: &OpContext, _record: &Value) -> QefroResult<()> {
        Ok(())
    }
    async fn before_delete(&self, _ctx: &OpContext, _record: &Value) -> QefroResult<()> {
        Ok(())
    }
    async fn after_delete(&self, _ctx: &OpContext, _record: &Value) -> QefroResult<()> {
        Ok(())
    }

    /// Runs inside the operation transaction, before the handler.
    async fn before_operation(
        &self,
        _ctx: &OpContext,
        _operation: &str,
        _record: &Value,
        _input: &Value,
    ) -> QefroResult<()> {
        Ok(())
    }

    /// Runs inside the operation transaction, after the handler and primary write.
    async fn after_operation(
        &self,
        _ctx: &OpContext,
        _operation: &str,
        _record: &Value,
    ) -> QefroResult<()> {
        Ok(())
    }
}

pub struct NoopHook {
    entity: String,
}

impl NoopHook {
    pub fn new(entity: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
        }
    }
}

#[async_trait]
impl EntityHook for NoopHook {
    fn entity(&self) -> &str {
        &self.entity
    }
}

#[derive(Default, Clone)]
pub struct HookRegistry {
    hooks: HashMap<String, Vec<Arc<dyn EntityHook>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, hook: Arc<dyn EntityHook>) {
        self.hooks
            .entry(hook.entity().to_string())
            .or_default()
            .push(hook);
    }

    pub fn for_entity(&self, entity: &str) -> &[Arc<dyn EntityHook>] {
        self.hooks.get(entity).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub async fn before_create(
        &self,
        ctx: &OpContext,
        entity: &str,
        record: &mut Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.before_create(ctx, record).await?;
        }
        Ok(())
    }

    pub async fn after_create(
        &self,
        ctx: &OpContext,
        entity: &str,
        record: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.after_create(ctx, record).await?;
        }
        Ok(())
    }

    pub async fn before_update(
        &self,
        ctx: &OpContext,
        entity: &str,
        current: &Value,
        patch: &mut Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.before_update(ctx, current, patch).await?;
        }
        Ok(())
    }

    pub async fn after_update(
        &self,
        ctx: &OpContext,
        entity: &str,
        record: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.after_update(ctx, record).await?;
        }
        Ok(())
    }

    pub async fn before_delete(
        &self,
        ctx: &OpContext,
        entity: &str,
        record: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.before_delete(ctx, record).await?;
        }
        Ok(())
    }

    pub async fn after_delete(
        &self,
        ctx: &OpContext,
        entity: &str,
        record: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.after_delete(ctx, record).await?;
        }
        Ok(())
    }

    pub async fn before_operation(
        &self,
        ctx: &OpContext,
        entity: &str,
        operation: &str,
        record: &Value,
        input: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.before_operation(ctx, operation, record, input).await?;
        }
        Ok(())
    }

    pub async fn after_operation(
        &self,
        ctx: &OpContext,
        entity: &str,
        operation: &str,
        record: &Value,
    ) -> QefroResult<()> {
        for hook in self.for_entity(entity) {
            hook.after_operation(ctx, operation, record).await?;
        }
        Ok(())
    }
}
