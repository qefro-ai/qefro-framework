//! Custom fields are an extension layer on [`EntityDef`], not a second runtime.
//!
//! Application fields are declared with [`crate::entity::EntityDef::custom_field`]
//! (Git-committable). Tenant fields are stored separately and merged at request
//! time. Values live in the shared JSONB bag [`CUSTOM_BAG_COLUMN`] — Studio never
//! issues ADD COLUMN for a custom field.

use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::field::{FieldDef, FieldType};
use crate::ident::{assert_safe_ident, snake_case};
use crate::identity::is_secret_key;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Single JSONB column on every business table. Not a per-field DDL target.
pub const CUSTOM_BAG_COLUMN: &str = "qefro_custom";

/// Nested REST alias: `{ "custom": { "loyalty_tier": "Gold" } }`.
pub const CUSTOM_NESTED_KEY: &str = "custom";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldStatus {
    #[default]
    Active,
    Deprecated,
    Disabled,
}

impl CustomFieldStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Active and deprecated fields appear in effective metadata; disabled do not.
    pub fn in_effective_metadata(self) -> bool {
        matches!(self, Self::Active | Self::Deprecated)
    }

    pub fn skip_if_active(status: &Self) -> bool {
        status.is_active()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Disabled => "disabled",
        }
    }
}

/// System, bag, and identity names that must never be used for custom fields.
pub const RESERVED_CUSTOM_NAMES: &[&str] = &[
    "id",
    "tenant_id",
    "created_at",
    "updated_at",
    "created_by",
    "updated_by",
    "deleted_at",
    "archived_at",
    CUSTOM_BAG_COLUMN,
    CUSTOM_NESTED_KEY,
    "password",
    "password_hash",
    "token",
    "token_hash",
    "access_token",
    "refresh_token",
    "secret",
    "jwt",
    "session_token",
    "session_hash",
    "reset_token",
    "private_key",
    "storage_credentials",
    "debit",
    "credit",
];

pub fn is_reserved_custom_name(name: &str) -> bool {
    let n = snake_case(name);
    RESERVED_CUSTOM_NAMES
        .iter()
        .any(|r| r.eq_ignore_ascii_case(&n))
        || is_secret_key(&n)
}

pub fn custom_type_allowed(ty: &FieldType) -> bool {
    matches!(
        ty,
        FieldType::String
            | FieldType::Text
            | FieldType::Integer
            | FieldType::Decimal
            | FieldType::Boolean
            | FieldType::DateTime
            | FieldType::Date
            | FieldType::Time
            | FieldType::Enum { .. }
    )
}

pub fn custom_types_compatible(from: &FieldType, to: &FieldType) -> bool {
    if std::mem::discriminant(from) == std::mem::discriminant(to) {
        return match (from, to) {
            (FieldType::Enum { values: old }, FieldType::Enum { values: new }) => {
                old.iter().all(|v| new.iter().any(|n| n == v))
            }
            _ => true,
        };
    }
    matches!(
        (from, to),
        (FieldType::String, FieldType::Text)
            | (FieldType::Text, FieldType::String)
            | (FieldType::Integer, FieldType::Decimal)
    )
}

/// Validate a custom field against the base (and already-merged) entity.
pub fn validate_custom_field(entity: &EntityDef, field: &FieldDef) -> QefroResult<()> {
    let name = snake_case(&field.name);
    assert_safe_ident(&name).map_err(|e| {
        QefroError::bad_request(format!("invalid custom field name '{}': {e}", field.name))
    })?;
    if name != field.name {
        return Err(QefroError::bad_request(format!(
            "custom field '{}' must be snake_case",
            field.name
        )));
    }
    if is_reserved_custom_name(&name) {
        return Err(QefroError::bad_request(format!(
            "custom field name '{name}' is reserved"
        )));
    }
    if !field.custom {
        return Err(QefroError::bad_request(format!(
            "field '{name}' must be marked custom"
        )));
    }
    if !custom_type_allowed(&field.field_type) {
        return Err(QefroError::bad_request(format!(
            "custom field '{name}' type '{}' is not allowed (relations and JSON are not supported)",
            field.field_type.as_str()
        )));
    }
    if field.relation.is_some() || field.is_child_table() {
        return Err(QefroError::bad_request(format!(
            "custom field '{name}' cannot be a relation"
        )));
    }
    if field.unique {
        return Err(QefroError::bad_request(format!(
            "custom field '{name}' cannot be unique (JSONB bag storage)"
        )));
    }
    if field.computed || field.formula.is_some() {
        return Err(QefroError::bad_request(format!(
            "custom field '{name}' cannot be computed"
        )));
    }
    if let FieldType::Enum { values } = &field.field_type {
        if values.is_empty() {
            return Err(QefroError::bad_request(format!(
                "custom field '{name}' select/enum requires options"
            )));
        }
        if let Some(default) = &field.default {
            if let Some(s) = default.as_str() {
                if !values.iter().any(|v| v == s) {
                    return Err(QefroError::bad_request(format!(
                        "custom field '{name}' default is not an allowed option"
                    )));
                }
            } else if !default.is_null() {
                return Err(QefroError::bad_request(format!(
                    "custom field '{name}' default must match select options"
                )));
            }
        }
    } else if let Some(default) = &field.default {
        if let Some(err) = field.type_error(default) {
            return Err(QefroError::bad_request(format!(
                "custom field '{name}' default is invalid: {}",
                err.message
            )));
        }
    }
    if let Some(when) = &field.ui.visible_when {
        if entity.get_field(&when.field).is_none() && when.field != field.name {
            return Err(QefroError::bad_request(format!(
                "custom field '{name}' visible_when references unknown field '{}'",
                when.field
            )));
        }
    }
    if let Some(when) = &field.ui.readonly_when {
        if entity.get_field(&when.field).is_none() && when.field != field.name {
            return Err(QefroError::bad_request(format!(
                "custom field '{name}' readonly_when references unknown field '{}'",
                when.field
            )));
        }
    }
    if let Some(when) = &field.required_when {
        if entity.get_field(&when.field).is_none() && when.field != field.name {
            return Err(QefroError::bad_request(format!(
                "custom field '{name}' required_when references unknown field '{}'",
                when.field
            )));
        }
    }
    Ok(())
}

/// Append active/deprecated custom fields. Collisions with the base entity are errors.
pub fn merge_custom_fields(base: &EntityDef, extras: &[FieldDef]) -> QefroResult<EntityDef> {
    let mut merged = base.clone();
    for extra in extras {
        if !extra.custom_status.in_effective_metadata() {
            continue;
        }
        let mut field = extra.clone();
        field.custom = true;
        field.ui.sortable = false;
        if field.label.is_empty() {
            field.label = field.name.clone();
        }
        if field.ui.label.is_empty() {
            field.ui.label = field.label.clone();
        }
        validate_custom_field(&merged, &field)?;
        if merged.get_field(&field.name).is_some() {
            return Err(QefroError::bad_request(format!(
                "custom field '{}' collides with existing field on {}",
                field.name, merged.name
            )));
        }
        merged.fields.push(field);
    }
    Ok(merged)
}

/// Lift `{ "custom": { "k": v } }` into top-level keys when those keys are custom fields.
pub fn flatten_nested_custom(entity: &EntityDef, data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    let Some(nested) = obj.remove(CUSTOM_NESTED_KEY) else {
        return;
    };
    let Some(map) = nested.as_object() else {
        obj.insert(CUSTOM_NESTED_KEY.into(), nested);
        return;
    };
    for (key, value) in map {
        if entity.get_field(key).is_some_and(|f| f.custom) {
            obj.entry(key.clone()).or_insert(value.clone());
        }
    }
}

/// Move custom field values into the JSONB bag. Remaining keys are core columns.
pub fn pack_custom_values(entity: &EntityDef, obj: &mut Map<String, Value>) -> Value {
    let mut bag = match obj.remove(CUSTOM_BAG_COLUMN) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    for field in entity.fields.iter().filter(|f| f.custom && !f.ephemeral) {
        if let Some(value) = obj.remove(&field.name) {
            bag.insert(field.name.clone(), value);
        }
    }
    Value::Object(bag)
}

/// Expand the JSONB bag onto the record using effective custom field names.
pub fn unpack_custom_values(entity: &EntityDef, value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let Some(bag) = obj.remove(CUSTOM_BAG_COLUMN) else {
        return;
    };
    let Some(map) = bag.as_object() else {
        return;
    };
    for field in entity.fields.iter().filter(|f| f.custom && !f.secret) {
        if !field.custom_status.in_effective_metadata() {
            continue;
        }
        if let Some(v) = map.get(&field.name) {
            if !obj.contains_key(&field.name) {
                obj.insert(field.name.clone(), v.clone());
            }
        }
    }
}

pub fn custom_fields_of(entity: &EntityDef) -> Vec<&FieldDef> {
    entity.fields.iter().filter(|f| f.custom).collect()
}

pub fn core_fields_of(entity: &EntityDef) -> Vec<&FieldDef> {
    entity
        .fields
        .iter()
        .filter(|f| !f.custom && !f.system)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;
    use serde_json::json;

    fn customer() -> EntityDef {
        EntityDef::new("Customer")
            .field(FieldDef::string("name").required())
            .field(FieldDef::string("email").email())
            .build()
    }

    #[test]
    fn reserved_names_rejected() {
        for name in [
            "id",
            "tenant_id",
            "created_at",
            "password",
            "qefro_custom",
            "custom",
            "debit",
        ] {
            let field = FieldDef::string(name).custom();
            assert!(
                validate_custom_field(&customer(), &field).is_err(),
                "{name} should be reserved"
            );
        }
    }

    #[test]
    fn merge_appends_custom_and_rejects_collision() {
        let extra =
            FieldDef::enum_values("loyalty_tier", vec!["Bronze", "Silver", "Gold"]).custom();
        let merged = merge_custom_fields(&customer(), &[extra.clone()]).unwrap();
        assert!(merged.get_field("loyalty_tier").unwrap().custom);
        assert!(!merged.get_field("loyalty_tier").unwrap().stores_column());
        let collide = FieldDef::string("email").custom();
        assert!(merge_custom_fields(&customer(), &[collide]).is_err());
    }

    #[test]
    fn disabled_fields_are_omitted() {
        let mut extra = FieldDef::string("internal_score").custom();
        extra.custom_status = CustomFieldStatus::Disabled;
        let merged = merge_custom_fields(&customer(), &[extra]).unwrap();
        assert!(merged.get_field("internal_score").is_none());
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let extra =
            FieldDef::enum_values("loyalty_tier", vec!["Bronze", "Silver", "Gold"]).custom();
        let entity = merge_custom_fields(&customer(), &[extra]).unwrap();
        let mut data = json!({
            "name": "Ahmed",
            "custom": { "loyalty_tier": "Gold" }
        });
        flatten_nested_custom(&entity, &mut data);
        let obj = data.as_object_mut().unwrap();
        let bag = pack_custom_values(&entity, obj);
        assert_eq!(bag["loyalty_tier"], json!("Gold"));
        assert!(obj.get("loyalty_tier").is_none());
        obj.insert(CUSTOM_BAG_COLUMN.into(), bag);
        let mut record = Value::Object(obj.clone());
        unpack_custom_values(&entity, &mut record);
        assert_eq!(record["loyalty_tier"], json!("Gold"));
        assert!(record.get(CUSTOM_BAG_COLUMN).is_none());
    }

    #[test]
    fn relation_and_unique_rejected() {
        let rel = FieldDef::many_to_one("branch_id", "Branch").custom();
        assert!(validate_custom_field(&customer(), &rel).is_err());
        let uniq = FieldDef::string("gst_number").custom().unique();
        assert!(validate_custom_field(&customer(), &uniq).is_err());
    }

    #[test]
    fn enum_default_must_be_option() {
        let field = FieldDef::enum_values("loyalty_tier", vec!["Bronze", "Silver"])
            .custom()
            .default_value(json!("Platinum"));
        assert!(validate_custom_field(&customer(), &field).is_err());
        let ok = FieldDef::enum_values("loyalty_tier", vec!["Bronze", "Silver"])
            .custom()
            .default_value(json!("Bronze"));
        assert!(validate_custom_field(&customer(), &ok).is_ok());
    }

    #[test]
    fn type_change_compatibility() {
        assert!(custom_types_compatible(
            &FieldType::String,
            &FieldType::Text
        ));
        assert!(custom_types_compatible(
            &FieldType::Integer,
            &FieldType::Decimal
        ));
        assert!(!custom_types_compatible(
            &FieldType::Text,
            &FieldType::Integer
        ));
        let old = FieldType::Enum {
            values: vec!["Bronze".into(), "Silver".into()],
        };
        let wider = FieldType::Enum {
            values: vec!["Bronze".into(), "Silver".into(), "Gold".into()],
        };
        let narrower = FieldType::Enum {
            values: vec!["Bronze".into()],
        };
        assert!(custom_types_compatible(&old, &wider));
        assert!(!custom_types_compatible(&old, &narrower));
    }

    #[test]
    fn merge_fills_ui_label_from_field_label() {
        let mut extra = FieldDef::string("gst_number").custom();
        extra.label = "GST Number".into();
        extra.ui.label.clear();
        let merged = merge_custom_fields(&customer(), &[extra]).unwrap();
        assert_eq!(
            merged.get_field("gst_number").unwrap().ui.label,
            "GST Number"
        );
        assert_eq!(
            merged
                .to_ui_meta()
                .fields
                .iter()
                .find(|f| f.name == "gst_number")
                .unwrap()
                .label,
            "GST Number"
        );
    }

    #[test]
    fn base_entity_unchanged_without_extensions() {
        let base = customer();
        let merged = merge_custom_fields(&base, &[]).unwrap();
        assert_eq!(merged.fields.len(), base.fields.len());
    }
}
