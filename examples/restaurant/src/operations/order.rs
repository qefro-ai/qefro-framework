use async_trait::async_trait;
use qefro_api::{
    inventory_consume, inventory_release, inventory_reserve, post_ledger, OperationCtx,
    OperationHandler,
};
use qefro_core::{LedgerPosting, QefroResult, ACCOUNT_KEY_CASH, ACCOUNT_KEY_SALES};
use serde_json::{json, Value};
use uuid::Uuid;

fn order_type(record: &Value) -> &str {
    record
        .get("order_type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Dine-in")
}

fn pickup_at(record: &Value) -> Option<&str> {
    record
        .get("pickup_at")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn apply_channel_rules(ctx: &mut OperationCtx<'_, '_>, require_pickup: bool) -> QefroResult<()> {
    if order_type(&ctx.record) == "Takeaway" {
        ctx.set_field("table_id", Value::Null);
        ctx.set_field("reservation_id", Value::Null);
        if require_pickup && pickup_at(&ctx.record).is_none() {
            return Err(OperationCtx::fail(
                "pickup_required",
                "Takeaway prebooking requires a pickup time",
            ));
        }
        return Ok(());
    }
    if ctx
        .record
        .get("table_id")
        .and_then(|v| v.as_str())
        .is_none()
    {
        return Err(OperationCtx::fail(
            "table_required",
            "Dine-in orders require a table",
        ));
    }
    ctx.set_field("pickup_at", Value::Null);
    Ok(())
}

async fn ensure_order_items(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<()> {
    let id = ctx.record_id()?;
    let items = ctx
        .list("OrderItem", "order_id", json!(id.to_string()))
        .await?;
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
            .ok_or_else(|| {
                OperationCtx::fail("invalid_item", "Order item is missing a menu item")
            })?;
        let menu = ctx.get("MenuItem", menu_id).await?;
        if menu.get("available") == Some(&json!(false)) {
            return Err(OperationCtx::fail(
                "menu_item_unavailable",
                "A menu item on this order is not available",
            ));
        }
    }
    Ok(())
}

pub struct ConfirmOrder;

#[async_trait]
impl OperationHandler for ConfirmOrder {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let id = ctx.record_id()?;
        ensure_order_items(ctx).await?;
        apply_channel_rules(ctx, false)?;
        inventory_reserve(ctx, &ctx.record.clone()).await?;
        ctx.apply_transition("confirm")?;
        ctx.emit("order.confirmed", json!({ "entity_id": id }));
        ctx.enqueue_job(
            "notify_order_confirmed",
            json!({ "entity": "Order", "entity_id": id }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct SchedulePickup;

#[async_trait]
impl OperationHandler for SchedulePickup {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if order_type(&ctx.record) != "Takeaway" {
            return Err(OperationCtx::fail(
                "not_takeaway",
                "Only takeaway orders can be scheduled for pickup",
            ));
        }
        let id = ctx.record_id()?;
        ensure_order_items(ctx).await?;
        apply_channel_rules(ctx, true)?;
        ctx.apply_transition("schedule")?;
        ctx.emit("order.scheduled", json!({ "entity_id": id }));
        Ok(ctx.record.clone())
    }
}

pub struct StartPreparation;

#[async_trait]
impl OperationHandler for StartPreparation {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        inventory_consume(ctx, &ctx.record.clone()).await?;
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
        let id = ctx.record_id()?;
        let number = ctx
            .record
            .get("doc_no")
            .and_then(|v| v.as_str())
            .or_else(|| ctx.record.get("number").and_then(|v| v.as_str()))
            .unwrap_or("order")
            .to_string();
        let task = ctx
            .create(
                "Task",
                json!({
                    "title": format!("Follow up on {number}"),
                    "description": "Thank the guest and collect feedback.",
                    "entity_type": "Order",
                    "entity_id": id,
                    "priority": "normal",
                }),
            )
            .await?;
        ctx.emit(
            "order.completed",
            json!({
                "entity_id": id,
                "task_id": task.get("id"),
            }),
        );
        let amount = ctx.record.get("grand_total").cloned().unwrap_or(json!(0));
        let date = ctx
            .record
            .get("order_date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mut posting = LedgerPosting::new(format!("Order {number}"), number)
            .debit(ACCOUNT_KEY_CASH, amount.clone())
            .credit(ACCOUNT_KEY_SALES, amount);
        if let Some(date) = date {
            posting = posting.date(date);
        }
        let _ = post_ledger(ctx, posting).await?;
        ctx.set_message("Order completed");
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
            "Scheduled" => ctx.apply_transition("cancel_scheduled")?,
            "Confirmed" => ctx.apply_transition("cancel_confirmed")?,
            "Preparing" => ctx.apply_transition("cancel_preparing")?,
            _ => {
                return Err(OperationCtx::fail(
                    "invalid_state",
                    "Order cannot be cancelled in the current state",
                ));
            }
        };
        inventory_release(ctx, &ctx.record.clone()).await?;
        ctx.emit("order.cancelled", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}
