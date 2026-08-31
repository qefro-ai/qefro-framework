//! Tenant-scoped custom field definitions. Merged at request time — never via
//! process-wide [`qefro_core::EntityRegistry::overlay_put`].

use chrono::{DateTime, Utc};
use qefro_core::{
    custom_types_compatible, merge_custom_fields, validate_custom_field, CustomFieldStatus,
    EntityDef, EntityRegistry, FieldDef, OpContext, QefroError, QefroResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomFieldRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity: String,
    pub name: String,
    pub definition: Value,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

pub struct CustomFieldStore {
    pool: PgPool,
    cache: RwLock<HashMap<(Uuid, String), Arc<Vec<FieldDef>>>>,
}

impl CustomFieldStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn invalidate(&self, tenant_id: Uuid, entity: &str) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(&(tenant_id, entity.to_string()));
        }
    }

    pub fn invalidate_tenant(&self, tenant_id: Uuid) {
        if let Ok(mut cache) = self.cache.write() {
            cache.retain(|(tid, _), _| *tid != tenant_id);
        }
    }

    pub async fn list_effective(
        &self,
        tenant_id: Uuid,
        entity: &str,
    ) -> QefroResult<Arc<Vec<FieldDef>>> {
        if let Ok(cache) = self.cache.read() {
            if let Some(hit) = cache.get(&(tenant_id, entity.to_string())) {
                return Ok(hit.clone());
            }
        }
        let rows: Vec<(Value, String)> = sqlx::query_as(
            r#"
            SELECT definition, status
            FROM qefro_custom_fields
            WHERE tenant_id = $1 AND entity = $2
            ORDER BY name
            "#,
        )
        .bind(tenant_id)
        .bind(entity)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let mut fields = Vec::new();
        for (definition, status) in rows {
            let mut field: FieldDef = serde_json::from_value(definition)
                .map_err(|e| QefroError::internal(format!("invalid stored custom field: {e}")))?;
            field.custom = true;
            field.custom_status = match status.as_str() {
                "deprecated" => CustomFieldStatus::Deprecated,
                "disabled" => CustomFieldStatus::Disabled,
                _ => CustomFieldStatus::Active,
            };
            fields.push(field);
        }
        let arc = Arc::new(fields);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert((tenant_id, entity.to_string()), arc.clone());
        }
        Ok(arc)
    }

    pub async fn merge_into(&self, tenant_id: Uuid, base: &EntityDef) -> QefroResult<EntityDef> {
        let extras = self.list_effective(tenant_id, &base.name).await?;
        if extras.is_empty() {
            return Ok(base.clone());
        }
        merge_custom_fields(base, extras.as_ref())
    }

    pub async fn upsert(
        &self,
        ctx: &OpContext,
        registry: &EntityRegistry,
        entity_name: &str,
        payload: &Value,
    ) -> QefroResult<FieldDef> {
        let base = registry.get(entity_name)?;
        let existing = self.list_effective(ctx.tenant_id, entity_name).await?;
        let mut field = field_from_payload(payload)?;
        field.custom = true;
        field.ui.sortable = false;
        if field.ui.section.is_none() {
            field.ui.section = Some("Custom".into());
        }

        let action = payload
            .get("status")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("action").and_then(|v| v.as_str()))
            .unwrap_or("active");
        field.custom_status = match action {
            "deprecated" | "deprecate" => CustomFieldStatus::Deprecated,
            "disabled" | "disable" => CustomFieldStatus::Disabled,
            _ => CustomFieldStatus::Active,
        };

        if let Some(prev) = existing.iter().find(|f| f.name == field.name) {
            if !custom_types_compatible(&prev.field_type, &field.field_type) {
                return Err(QefroError::bad_request(format!(
                    "cannot change custom field '{}' from {} to {}",
                    field.name,
                    prev.field_type.as_str(),
                    field.field_type.as_str()
                )));
            }
            if let Some(expected) = payload.get("version").and_then(|v| v.as_i64()) {
                let row: Option<(i32,)> = sqlx::query_as(
                    "SELECT version FROM qefro_custom_fields WHERE tenant_id = $1 AND entity = $2 AND name = $3",
                )
                .bind(ctx.tenant_id)
                .bind(entity_name)
                .bind(&field.name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
                if let Some((version,)) = row {
                    if i64::from(version) != expected {
                        return Err(QefroError::conflict(format!(
                            "custom field {}.{} was modified concurrently (version {version})",
                            entity_name, field.name
                        )));
                    }
                }
            }
        } else if base.get_field(&field.name).is_some() {
            return Err(QefroError::bad_request(format!(
                "custom field '{}' collides with existing field on {}",
                field.name, entity_name
            )));
        }

        let mut probe = (*base).clone();
        for extra in existing.iter().filter(|f| f.name != field.name) {
            if extra.custom_status.in_effective_metadata() {
                probe.fields.push(extra.clone());
            }
        }
        validate_custom_field(&probe, &field)?;

        let definition =
            serde_json::to_value(&field).map_err(|e| QefroError::internal(e.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO qefro_custom_fields (
                id, tenant_id, entity, name, definition, status, version,
                created_at, updated_at, created_by, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,1, now(), now(), $7, $7)
            ON CONFLICT (tenant_id, entity, name) DO UPDATE SET
                definition = EXCLUDED.definition,
                status = EXCLUDED.status,
                version = qefro_custom_fields.version + 1,
                updated_at = now(),
                updated_by = EXCLUDED.updated_by
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctx.tenant_id)
        .bind(entity_name)
        .bind(&field.name)
        .bind(&definition)
        .bind(field.custom_status.as_str())
        .bind(ctx.user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        self.invalidate(ctx.tenant_id, entity_name);
        Ok(field)
    }
}

pub(crate) fn field_from_payload(payload: &Value) -> QefroResult<FieldDef> {
    let studio_alias = payload.get("type").and_then(|v| v.as_str()).is_some_and(|t| {
        matches!(
            t,
            "select" | "textarea" | "number" | "email" | "phone" | "currency" | "enum"
        )
    });
    if !studio_alias {
        if let Ok(field) = serde_json::from_value::<FieldDef>(payload.clone()) {
            return Ok(field);
        }
    }
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| QefroError::bad_request("custom field requires name"))?;
    let type_name = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("string");
    let mut field = match type_name {
        "text" | "textarea" => FieldDef::text(name),
        "number" | "integer" => FieldDef::integer(name),
        "decimal" | "currency" => FieldDef::decimal(name),
        "boolean" => FieldDef::boolean(name),
        "date" => FieldDef::date(name),
        "datetime" => FieldDef::datetime(name),
        "time" => FieldDef::time(name),
        "select" | "enum" => {
            let values: Vec<String> = payload
                .get("options")
                .or_else(|| payload.get("values"))
                .or_else(|| payload.get("enum_values"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            FieldDef::enum_values(name, values)
        }
        "email" => FieldDef::string(name).email(),
        "phone" => FieldDef::string(name).phone(),
        _ => FieldDef::string(name),
    };
    if let Some(label) = payload.get("label").and_then(|v| v.as_str()) {
        field = field.label(label);
    }
    if payload.get("required").and_then(|v| v.as_bool()) == Some(true) {
        field = field.required();
    }
    if payload.get("read_only").and_then(|v| v.as_bool()) == Some(true)
        || payload.get("readonly").and_then(|v| v.as_bool()) == Some(true)
    {
        field = field.readonly();
    }
    if payload.get("hidden").and_then(|v| v.as_bool()) == Some(true) {
        field = field.hidden();
    }
    if payload.get("filterable").and_then(|v| v.as_bool()) == Some(true)
        || payload.get("filter").and_then(|v| v.as_bool()) == Some(true)
    {
        field = field.filterable();
    }
    if payload.get("searchable").and_then(|v| v.as_bool()) == Some(true) {
        field = field.searchable();
    }
    if let Some(default) = payload.get("default") {
        if !default.is_null() {
            field = field.default_value(default.clone());
        }
    }
    if let Some(section) = payload.get("section").and_then(|v| v.as_str()) {
        field = field.section(section);
    }
    if let Some(help) = payload.get("help").and_then(|v| v.as_str()) {
        field = field.help(help);
    }
    if payload.get("type").and_then(|v| v.as_str()) == Some("currency") {
        field = field.with_currency();
    }
    if let Some(level) = payload.get("permission_level").and_then(|v| v.as_u64()) {
        field = field.permission_level(level as u8);
    }
    if let Some(when) = payload.get("visible_when") {
        if let Ok(w) = serde_json::from_value::<qefro_core::UiWhen>(when.clone()) {
            field.ui.visible_when = Some(w);
        }
    }
    Ok(field.custom())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn payload_select_options() {
        let field = field_from_payload(&json!({
            "name": "loyalty_tier",
            "label": "Loyalty Tier",
            "type": "select",
            "options": ["Bronze", "Silver", "Gold"],
            "required": true,
            "default": "Bronze"
        }))
        .unwrap();
        assert!(field.custom);
        assert!(field.required);
        assert_eq!(field.default, Some(json!("Bronze")));
        match &field.field_type {
            qefro_core::FieldType::Enum { values } => {
                assert_eq!(
                    values,
                    &vec!["Bronze".to_string(), "Silver".into(), "Gold".into()]
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
