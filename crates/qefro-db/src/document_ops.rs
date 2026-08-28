//! Generic document operations (duplicate / amend) on the existing operation pipeline.

use crate::operation::{OperationCtx, OperationHandler};
use async_trait::async_trait;
use qefro_core::{OperationDef, QefroResult, RelationKind};
use serde_json::{json, Value};

pub fn duplicate_def(entity: &str) -> OperationDef {
    OperationDef::new("duplicate", entity)
        .label("Duplicate")
        .description("Copy this document and its child rows into a new draft")
}

pub fn amend_def(entity: &str) -> OperationDef {
    OperationDef::new("amend", entity)
        .label("Amend")
        .description("Create a draft copy of a submitted document")
}

pub struct DuplicateDocument;

#[async_trait]
impl OperationHandler for DuplicateDocument {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        copy_document(ctx).await
    }
}

pub struct AmendDocument;

#[async_trait]
impl OperationHandler for AmendDocument {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        copy_document(ctx).await
    }
}

async fn copy_document(ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
    let id = ctx.record_id()?;
    let mut data = ctx.record.clone();
    if let Some(obj) = data.as_object_mut() {
        for key in [
            "id",
            "tenant_id",
            "created_at",
            "updated_at",
            "created_by",
            "updated_by",
            "deleted_at",
            "_expanded",
            "_related",
            "_workflow",
            "_actions",
            "_permissions",
            "_links",
        ] {
            obj.remove(key);
        }
        if let Some(naming) = &ctx.entity.naming {
            obj.remove(&naming.field);
        }
        obj.insert("status".into(), json!("Draft"));
        for field in &ctx.entity.fields {
            if field.is_child_table() || field.computed {
                obj.remove(&field.name);
            }
        }
    }
    let entity_name = ctx.entity.name.clone();
    let created = ctx.create(&entity_name, data).await?;
    let new_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    for field in ctx.entity.fields.clone() {
        let Some(rel) = field.relation.clone() else {
            continue;
        };
        if rel.kind != RelationKind::ChildTable {
            continue;
        }
        let inverse = rel
            .inverse_field
            .clone()
            .unwrap_or_else(|| "parent_id".into());
        let rows = ctx
            .list(&rel.target_entity, &inverse, json!(id.to_string()))
            .await?;
        let child_def = ctx.entity_def(&rel.target_entity)?;
        for mut row in rows {
            if let Some(obj) = row.as_object_mut() {
                obj.remove("id");
                obj.remove("tenant_id");
                obj.remove("created_at");
                obj.remove("updated_at");
                obj.insert(inverse.clone(), json!(new_id));
                for f in &child_def.fields {
                    if f.computed {
                        obj.remove(&f.name);
                    }
                }
            }
            ctx.create(&rel.target_entity, row).await?;
        }
    }
    ctx.emit(
        format!("{}.duplicated", qefro_core::snake_case(&ctx.entity.name)),
        json!({ "source_id": id, "entity_id": new_id }),
    );
    Ok(created)
}

pub fn register_document_operations(
    operations: &mut crate::operation::OperationRegistry,
    registry: &qefro_core::EntityRegistry,
) {
    use std::sync::Arc;
    for entity in registry.list() {
        let Some(doc) = &entity.document else {
            continue;
        };
        if doc.submit_enabled
            && operations.try_get(&entity.name, "submit").is_none()
            && operations.try_get(&entity.name, "confirm").is_none()
        {
            operations.register(
                OperationDef::new("submit", &entity.name)
                    .label("Submit")
                    .transition("submit")
                    .event(format!(
                        "{}.submitted",
                        qefro_core::snake_case(&entity.name)
                    )),
                Arc::new(SubmitDocument),
            );
        }
        if doc.cancel_enabled && operations.try_get(&entity.name, "cancel").is_none() {
            operations.register(
                OperationDef::new("cancel", &entity.name)
                    .label("Cancel")
                    .confirm()
                    .style("danger")
                    .event(format!(
                        "{}.cancelled",
                        qefro_core::snake_case(&entity.name)
                    )),
                Arc::new(CancelDocument),
            );
        }
        if doc.duplicate_enabled && operations.try_get(&entity.name, "duplicate").is_none() {
            operations.register(duplicate_def(&entity.name), Arc::new(DuplicateDocument));
        }
        if doc.amend_enabled && operations.try_get(&entity.name, "amend").is_none() {
            operations.register(amend_def(&entity.name), Arc::new(AmendDocument));
        }
    }
}

pub struct SubmitDocument;

#[async_trait]
impl OperationHandler for SubmitDocument {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        ctx.apply_transition("submit")?;
        ctx.emit(
            format!("{}.submitted", qefro_core::snake_case(&ctx.entity.name)),
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}

pub struct CancelDocument;

#[async_trait]
impl OperationHandler for CancelDocument {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> QefroResult<Value> {
        if ctx.apply_transition("cancel").is_err() {
            ctx.apply_transition("cancel_submitted")?;
        }
        ctx.emit(
            format!("{}.cancelled", qefro_core::snake_case(&ctx.entity.name)),
            json!({ "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}
