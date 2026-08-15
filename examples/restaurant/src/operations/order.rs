use async_trait::async_trait;
use qefro_api::{OperationCtx, OperationHandler};
use qefro_core::QefroResult;
use serde_json::{json, Value};
use uuid::Uuid;

pub struct ConfirmOrder;

#[async_trait]
impl OperationHandler for ConfirmOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let id = ctx.record_id()?;
        let items = ctx.list("OrderItem", "order_id", json!(id.to_string())).await?;
        if items.is_empty() {
            return Err(OperationCtx::fail(
                "empty_order",
                "Order must have at least one item",
            ));
        }
        for item in &items {
            let menu_id = item
                .get("menu_item_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| OperationCtx::fail("invalid_item", "Order item is missing a menu item"))?;
            let menu = ctx.get("MenuItem", menu_id).await?;
            if menu.get("available") == Some(&json!(false)) {
                return Err(OperationCtx::fail(
                    "menu_item_unavailable",
                    "A menu item on this order is not available",
                ));
            }
        }
        ctx.apply_transition("confirm")?;
        ctx.emit("order.confirmed", json!({ "entity_id": id }));
        ctx.enqueue_job(
            "notify_order_confirmed",
            json!({ "entity": "Order", "entity_id": id }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct StartPreparation;

#[async_trait]
impl OperationHandler for StartPreparation {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("prepare")?;
        ctx.emit("order.preparing", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}

pub struct MarkReady;

#[async_trait]
impl OperationHandler for MarkReady {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("ready")?;
        ctx.emit("order.ready", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}

pub struct CompleteOrder;

#[async_trait]
impl OperationHandler for CompleteOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("complete")?;
        ctx.emit("order.completed", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}

pub struct CancelOrder;

#[async_trait]
impl OperationHandler for CancelOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let status = ctx.status().to_string();
        match status.as_str() {
            "Draft" => ctx.apply_transition("cancel")?,
            "Confirmed" => ctx.apply_transition("cancel_confirmed")?,
            "Preparing" => ctx.apply_transition("cancel_preparing")?,
            _ => {
                return Err(OperationCtx::fail(
                    "invalid_state",
                    "Order cannot be cancelled in the current state",
                ));
            }
        };
        ctx.emit("order.cancelled", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}
