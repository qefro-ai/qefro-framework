use async_trait::async_trait;
use qefro_api::{OperationCtx, OperationHandler};
use qefro_core::QefroResult;
use serde_json::{json, Value};

pub struct ConfirmReservation;

#[async_trait]
impl OperationHandler for ConfirmReservation {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let table_id = ctx.uuid_field("table_id")?;
        let table = ctx.get("DiningTable", table_id).await?;
        let status = table.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status != "available" {
            return Err(OperationCtx::fail(
                "table_unavailable",
                "The selected table is not available",
            ));
        }
        ctx.update("DiningTable", table_id, json!({ "status": "reserved" }))
            .await?;
        ctx.apply_transition("confirm")?;
        let id = ctx.record_id()?;
        ctx.emit("reservation.confirmed", json!({ "entity_id": id }));
        ctx.enqueue_job(
            "notify_reservation_confirmed",
            json!({ "entity": "Reservation", "entity_id": id }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct SeatCustomer;

#[async_trait]
impl OperationHandler for SeatCustomer {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let table_id = ctx.uuid_field("table_id")?;
        let _table = ctx.get("DiningTable", table_id).await?;
        ctx.update("DiningTable", table_id, json!({ "status": "occupied" }))
            .await?;
        ctx.apply_transition("seat")?;
        ctx.emit(
            "reservation.seated",
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct CompleteReservation;

#[async_trait]
impl OperationHandler for CompleteReservation {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let table_id = ctx.uuid_field("table_id")?;
        ctx.update("DiningTable", table_id, json!({ "status": "available" }))
            .await?;
        ctx.apply_transition("complete")?;
        ctx.emit(
            "reservation.completed",
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct CancelReservation;

#[async_trait]
impl OperationHandler for CancelReservation {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let status = ctx.status().to_string();
        let table_id = ctx.uuid_field("table_id")?;
        match status.as_str() {
            "Pending" => {
                ctx.apply_transition("cancel")?;
            }
            "Confirmed" => {
                ctx.apply_transition("cancel_confirmed")?;
                ctx.update("DiningTable", table_id, json!({ "status": "available" }))
                    .await?;
            }
            _ => {
                return Err(OperationCtx::fail(
                    "invalid_state",
                    "Reservation cannot be cancelled in the current state",
                ));
            }
        }
        ctx.emit(
            "reservation.cancelled",
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}
