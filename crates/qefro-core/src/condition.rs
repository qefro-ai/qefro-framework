//! Safe declarative conditions for validation and automation.
//!
//! No Rust, JavaScript, SQL, or arbitrary expressions.

use crate::error::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A composable predicate over a JSON object (record or event view).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_equals: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "in")]
    pub in_values: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_in: Option<Vec<Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "gt",
        alias = "greater_than"
    )]
    pub greater_than: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "lt",
        alias = "less_than"
    )]
    pub less_than: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "gte",
        alias = "greater_or_equal"
    )]
    pub greater_or_equal: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "lte",
        alias = "less_or_equal"
    )]
    pub less_or_equal: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_empty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_not_empty: Option<bool>,
    /// Named operator when using `{ field, rule, value }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<Condition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any: Option<Vec<Condition>>,
}

impl Condition {
    pub fn field_equals(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            field: Some(field.into()),
            equals: Some(value.into()),
            ..Default::default()
        }
    }

    pub fn all(parts: Vec<Condition>) -> Self {
        Self {
            all: Some(parts),
            ..Default::default()
        }
    }

    pub fn any(parts: Vec<Condition>) -> Self {
        Self {
            any: Some(parts),
            ..Default::default()
        }
    }

    pub fn matches(&self, record: &Value) -> bool {
        if let Some(all) = &self.all {
            if !all.iter().all(|c| c.matches(record)) {
                return false;
            }
        }
        if let Some(any) = &self.any {
            if any.is_empty() || !any.iter().any(|c| c.matches(record)) {
                return false;
            }
        }
        let Some(field) = &self.field else {
            return self.all.is_some() || self.any.is_some();
        };
        let length_holder;
        let actual = if let Some(base) = field
            .strip_suffix(".length")
            .or_else(|| field.strip_suffix(".len"))
        {
            match lookup(record, base) {
                Value::Array(a) => {
                    length_holder = Value::from(a.len() as i64);
                    &length_holder
                }
                other if other.is_null() => {
                    length_holder = Value::from(0);
                    &length_holder
                }
                _ => lookup(record, field),
            }
        } else {
            lookup(record, field)
        };
        let mut ok = true;
        if let Some(expected) = self.resolved_equals() {
            ok &= values_equal(actual, expected);
        }
        if let Some(expected) = &self.not_equals {
            ok &= !values_equal(actual, expected);
        }
        if let Some(needle) = &self.contains {
            ok &= value_contains(actual, needle);
        }
        if let Some(list) = &self.in_values {
            ok &= list.iter().any(|v| values_equal(actual, v));
        }
        if let Some(list) = &self.not_in {
            ok &= !list.iter().any(|v| values_equal(actual, v));
        }
        if let Some(rhs) = self.resolved_cmp("greater_than") {
            ok &= cmp_ord(actual, rhs) == Some(std::cmp::Ordering::Greater);
        }
        if let Some(rhs) = self.resolved_cmp("less_than") {
            ok &= cmp_ord(actual, rhs) == Some(std::cmp::Ordering::Less);
        }
        if let Some(rhs) = self.resolved_cmp("greater_or_equal") {
            ok &= matches!(
                cmp_ord(actual, rhs),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            );
        }
        if let Some(rhs) = self.resolved_cmp("less_or_equal") {
            ok &= matches!(
                cmp_ord(actual, rhs),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            );
        }
        if self.is_empty == Some(true) {
            ok &= is_empty(actual);
        }
        if self.is_not_empty == Some(true) || self.is_empty == Some(false) {
            ok &= !is_empty(actual);
        }
        if let Some(rule) = self.rule.as_deref() {
            ok &= match normalize_op(rule) {
                "equals" => values_equal(actual, self.value.as_ref().unwrap_or(&Value::Null)),
                "not_equals" => !values_equal(actual, self.value.as_ref().unwrap_or(&Value::Null)),
                "contains" => value_contains(actual, self.value.as_ref().unwrap_or(&Value::Null)),
                "is_empty" => is_empty(actual),
                "is_not_empty" => !is_empty(actual),
                "greater_than" => {
                    cmp_ord(actual, self.value.as_ref().unwrap_or(&Value::Null))
                        == Some(std::cmp::Ordering::Greater)
                }
                "less_than" => {
                    cmp_ord(actual, self.value.as_ref().unwrap_or(&Value::Null))
                        == Some(std::cmp::Ordering::Less)
                }
                "greater_or_equal" => matches!(
                    cmp_ord(actual, self.value.as_ref().unwrap_or(&Value::Null)),
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                ),
                "less_or_equal" => matches!(
                    cmp_ord(actual, self.value.as_ref().unwrap_or(&Value::Null)),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                ),
                "in" => self
                    .value
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .map(|list| list.iter().any(|v| values_equal(actual, v)))
                    .unwrap_or(false),
                "not_in" => self
                    .value
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .map(|list| !list.iter().any(|v| values_equal(actual, v)))
                    .unwrap_or(false),
                other => {
                    tracing::warn!(rule = other, "unknown condition rule");
                    false
                }
            };
        }
        ok
    }

    fn resolved_equals(&self) -> Option<&Value> {
        self.equals.as_ref()
    }

    fn resolved_cmp(&self, which: &str) -> Option<&Value> {
        match which {
            "greater_than" => self.greater_than.as_ref(),
            "less_than" => self.less_than.as_ref(),
            "greater_or_equal" => self.greater_or_equal.as_ref(),
            "less_or_equal" => self.less_or_equal.as_ref(),
            _ => None,
        }
    }
}

pub fn normalize_op(op: &str) -> &str {
    match op {
        "gt" | "greater_than" => "greater_than",
        "lt" | "less_than" => "less_than",
        "gte" | "greater_or_equal" => "greater_or_equal",
        "lte" | "less_or_equal" => "less_or_equal",
        other => other,
    }
}

pub fn lookup<'a>(record: &'a Value, path: &str) -> &'a Value {
    let mut cur = record;
    let mut used_expanded = false;
    for part in path.split('.') {
        match cur {
            Value::Object(map) => {
                if let Some(v) = map.get(part) {
                    cur = v;
                    continue;
                }
                if !used_expanded {
                    if let Some(v) = map
                        .get("_expanded")
                        .and_then(|e| e.as_object())
                        .and_then(|e| e.get(part))
                    {
                        used_expanded = true;
                        cur = v;
                        continue;
                    }
                }
                return &Value::Null;
            }
            _ => return &Value::Null,
        }
    }
    cur
}

pub fn values_equal(left: &Value, right: &Value) -> bool {
    if left == right {
        return true;
    }
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::String(a), Value::String(b)) => a.eq_ignore_ascii_case(b),
        (Value::String(a), other) => {
            let b = other
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| other.to_string().trim_matches('"').to_string());
            a.eq_ignore_ascii_case(&b)
        }
        (other, Value::String(b)) => other
            .as_str()
            .map(|a| a.eq_ignore_ascii_case(b))
            .unwrap_or(false),
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => false,
    }
}

fn value_contains(haystack: &Value, needle: &Value) -> bool {
    match haystack {
        Value::String(s) => needle
            .as_str()
            .map(|n| s.to_ascii_lowercase().contains(&n.to_ascii_lowercase()))
            .unwrap_or(false),
        Value::Array(items) => items.iter().any(|v| values_equal(v, needle)),
        _ => false,
    }
}

pub fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

pub fn cmp_ord(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    if let (Some(a), Some(b)) = (as_f64(left), as_f64(right)) {
        return a.partial_cmp(&b);
    }
    match (left, right) {
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::String(a), other) => other.as_str().map(|b| a.as_str().cmp(b)),
        (other, Value::String(b)) => other.as_str().map(|a| a.cmp(b)),
        _ => None,
    }
}

pub fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) => Some(0.0),
        _ => None,
    }
}

pub fn require_object(record: &Value) -> QefroResult<&serde_json::Map<String, Value>> {
    record
        .as_object()
        .ok_or_else(|| QefroError::bad_request("record must be a JSON object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn and_or_and_operators() {
        let rec = json!({ "entity": "Order", "to_state": "Ready", "qty": 3 });
        let all = Condition::all(vec![
            Condition::field_equals("entity", "Order"),
            Condition::field_equals("to_state", "ready"),
        ]);
        assert!(all.matches(&rec));
        let gt = Condition {
            field: Some("qty".into()),
            greater_than: Some(json!(2)),
            ..Default::default()
        };
        assert!(gt.matches(&rec));
        let any = Condition::any(vec![
            Condition::field_equals("entity", "Reservation"),
            Condition::field_equals("entity", "Order"),
        ]);
        assert!(any.matches(&rec));
    }

    #[test]
    fn empty_and_in() {
        let rec = json!({ "name": "", "status": "open" });
        let empty = Condition {
            field: Some("name".into()),
            is_empty: Some(true),
            ..Default::default()
        };
        assert!(empty.matches(&rec));
        let inn = Condition {
            field: Some("status".into()),
            in_values: Some(vec![json!("open"), json!("closed")]),
            ..Default::default()
        };
        assert!(inn.matches(&rec));
    }

    #[test]
    fn relation_and_length_lookups() {
        let nested = json!({
            "customer": { "enabled": true },
            "items": [1, 2]
        });
        assert!(Condition::field_equals("customer.enabled", true).matches(&nested));
        let len = Condition {
            field: Some("items.length".into()),
            greater_than: Some(json!(0)),
            ..Default::default()
        };
        assert!(len.matches(&nested));
        let expanded = json!({ "_expanded": { "customer": { "party_type": "Person" } } });
        assert!(Condition::field_equals("customer.party_type", "Person").matches(&expanded));
    }
}
