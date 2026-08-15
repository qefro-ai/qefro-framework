use async_trait::async_trait;
use qefro_api::{OperationCtx, OperationHandler};
use qefro_core::QefroResult;
use serde_json::{json, Value};

pub struct LoseOpportunity;

#[async_trait]
impl OperationHandler for LoseOpportunity {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        match ctx.status() {
            "Qualified" => ctx.apply_transition("lose")?,
            "Open" => ctx.apply_transition("lose_open")?,
            _ => {
                return Err(OperationCtx::fail(
                    "invalid_state",
                    "Opportunity cannot be marked lost in the current state",
                ));
            }
        };
        ctx.emit("opportunity.lost", json!({ "entity_id": ctx.record_id()? }));
        Ok(ctx.record.clone())
    }
}
