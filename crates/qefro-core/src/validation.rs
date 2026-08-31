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

impl ValidationRule {
    pub fn require_when(
        when_field: impl Into<String>,
        equals: impl Into<Value>,
        fields: &[&str],
    ) -> Self {
        Self {
            when: Some(WhenClause {
                field: when_field.into(),
                equals: Some(equals.into()),
                not_equals: None,
            }),
            require: fields.iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        }
    }

    pub fn compare(field: impl Into<String>, op: &str, other: impl Into<String>) -> Self {
        let other = other.into();
        let mut compare = CompareClause {
            field: field.into(),
            ..Default::default()
        };
        match crate::condition::normalize_op(op) {
            "greater_than" => compare.greater_than = Some(other),
            "less_than" => compare.less_than = Some(other),
            "greater_or_equal" => compare.greater_or_equal = Some(other),
            "less_or_equal" => compare.less_or_equal = Some(other),
            _ => compare.equals = Some(other),
        }
        Self {
            compare: Some(compare),
            ..Default::default()
        }
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
                } else if !partial {
                    if let Some(when) = &field.required_when {
                        if when.matches(record) {
                            errors.push(
                                FieldError::new(
                                    &field.name,
                                    "required",
                                    format!("{} is required", field.label),
                                )
                                .with_rule("required_when"),
                            );
                        }
                    }
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

/// Drop client-supplied computed values. The server recalculates them.
pub fn strip_computed_fields(fields: &[FieldDef], data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };
    for field in fields {
        if field.computed {
            obj.remove(&field.name);
        }
    }
}

/// Whether a field is readonly for this record (static, computed, or `readonly_when`).
pub fn field_is_readonly(field: &FieldDef, record: &Value) -> bool {
    if field.computed || field.ui.readonly {
        return true;
    }
    field
        .ui
        .readonly_when
        .as_ref()
        .map(|when| when.matches(record))
        .unwrap_or(false)
}

/// Reject mutations of readonly / `readonly_when` fields. Identical values are allowed.
pub fn reject_readonly_writes(
    fields: &[FieldDef],
    current: Option<&Value>,
    patch: &Value,
) -> QefroResult<()> {
    let Some(obj) = patch.as_object() else {
        return Ok(());
    };
    let record = current.unwrap_or(patch);
    let mut errors = Vec::new();
    for key in obj.keys() {
        if key.starts_with('_') {
            continue;
        }
        let Some(field) = fields.iter().find(|f| f.name == *key) else {
            continue;
        };
        if field.system || field.computed {
            continue;
        }
        if !field_is_readonly(field, record) {
            continue;
        }
        let new_val = obj.get(key).unwrap_or(&Value::Null);
        let old_val = current.and_then(|c| c.get(key)).unwrap_or(&Value::Null);
        if current.is_some() && crate::condition::values_equal(new_val, old_val) {
            continue;
        }
        if current.is_none() {
            continue;
        }
        errors.push(
            FieldError::new(
                &field.name,
                "readonly",
                format!("{} cannot be changed", field.label),
            )
            .with_rule("readonly_when"),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(crate::error::QefroError::validation(errors))
    }
}

/// Enforce `required_when` against a (possibly merged) record.
pub fn apply_field_rules(fields: &[FieldDef], record: &Value, partial: bool) -> QefroResult<()> {
    let mut errors = Vec::new();
    for field in fields {
        if field.system || field.computed || field.is_child_table() {
            continue;
        }
        let Some(when) = &field.required_when else {
            continue;
        };
        if !when.matches(record) {
            continue;
        }
        if partial && lookup(record, &field.name).is_null() && !record.get(&field.name).is_some() {
            continue;
        }
        if is_empty(lookup(record, &field.name)) {
            errors.push(
                FieldError::new(
                    &field.name,
                    "required",
                    format!("{} is required", field.label),
                )
                .with_rule("required_when"),
            );
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(crate::error::QefroError::validation(errors))
    }
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
            errors.extend(eval_compare(compare, record, fields));
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
    if let Err(crate::error::QefroError::Validation { fields: extra, .. }) =
        apply_field_rules(fields, record, partial)
    {
        errors.extend(extra);
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
                errors.push(FieldError::new(
                    field,
                    "required",
                    format!("{field} is required"),
                ));
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
                    matches!(
                        ord,
                        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                    )
                }
                "less_or_equal" => {
                    matches!(
                        ord,
                        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                    )
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

fn eval_compare(compare: &CompareClause, record: &Value, fields: &[FieldDef]) -> Vec<FieldError> {
    let mut errors = Vec::new();
    let left = lookup(record, &compare.field);
    let left_label = fields
        .iter()
        .find(|f| f.name == compare.field)
        .map(|f| f.label.as_str())
        .unwrap_or(compare.field.as_str());
    let mut check = |other: &str, op: &str, ok: fn(std::cmp::Ordering) -> bool| {
        let right = lookup(record, other);
        if is_empty(left) || is_empty(right) {
            return;
        }
        let right_label = fields
            .iter()
            .find(|f| f.name == other)
            .map(|f| f.label.as_str())
            .unwrap_or(other);
        let temporal = looks_temporal(left) || looks_temporal(right);
        let code = if temporal { "invalid_range" } else { op };
        let human = match (op, temporal) {
            ("greater_than", true) => "after",
            ("less_than", true) => "before",
            ("greater_or_equal", true) => "on or after",
            ("less_or_equal", true) => "on or before",
            ("greater_than", false) => "greater than",
            ("less_than", false) => "less than",
            ("greater_or_equal", false) => "greater than or equal to",
            ("less_or_equal", false) => "less than or equal to",
            _ => op,
        };
        match cmp_ord(left, right) {
            Some(ord) if ok(ord) => {}
            None => errors.push(
                FieldError::new(
                    compare.field.clone(),
                    "invalid_type",
                    format!("cannot compare {left_label} with {right_label}"),
                )
                .with_rule("compare"),
            ),
            _ => errors.push(
                FieldError::new(
                    compare.field.clone(),
                    code,
                    format!("{left_label} must be {human} {right_label}."),
                )
                .with_rule("compare"),
            ),
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
            errors.push(
                FieldError::new(
                    compare.field.clone(),
                    "equals",
                    format!("{left_label} must equal {other}"),
                )
                .with_rule("compare"),
            );
        }
    }
    errors
}

fn looks_temporal(value: &Value) -> bool {
    value
        .as_str()
        .map(|s| {
            s.len() >= 8
                && (s.contains('-') || s.contains('T') || s.contains(':'))
                && s.chars().any(|c| c.is_ascii_digit())
        })
        .unwrap_or(false)
}

pub fn existence_rules(rules: &[ValidationRule]) -> Vec<&ValidationRule> {
    rules
        .iter()
        .filter(|r| r.rule.as_deref() == Some("exists") && r.field.is_some())
        .collect()
}

/// Human-readable compare line for `qefro inspect`.
pub fn compare_rule_line(rule: &ValidationRule) -> Option<String> {
    let compare = rule.compare.as_ref()?;
    if let Some(other) = &compare.greater_than {
        return Some(format!("{} > {}", compare.field, other));
    }
    if let Some(other) = &compare.less_than {
        return Some(format!("{} < {}", compare.field, other));
    }
    if let Some(other) = &compare.greater_or_equal {
        return Some(format!("{} >= {}", compare.field, other));
    }
    if let Some(other) = &compare.less_or_equal {
        return Some(format!("{} <= {}", compare.field, other));
    }
    if let Some(other) = &compare.equals {
        return Some(format!("{} = {}", compare.field, other));
    }
    Some(format!("{} compared", compare.field))
}

/// Human-readable rule lines for `qefro inspect`. Empty when the field has none.
pub fn field_rule_lines(field: &FieldDef) -> Vec<String> {
    let mut lines = Vec::new();
    if field.required {
        lines.push("required".into());
    }
    if let Some(when) = &field.required_when {
        lines.push(format!(
            "required when {} = {}",
            when.field,
            display_equals(&when.equals)
        ));
    }
    if field.ui.readonly {
        lines.push("readonly".into());
    }
    if let Some(when) = &field.ui.readonly_when {
        lines.push(format!(
            "readonly when {} = {}",
            when.field,
            display_equals(&when.equals)
        ));
    }
    if let Some(when) = &field.ui.visible_when {
        lines.push(format!(
            "visible when {} = {}",
            when.field,
            display_equals(&when.equals)
        ));
    }
    if field.computed {
        if let Some(formula) = &field.formula {
            lines.push(format!("computed: {formula}"));
        } else {
            lines.push("computed".into());
        }
    }
    if let Some(min) = field.validation.min {
        lines.push(format!("validation: >= {min}"));
    }
    if let Some(max) = field.validation.max {
        lines.push(format!("validation: <= {max}"));
    }
    if let Some(gt) = field.validation.greater_than {
        lines.push(format!("validation: > {gt}"));
    }
    if let Some(lt) = field.validation.less_than {
        lines.push(format!("validation: < {lt}"));
    }
    if let Some(default) = &field.default {
        lines.push(format!("default: {}", display_equals(default)));
    }
    if let Some(from) = &field.default_from {
        lines.push(format!("default from {from}"));
    }
    lines
}

fn display_equals(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
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
        assert!(
            apply_entity_rules(&fields, &rules, &json!({ "status": "confirmed" }), false).is_err()
        );
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

    #[test]
    fn required_when_is_enforced() {
        let fields = vec![
            FieldDef::enum_values("contact_method", vec!["email", "phone"]),
            FieldDef::string("email")
                .nullable()
                .required_when("contact_method", json!("email")),
        ];
        apply_entity_rules(&fields, &[], &json!({ "contact_method": "phone" }), false).unwrap();
        let err = apply_entity_rules(&fields, &[], &json!({ "contact_method": "email" }), false)
            .unwrap_err();
        match err {
            crate::error::QefroError::Validation { fields, .. } => {
                assert!(fields
                    .iter()
                    .any(|e| e.field == "email" && e.code == "required"));
            }
            other => panic!("{other:?}"),
        }
        apply_entity_rules(
            &fields,
            &[],
            &json!({ "contact_method": "email", "email": "a@b.co" }),
            false,
        )
        .unwrap();
    }

    #[test]
    fn readonly_when_rejects_mutation() {
        let fields = vec![
            FieldDef::string("status"),
            FieldDef::decimal("amount").readonly_when("status", json!("completed")),
        ];
        let current = json!({ "status": "completed", "amount": 10 });
        let err =
            reject_readonly_writes(&fields, Some(&current), &json!({ "amount": 1 })).unwrap_err();
        match err {
            crate::error::QefroError::Validation { fields, .. } => {
                assert!(fields.iter().any(|e| e.code == "readonly"));
            }
            other => panic!("{other:?}"),
        }
        reject_readonly_writes(
            &fields,
            Some(&json!({ "status": "draft", "amount": 10 })),
            &json!({ "amount": 1 }),
        )
        .unwrap();
        reject_readonly_writes(&fields, Some(&current), &json!({ "amount": 10 })).unwrap();
    }

    #[test]
    fn compare_type_mismatch_is_invalid_type() {
        let fields = vec![FieldDef::integer("qty"), FieldDef::string("name")];
        let rules = vec![ValidationRule::compare("qty", "greater_than", "name")];
        let err = apply_entity_rules(
            &fields,
            &rules,
            &json!({ "qty": 2, "name": "hello" }),
            false,
        )
        .unwrap_err();
        match err {
            crate::error::QefroError::Validation { fields, .. } => {
                assert!(fields.iter().any(|e| e.code == "invalid_type"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn field_rule_lines_cover_categories() {
        let field = FieldDef::decimal("discount")
            .readonly_when("status", json!("completed"))
            .min(0.0)
            .default_value(json!(0));
        let lines = field_rule_lines(&field);
        assert!(lines.iter().any(|l| l.contains("readonly when")));
        assert!(lines.iter().any(|l| l.contains("default")));
        let compare = ValidationRule::compare("end_time", "greater_than", "start_time");
        assert_eq!(
            compare_rule_line(&compare).as_deref(),
            Some("end_time > start_time")
        );
    }

    #[test]
    fn rule_evaluation_is_cheap_without_io() {
        let fields = vec![
            FieldDef::integer("qty").greater_than(0.0),
            FieldDef::string("email")
                .nullable()
                .required_when("contact_method", json!("email")),
            FieldDef::string("contact_method"),
            FieldDef::date("start_date"),
            FieldDef::date("end_date"),
        ];
        let rules = vec![ValidationRule::compare(
            "end_date",
            "greater_or_equal",
            "start_date",
        )];
        let record = json!({
            "qty": 2,
            "contact_method": "phone",
            "start_date": "2026-01-01",
            "end_date": "2026-01-02"
        });
        let start = std::time::Instant::now();
        for _ in 0..200 {
            validate_record(&fields, &record, false).unwrap();
            apply_entity_rules(&fields, &rules, &record, false).unwrap();
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 2_000,
            "rule evaluation took {elapsed:?}"
        );
    }
}
