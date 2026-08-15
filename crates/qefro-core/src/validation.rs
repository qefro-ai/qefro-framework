use crate::error::{FieldError, QefroResult};
use crate::field::{FieldDef, FieldType};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValidationRules {
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub email: bool,
}

const EMAIL_RE: &str = r"(?i)^[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}$";

/// Validate a JSON object against entity field metadata. Unique checks are
/// performed later by the database layer because they require I/O.
pub fn validate_record(fields: &[FieldDef], record: &Value, partial: bool) -> QefroResult<()> {
    let obj = record
        .as_object()
        .ok_or_else(|| crate::error::QefroError::bad_request("record must be a JSON object"))?;

    let mut errors = Vec::new();

    for field in fields {
        if field.system {
            continue;
        }
        if !field.stores_column() {
            continue;
        }
        let value = obj.get(&field.name);
        match value {
            None | Some(Value::Null) => {
                if field.required && !partial {
                    errors.push(FieldError::new(
                        &field.name,
                        "required",
                        format!("{} is required", field.label),
                    ));
                }
            }
            Some(v) => {
                if let Some(err) = field.type_error(v) {
                    errors.push(err);
                    continue;
                }
                errors.extend(validate_value(field, v));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(crate::error::QefroError::validation(errors))
    }
}

fn validate_value(field: &FieldDef, value: &Value) -> Vec<FieldError> {
    let mut errors = Vec::new();
    let rules = &field.validation;

    if let Some(s) = value.as_str() {
        if let Some(min) = rules.min_length {
            if s.chars().count() < min {
                errors.push(FieldError::new(
                    &field.name,
                    "min_length",
                    format!("must be at least {min} characters"),
                ));
            }
        }
        if let Some(max) = rules.max_length {
            if s.chars().count() > max {
                errors.push(FieldError::new(
                    &field.name,
                    "max_length",
                    format!("must be at most {max} characters"),
                ));
            }
        }
        if rules.email && !Regex::new(EMAIL_RE).expect("email regex").is_match(s) {
            errors.push(FieldError::new(
                &field.name,
                "email",
                "invalid email address",
            ));
        }
        if let Some(pattern) = &rules.regex {
            match Regex::new(pattern) {
                Ok(re) if !re.is_match(s) => {
                    errors.push(FieldError::new(
                        &field.name,
                        "regex",
                        "does not match required pattern",
                    ));
                }
                Err(_) => {
                    errors.push(FieldError::new(
                        &field.name,
                        "regex",
                        "invalid validation pattern configured",
                    ));
                }
                _ => {}
            }
        }
        if let FieldType::Enum { values } = &field.field_type {
            if !values.iter().any(|v| v == s) {
                errors.push(FieldError::new(
                    &field.name,
                    "enum",
                    format!("must be one of: {}", values.join(", ")),
                ));
            }
        }
        if matches!(field.field_type, FieldType::Uuid | FieldType::Relation)
            && uuid::Uuid::parse_str(s).is_err()
        {
            errors.push(FieldError::new(&field.name, "uuid", "invalid UUID"));
        }
    }

    let numeric = value.as_f64();
    if let Some(n) = numeric {
        if let Some(min) = rules.min {
            if n < min {
                errors.push(FieldError::new(
                    &field.name,
                    "min",
                    format!("must be >= {min}"),
                ));
            }
        }
        if let Some(max) = rules.max {
            if n > max {
                errors.push(FieldError::new(
                    &field.name,
                    "max",
                    format!("must be <= {max}"),
                ));
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;
    use serde_json::json;

    #[test]
    fn required_and_email() {
        let fields = vec![
            FieldDef::string("name").required().min_length(2),
            FieldDef::string("email").required().email(),
            FieldDef::string("phone").nullable(),
        ];
        let err =
            validate_record(&fields, &json!({"name": "A", "email": "nope"}), false).unwrap_err();
        match err {
            crate::error::QefroError::Validation { fields, .. } => {
                assert!(fields.iter().any(|e| e.code == "min_length"));
                assert!(fields.iter().any(|e| e.code == "email"));
            }
            other => panic!("unexpected {other:?}"),
        }
        validate_record(
            &fields,
            &json!({"name": "Ada", "email": "ada@example.com"}),
            false,
        )
        .unwrap();
    }

    #[test]
    fn enum_and_range() {
        let fields = vec![
            FieldDef::enum_values("status", vec!["open", "closed"]).required(),
            FieldDef::integer("qty").required().min(1.0).max(10.0),
        ];
        assert!(validate_record(&fields, &json!({"status": "nope", "qty": 0}), false).is_err());
        validate_record(&fields, &json!({"status": "open", "qty": 3}), false).unwrap();
    }

    #[test]
    fn partial_skips_missing_required() {
        let fields = vec![FieldDef::string("name").required()];
        validate_record(&fields, &json!({}), true).unwrap();
        assert!(validate_record(&fields, &json!({}), false).is_err());
    }
}
