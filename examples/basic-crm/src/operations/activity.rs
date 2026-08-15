use async_trait::async_trait;
use qefro_api::{OperationCtx, OperationHandler};
use qefro_core::QefroResult;
use serde_json::{json, Value};

pub struct CompleteActivity;

#[async_trait]
impl OperationHandler for CompleteActivity {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if ctx.record.get("done") == Some(&json!(true)) {
            return Err(OperationCtx::fail(
                "already_complete",
                "Activity is already complete",
            ));
        }
        ctx.set_field("done", json!(true));
        ctx.emit(
            "activity.completed",
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}
