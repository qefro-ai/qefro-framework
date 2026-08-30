use crate::condition::{as_f64, cmp_ord, is_empty, lookup};
use crate::error::{FieldError, QefroResult};
use crate::field::{FieldDef, FieldType};
use chrono::DateTime;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValidationRules {
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    /// Inclusive lower bound (`greater_or_equal` / `min`).
    #[serde(default)]
    pub min: Option<f64>,
    /// Inclusive upper bound (`less_or_equal` / `max`).
    #[serde(default)]
    pub max: Option<f64>,
    /// Exclusive lower bound (`greater_than`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater_than: Option<f64>,
    /// Exclusive upper bound (`less_than`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub less_than: Option<f64>,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub email: bool,
    #[serde(default)]
    pub phone: bool,
    #[serde(default)]
    pub url: bool,
    #[serde(default)]
    pub color: bool,
}

/// Entity-level declarative rule. Complements per-field [`ValidationRules`].
/// `UiWhen` remains presentation-only and is not evaluated here.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValidationRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenClause>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<CompareClause>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenClause {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_equals: Option<Value>,
}

impl WhenClause {
    pub fn matches(&self, record: &Value) -> bool {
        let actual = lookup(record, &self.field);
        if let Some(eq) = &self.equals {
            return crate::condition::values_equal(actual, eq);
        }
        if let Some(neq) = &self.not_equals {
            return !crate::condition::values_equal(actual, neq);
        }
        !is_empty(actual)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CompareClause {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater_than: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub less_than: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater_or_equal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub less_or_equal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
}

const EMAIL_RE: &str = r"(?i)^[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}$";
const PHONE_RE: &str = r"^\+?[0-9][0-9\s\-()]{6,20}$";
const URL_RE: &str = r"(?i)^https?://[^\s]+$";
const COLOR_RE: &str = r"(?i)^#([0-9A-F]{3}|[0-9A-F]{6}|[0-9A-F]{8})$|^rgb(a)?\(";
const TIME_RE: &str = r"^([01]?\d|2[0-3]):[0-5]\d(:[0-5]\d)?$";
const DATE_RE: &str = r"^\d{4}-\d{2}-\d{2}$";

/// Validate a JSON object against entity field metadata. Unique checks are
/// performed later by the database layer because they require I/O.
pub fn validate_record(fields: &[FieldDef], record: &Value, partial: bool) -> QefroResult<()> {
    let obj = record
        .as_object()
        .ok_or_else(|| crate::error::QefroError::bad_request("record must be a JSON object"))?;

    let mut errors = Vec::new();

    for field in fields {
        if field.system || field.computed || field.is_child_table() {
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
        if rules.phone && !Regex::new(PHONE_RE).expect("phone regex").is_match(s) {
            errors.push(FieldError::new(
                &field.name,
                "phone",
                "invalid phone number",
            ));
        }
        if rules.url && !Regex::new(URL_RE).expect("url regex").is_match(s) {
            errors.push(FieldError::new(&field.name, "url", "invalid URL"));
        }
        if rules.color && !Regex::new(COLOR_RE).expect("color regex").is_match(s) {
            errors.push(FieldError::new(
                &field.name,
                "color",
                "invalid color (use #RGB, #RRGGBB, or rgb())",
            ));
        }
        if matches!(field.field_type, FieldType::Date)
            && !Regex::new(DATE_RE).expect("date regex").is_match(s)
        {
            errors.push(FieldError::new(
                &field.name,
                "date",
                "expected date YYYY-MM-DD",
            ));
        }
        if matches!(field.field_type, FieldType::Time)
            && !Regex::new(TIME_RE).expect("time regex").is_match(s)
        {
            errors.push(FieldError::new(
                &field.name,
                "time",
                "expected time HH:MM or HH:MM:SS",
            ));
        }
        if matches!(field.field_type, FieldType::DateTime)
            && DateTime::parse_from_rfc3339(s).is_err()
            && crate::timezone::canonicalize_datetime(s, "UTC").is_none()
        {
            errors.push(FieldError::new(
                &field.name,
                "datetime",
                "expected RFC3339 datetime",
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
        if let Some(gt) = rules.greater_than {
            if n <= gt {
                errors.push(FieldError::new(
                    &field.name,
                    "greater_than",
                    format!("must be greater than {gt}"),
                ));
            }
        }
        if let Some(lt) = rules.less_than {
            if n >= lt {
                errors.push(FieldError::new(
                    &field.name,
                    "less_than",
                    format!("must be less than {lt}"),
                ));
            }
        }
    }

    errors
}

/// Apply entity-level declarative rules. `exists` is skipped (needs I/O).
pub fn apply_entity_rules(
    fields: &[FieldDef],
    rules: &[ValidationRule],
    record: &Value,
    partial: bool,
) -> QefroResult<()> {
    let mut errors = Vec::new();
    for rule in rules {
        if let Some(when) = &rule.when {
            if !when.matches(record) {
                continue;
            }
        }
        if let Some(compare) = &rule.compare {
            errors.extend(eval_compare(compare, record));
        }
        for name in &rule.require {
            if partial && lookup(record, name).is_null() && !record.get(name).is_some() {
                continue;
            }
            if is_empty(lookup(record, name)) {
                let label = fields
                    .iter()
                    .find(|f| f.name == *name)
                    .map(|f| f.label.clone())
                    .unwrap_or_else(|| name.clone());
                errors.push(FieldError::new(
                    name,
                    "required",
                    format!("{label} is required"),
                ));
            }
        }
        let Some(field_name) = rule.field.as_deref() else {
            continue;
        };
        let actual = lookup(record, field_name);
        if rule.rule.as_deref() == Some("exists") {
            continue;
        }
        if let Some(op) = rule.rule.as_deref() {
            errors.extend(eval_named_rule(field_name, op, rule, actual, fields));
        }
        if let (Some(min), Some(n)) = (rule.min.or(range_min(rule)), as_f64(actual)) {
            if n < min {
                errors.push(FieldError::new(
                    field_name,
                    "min",
                    format!("must be >= {min}"),
                ));
            }
        }
        if let (Some(max), Some(n)) = (rule.max.or(range_max(rule)), as_f64(actual)) {
            if n > max {
                errors.push(FieldError::new(
                    field_name,
                    "max",
                    format!("must be <= {max}"),
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(crate::error::QefroError::validation(errors))
    }
}

fn range_min(rule: &ValidationRule) -> Option<f64> {
    if rule.rule.as_deref() == Some("range") {
        rule.min.or_else(|| rule.value.as_ref().and_then(as_f64))
    } else {
        None
    }
}

fn range_max(rule: &ValidationRule) -> Option<f64> {
    if rule.rule.as_deref() == Some("range") {
        rule.max
    } else {
        None
    }
}

fn eval_named_rule(
    field: &str,
    op: &str,
    rule: &ValidationRule,
    actual: &Value,
    fields: &[FieldDef],
) -> Vec<FieldError> {
    let mut errors = Vec::new();
    let op = crate::condition::normalize_op(op);
    match op {
        "required" => {
            if is_empty(actual) {
                errors.push(FieldError::new(field, "required", format!("{field} is required")));
            }
        }
        "email" | "phone" | "url" | "regex" | "min_length" | "max_length" => {
            if let Some(def) = fields.iter().find(|f| f.name == field) {
                let mut tmp = def.validation.clone();
                match op {
                    "email" => tmp.email = true,
                    "phone" => tmp.phone = true,
                    "url" => tmp.url = true,
                    "regex" => {
                        if let Some(Value::String(p)) = &rule.value {
                            tmp.regex = Some(p.clone());
                        }
                    }
                    "min_length" => {
                        if let Some(n) = rule.value.as_ref().and_then(as_f64) {
                            tmp.min_length = Some(n as usize);
                        }
                    }
                    "max_length" => {
                        if let Some(n) = rule.value.as_ref().and_then(as_f64) {
                            tmp.max_length = Some(n as usize);
                        }
                    }
                    _ => {}
                }
                let mut clone = def.clone();
                clone.validation = tmp;
                errors.extend(validate_value(&clone, actual));
            }
        }
        "greater_than" | "less_than" | "greater_or_equal" | "less_or_equal" => {
            let Some(rhs) = rule.value.as_ref() else {
                return errors;
            };
            let ord = cmp_ord(actual, rhs);
            let ok = match op {
                "greater_than" => ord == Some(std::cmp::Ordering::Greater),
                "less_than" => ord == Some(std::cmp::Ordering::Less),
                "greater_or_equal" => {
                    matches!(ord, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))
                }
                "less_or_equal" => {
                    matches!(ord, Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))
                }
                _ => true,
            };
            if !ok && !actual.is_null() {
                errors.push(FieldError::new(field, op, format!("failed {op} check")));
            }
        }
        "range" => {}
        "exists" => {}
        other => {
            errors.push(FieldError::new(
                field,
                "unknown_rule",
                format!("unknown validation rule '{other}'"),
            ));
        }
    }
    errors
}

fn eval_compare(compare: &CompareClause, record: &Value) -> Vec<FieldError> {
    let mut errors = Vec::new();
    let left = lookup(record, &compare.field);
    let mut check = |other: &str, op: &str, ok: fn(std::cmp::Ordering) -> bool| {
        let right = lookup(record, other);
        if is_empty(left) || is_empty(right) {
            return;
        }
        match cmp_ord(left, right) {
            Some(ord) if ok(ord) => {}
            _ => errors.push(FieldError::new(
                compare.field.clone(),
                op,
                format!("{} must be {op} {other}", compare.field),
            )),
        }
    };
    if let Some(other) = &compare.greater_than {
        check(other, "greater_than", |o| o == std::cmp::Ordering::Greater);
    }
    if let Some(other) = &compare.less_than {
        check(other, "less_than", |o| o == std::cmp::Ordering::Less);
    }
    if let Some(other) = &compare.greater_or_equal {
        check(other, "greater_or_equal", |o| {
            matches!(o, std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        });
    }
    if let Some(other) = &compare.less_or_equal {
        check(other, "less_or_equal", |o| {
            matches!(o, std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        });
    }
    if let Some(other) = &compare.equals {
        if !crate::condition::values_equal(left, lookup(record, other)) && !is_empty(left) {
            errors.push(FieldError::new(
                compare.field.clone(),
                "equals",
                format!("{} must equal {other}", compare.field),
            ));
        }
    }
    errors
}

pub fn existence_rules(rules: &[ValidationRule]) -> Vec<&ValidationRule> {
    rules
        .iter()
        .filter(|r| r.rule.as_deref() == Some("exists") && r.field.is_some())
        .collect()
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

    #[test]
    fn greater_than_is_exclusive() {
        let fields = vec![FieldDef::integer("qty").greater_than(0.0)];
        assert!(validate_record(&fields, &json!({ "qty": 0 }), false).is_err());
        validate_record(&fields, &json!({ "qty": 1 }), false).unwrap();
    }

    #[test]
    fn conditional_require() {
        let fields = vec![
            FieldDef::string("status"),
            FieldDef::string("customer_id").nullable(),
        ];
        let rules = vec![ValidationRule {
            when: Some(WhenClause {
                field: "status".into(),
                equals: Some(json!("confirmed")),
                not_equals: None,
            }),
            require: vec!["customer_id".into()],
            ..Default::default()
        }];
        apply_entity_rules(&fields, &rules, &json!({ "status": "draft" }), false).unwrap();
        assert!(apply_entity_rules(
            &fields,
            &rules,
            &json!({ "status": "confirmed" }),
            false
        )
        .is_err());
        apply_entity_rules(
            &fields,
            &rules,
            &json!({ "status": "confirmed", "customer_id": "abc" }),
            false,
        )
        .unwrap();
    }

    #[test]
    fn cross_field_compare() {
        let fields = vec![FieldDef::date("start_date"), FieldDef::date("end_date")];
        let rules = vec![ValidationRule {
            compare: Some(CompareClause {
                field: "end_date".into(),
                greater_than: Some("start_date".into()),
                ..Default::default()
            }),
            ..Default::default()
        }];
        assert!(apply_entity_rules(
            &fields,
            &rules,
            &json!({ "start_date": "2026-08-02", "end_date": "2026-08-01" }),
            false
        )
        .is_err());
        apply_entity_rules(
            &fields,
            &rules,
            &json!({ "start_date": "2026-08-01", "end_date": "2026-08-02" }),
            false,
        )
        .unwrap();
    }
}
