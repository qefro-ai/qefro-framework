//! Bulk, export, archive, and row-policy helpers on EntityService.

use super::service::EntityService;
use chrono::Utc;
use qefro_core::{OpContext, QefroError, QefroResult, RowPolicy};
use qefro_permissions::Action;
use qefro_search::Query;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkRequest {
    pub action: String,
    pub ids: Vec<Uuid>,
    #[serde(default)]
    pub fields: Value,
}

impl EntityService {
    pub fn enforce_row_policy(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &Value,
    ) -> QefroResult<()> {
        if ctx.is_admin() {
            return Ok(());
        }
        match entity.row_policy {
            Some(RowPolicy::AssignedTo) => {
                let assigned = record
                    .get("assigned_to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if assigned != ctx.user_id.to_string() {
                    return Err(QefroError::not_found(format!("{} not found", entity.name)));
                }
            }
            Some(RowPolicy::CreatedBy) => {
                let created = record
                    .get("created_by")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if created != ctx.user_id.to_string() {
                    return Err(QefroError::not_found(format!("{} not found", entity.name)));
                }
            }
            None => {}
        }
        Ok(())
    }

    pub fn apply_row_policy_filters(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        query: &mut Query,
    ) {
        if ctx.is_admin() {
            return;
        }
        match entity.row_policy {
            Some(RowPolicy::AssignedTo) if entity.get_field("assigned_to").is_some() => {
                query.filters.push(qefro_search::Filter::Eq {
                    field: "assigned_to".into(),
                    value: json!(ctx.user_id.to_string()),
                });
            }
            Some(RowPolicy::CreatedBy) => {
                query.filters.push(qefro_search::Filter::Eq {
                    field: "created_by".into(),
                    value: json!(ctx.user_id.to_string()),
                });
            }
            _ => {}
        }
    }

    pub async fn bulk(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        request: BulkRequest,
    ) -> QefroResult<Value> {
        if request.ids.is_empty() {
            return Err(QefroError::bad_request("ids are required"));
        }
        if request.ids.len() > 200 {
            return Err(QefroError::bad_request("bulk operations are limited to 200 records"));
        }
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.reject_worker_crud(ctx)?;
        let action = request.action.to_ascii_lowercase();
        match action.as_str() {
            "delete" => self.permissions.check(ctx, &entity.name, Action::Delete)?,
            "export" => self.permissions.check(ctx, &entity.name, Action::Export)?,
            "archive" | "restore" => {
                if !entity.archives() {
                    return Err(QefroError::bad_request("archive is not enabled for this entity"));
                }
                self.permissions.check(ctx, &entity.name, Action::Update)?;
            }
            "update" | "assign" => {
                self.permissions.check(ctx, &entity.name, Action::Update)?;
            }
            _ => {
                return Err(QefroError::bad_request(format!(
                    "unsupported bulk action '{action}'"
                )));
            }
        }

        let mut succeeded = 0u32;
        let mut failed = 0u32;
        let mut results = Vec::new();
        for id in &request.ids {
            let outcome = match action.as_str() {
                "delete" => self.delete(ctx, &entity.name, *id).await.map(|_| ()),
                "archive" => self.set_archived(ctx, &entity.name, *id, true).await,
                "restore" => self.set_archived(ctx, &entity.name, *id, false).await,
                "assign" => {
                    let assignee = request
                        .fields
                        .get("assigned_to")
                        .cloned()
                        .unwrap_or(Value::Null);
                    self.update(ctx, &entity.name, *id, json!({ "assigned_to": assignee }))
                        .await
                        .map(|_| ())
                }
                "update" => self
                    .update(ctx, &entity.name, *id, request.fields.clone())
                    .await
                    .map(|_| ()),
                "export" => Ok(()),
                _ => unreachable!(),
            };
            match outcome {
                Ok(()) => {
                    succeeded += 1;
                    results.push(json!({ "id": id, "ok": true }));
                }
                Err(e) => {
                    failed += 1;
                    results.push(json!({
                        "id": id,
                        "ok": false,
                        "error": e.to_string(),
                    }));
                }
            }
        }
        if entity.audit {
            let _ = self
                .audit
                .record(
                    ctx,
                    &entity.name,
                    None,
                    &format!("bulk:{action}"),
                    None,
                    Some(&json!({ "ids": request.ids, "succeeded": succeeded, "failed": failed })),
                )
                .await;
        }
        Ok(json!({
            "action": action,
            "succeeded": succeeded,
            "failed": failed,
            "results": results,
        }))
    }

    pub async fn set_archived(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        id: Uuid,
        archived: bool,
    ) -> QefroResult<()> {
        let entity = self.registry.get(entity_name)?;
        if !entity.archives() {
            return Err(QefroError::bad_request("archive is not enabled"));
        }
        self.permissions.check(ctx, &entity.name, Action::Update)?;
        let current = self.repo.get(&entity, ctx, id).await?;
        self.enforce_row_policy(ctx, &entity, &current)?;
        let table = qefro_core::quote_ident(&entity.table)?;
        let sql = if archived {
            format!(
                "UPDATE {table} SET archived_at = $1, updated_at = $1, updated_by = $2 WHERE id = $3 AND tenant_id = $4"
            )
        } else {
            format!(
                "UPDATE {table} SET archived_at = NULL, updated_at = $1, updated_by = $2 WHERE id = $3 AND tenant_id = $4"
            )
        };
        let now = Utc::now();
        sqlx::query(&sql)
            .bind(now)
            .bind(ctx.user_id)
            .bind(id)
            .bind(ctx.tenant_id)
            .execute(self.pool())
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        if entity.activity {
            let kind = if archived { "archived" } else { "restored" };
            let _ = self
                .activity
                .record(
                    ctx,
                    &entity.name,
                    id,
                    crate::activity::TYPE_UPDATED,
                    &format!("{} {kind}", entity.label),
                    json!({ "lifecycle": kind }),
                )
                .await;
        }
        Ok(())
    }

    pub async fn export_csv(
        &self,
        ctx: &OpContext,
        entity_name: &str,
        mut query: Query,
        ids: Option<Vec<Uuid>>,
    ) -> QefroResult<(String, String)> {
        let entity = self.registry.get(entity_name)?;
        self.ensure_app(ctx, &entity)?;
        self.permissions.check(ctx, &entity.name, Action::Export)?;
        query.page = 1;
        query.page_size = 1000;
        self.apply_row_policy_filters(ctx, &entity, &mut query);
        let page = self.list(ctx, entity_name, query).await?;
        let items: Vec<Value> = if let Some(ids) = ids {
            let set: std::collections::HashSet<_> = ids.into_iter().collect();
            page.items
                .into_iter()
                .filter(|row| {
                    row.get("id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .is_some_and(|id| set.contains(&id))
                })
                .collect()
        } else {
            page.items
        };
        let fields: Vec<_> = entity
            .business_fields()
            .iter()
            .filter(|f| f.ui.list && !f.secret && !f.system)
            .collect();
        let mut csv = String::new();
        csv.push_str(
            &fields
                .iter()
                .map(|f| csv_escape(&f.label))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
        for row in &items {
            let line = fields
                .iter()
                .map(|f| {
                    let v = row.get(&f.name).cloned().unwrap_or(Value::Null);
                    csv_escape(&value_text(&v))
                })
                .collect::<Vec<_>>()
                .join(",");
            csv.push_str(&line);
            csv.push('\n');
        }
        Ok((format!("{}.csv", entity.slug), csv))
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}
