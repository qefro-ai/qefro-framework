use async_trait::async_trait;
use qefro_api::{OperationCtx, OperationHandler};
use qefro_core::QefroResult;
use serde_json::{json, Value};

pub struct ConvertLead;

#[async_trait]
impl OperationHandler for ConvertLead {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if ctx.status() != "Contacted" {
            return Err(OperationCtx::fail(
                "invalid_state",
                "Only contacted leads can be converted",
            ));
        }
        let name = ctx
            .record
            .get("company")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| ctx.record.get("title").and_then(|v| v.as_str()))
            .unwrap_or("Converted lead")
            .to_string();
        let customer = ctx
            .create(
                "CrmCustomer",
                json!({
                    "name": name,
                    "email": ctx.record.get("email"),
                    "phone": ctx.record.get("phone"),
                }),
            )
            .await?;
        ctx.apply_transition("qualify")?;
        ctx.emit(
            "lead.converted",
            json!({
                "lead_id": ctx.record_id()?,
                "customer_id": customer.get("id"),
            }),
        );
        Ok(ctx.record.clone())
    }
}
