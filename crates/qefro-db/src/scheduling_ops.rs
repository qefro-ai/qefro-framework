//! Generic reschedule operation. Updates start/end through EntityService rules.

use crate::operation::{OperationCtx, OperationHandler};
use async_trait::async_trait;
use qefro_core::{ident::snake_case, OperationDef, QefroError, QefroResult};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn reschedule_def(entity: &str) -> OperationDef {
    OperationDef::new("reschedule", entity)
        .label("Reschedule")
        .description("Change the scheduled start and end. Conflict detection still applies.")
}

pub struct RescheduleRecord;

#[async_trait]
impl OperationHandler for RescheduleRecord {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        let Some(config) = &ctx.entity.scheduling else {
            return Err(QefroError::bad_request(format!(
                "{} is not schedulable",
                ctx.entity.name
            )));
        };
        let id = ctx.record_id()?;
        let mut patch = json!({});
        let obj = patch.as_object_mut().unwrap();
        for name in [
            Some(config.start_field.as_str()),
            config.end_field.as_deref(),
            config.time_field.as_deref(),
            config.end_time_field.as_deref(),
            config.all_day_field.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(value) = ctx.input.get(name) {
                obj.insert(name.to_string(), value.clone());
            }
        }
        for resource in &config.resources {
            if let Some(value) = ctx.input.get(resource) {
                obj.insert(resource.clone(), value.clone());
            }
        }
        if obj.is_empty() {
            return Err(QefroError::bad_request(
                "reschedule requires a start, end, or resource field",
            ));
        }
        let entity_name = ctx.entity.name.clone();
        let start_field = config.start_field.clone();
        let previous = ctx.record.clone();
        let updated = ctx.update(&entity_name, id, patch).await?;
        ctx.record = updated.clone();
        ctx.emit(
            format!("{}.rescheduled", snake_case(&entity_name)),
            json!({
                "entity_id": id,
                "from": previous.get(&start_field),
                "to": updated.get(&start_field),
            }),
        );
        Ok(updated)
    }
}

pub fn register_scheduling_operations(
    operations: &mut crate::operation::OperationRegistry,
    registry: &qefro_core::EntityRegistry,
) {
    for entity in registry.list() {
        if entity.scheduling.is_none() {
            continue;
        }
        if operations.try_get(&entity.name, "reschedule").is_none() {
            operations.register(reschedule_def(&entity.name), Arc::new(RescheduleRecord));
        }
    }
}
