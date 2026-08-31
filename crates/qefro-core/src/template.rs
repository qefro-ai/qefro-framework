//! Safe document templates. No JavaScript, SQL, filesystem, or network.
//!
//! Supported:
//! - `{{ path.to.field }}`
//! - `{{ amount | currency }}` / `date` / `time` / `number` / `percent`
//! - `{% for row in items %} ... {% endfor %}`
//! - `{% if path %}` / `{% if path > 0 %}` / `{% endif %}`
//!
//! Missing values render as empty strings. Nested loops are capped.

use crate::error::{QefroError, QefroResult};
use crate::ident::snake_case;
use crate::registry::EntityRegistry;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;

pub const MAX_LOOP: usize = 200;
pub const MAX_DEPTH: usize = 8;
pub const MAX_TEMPLATE_CHARS: usize = 32_768;

const BANNED_KEYS: &[&str] = &[
    "javascript",
    "script",
    "html",
    "sql",
    "query_sql",
    "raw_sql",
    "onclick",
    "href",
    "src",
    "url",
    "endpoint",
    "handler",
    "code",
    "eval",
    "exec",
    "filesystem",
    "env",
    "secret",
    "password",
];

#[derive(Debug, Clone)]
pub struct FormatOpts {
    pub currency: String,
    pub locale: String,
    pub date_format: String,
}

impl Default for FormatOpts {
    fn default() -> Self {
        Self {
            currency: "USD".into(),
            locale: "en-US".into(),
            date_format: "YYYY-MM-DD".into(),
        }
    }
}

pub fn reject_unsafe_template(src: &str) -> QefroResult<()> {
    if src.len() > MAX_TEMPLATE_CHARS {
        return Err(QefroError::bad_request("template is too large"));
    }
    let lower = src.to_ascii_lowercase();
    if lower.contains("<script")
        || lower.contains("javascript:")
        || lower.contains("onerror=")
        || lower.contains("onload=")
    {
        return Err(QefroError::bad_request(
            "templates reject custom JavaScript or HTML",
        ));
    }
    if lower.contains("select ")
        || lower.contains("insert ")
        || lower.contains("update ")
        || lower.contains("delete ")
        || lower.contains("drop ")
        || lower.contains(" union ")
    {
        return Err(QefroError::bad_request("templates reject custom SQL"));
    }
    if lower.contains("://") || lower.contains("std::") || lower.contains("/etc/") {
        return Err(QefroError::bad_request(
            "templates reject filesystem, network, or code paths",
        ));
    }
    Ok(())
}

pub fn reject_unsafe_print_payload(payload: &Value) -> QefroResult<()> {
    walk_reject(payload)
}

fn walk_reject(value: &Value) -> QefroResult<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let key_l = key.to_ascii_lowercase();
                if BANNED_KEYS.contains(&key_l.as_str()) {
                    return Err(QefroError::bad_request(format!(
                        "document template rejects '{key}'"
                    )));
                }
                walk_reject(child)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                walk_reject(item)?;
            }
            Ok(())
        }
        Value::String(s) => reject_unsafe_template(s),
        _ => Ok(()),
    }
}

/// Interpolate `src` against `ctx`. Loops and conditions are declarative only.
pub fn render_template(src: &str, ctx: &Value, opts: &FormatOpts) -> QefroResult<String> {
    reject_unsafe_template(src)?;
    render_inner(src, ctx, opts, 0)
}

fn render_inner(src: &str, ctx: &Value, opts: &FormatOpts, depth: usize) -> QefroResult<String> {
    if depth > MAX_DEPTH {
        return Err(QefroError::bad_request("template nesting is too deep"));
    }
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while !rest.is_empty() {
        if let Some(idx) = rest.find("{%") {
            out.push_str(&interpolate(&rest[..idx], ctx, opts)?);
            rest = &rest[idx..];
            if rest.starts_with("{% for ") {
                let (body, after) = split_block(rest, "for", "endfor")?;
                let header = tag_header(rest, "for")?;
                let (alias, path) = parse_for(&header)?;
                let items = resolve_path(ctx, &path);
                let rows = items.as_array().cloned().unwrap_or_default();
                for row in rows.iter().take(MAX_LOOP) {
                    let mut nested = ctx.clone();
                    if let Some(obj) = nested.as_object_mut() {
                        obj.insert(alias.clone(), row.clone());
                    }
                    out.push_str(&render_inner(body, &nested, opts, depth + 1)?);
                }
                rest = after;
                continue;
            }
            if rest.starts_with("{% if ") {
                let (body, after) = split_block(rest, "if", "endif")?;
                let header = tag_header(rest, "if")?;
                if eval_condition(ctx, &header)? {
                    out.push_str(&render_inner(body, ctx, opts, depth + 1)?);
                }
                rest = after;
                continue;
            }
            return Err(QefroError::bad_request("invalid template tag"));
        }
        out.push_str(&interpolate(rest, ctx, opts)?);
        break;
    }
    Ok(out)
}

fn interpolate(src: &str, ctx: &Value, opts: &FormatOpts) -> QefroResult<String> {
    let mut out = String::new();
    let mut rest = src;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err(QefroError::bad_request("unclosed template expression"));
        };
        let expr = after[..end].trim();
        if expr.is_empty() {
            return Err(QefroError::bad_request("empty template expression"));
        }
        out.push_str(&eval_expr(ctx, expr, opts));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn eval_expr(ctx: &Value, expr: &str, opts: &FormatOpts) -> String {
    let (path, filter) = match expr.split_once('|') {
        Some((p, f)) => (p.trim(), Some(f.trim().to_ascii_lowercase())),
        None => (expr.trim(), None),
    };
    let value = resolve_path(ctx, path);
    apply_filter(&value, filter.as_deref(), opts)
}

fn apply_filter(value: &Value, filter: Option<&str>, opts: &FormatOpts) -> String {
    match filter {
        Some("currency") => format_currency(value, &opts.currency, &opts.locale),
        Some("number") => format_number(value),
        Some("percent") => format_percent(value),
        Some("date") => format_date(value, &opts.date_format),
        Some("time") => format_time(value),
        Some(_) => display_value(value),
        None => display_value(value),
    }
}

pub fn display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn format_currency(value: &Value, currency: &str, _locale: &str) -> String {
    let Some(n) = as_decimal(value) else {
        return String::new();
    };
    let rounded = n.round_dp(2);
    format!("{currency} {rounded}")
}

fn format_number(value: &Value) -> String {
    as_decimal(value)
        .map(|n| {
            if n == n.trunc() {
                n.trunc().to_string()
            } else {
                n.round_dp(2).to_string()
            }
        })
        .unwrap_or_default()
}

fn format_percent(value: &Value) -> String {
    as_decimal(value)
        .map(|n| format!("{}%", n.round_dp(1)))
        .unwrap_or_default()
}

fn format_date(value: &Value, pattern: &str) -> String {
    let Some(raw) = value.as_str().filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let date = raw.get(..10).unwrap_or(raw);
    if pattern.eq_ignore_ascii_case("DD/MM/YYYY") && date.len() == 10 {
        return format!("{}/{}/{}", &date[8..10], &date[5..7], &date[0..4]);
    }
    if pattern.eq_ignore_ascii_case("MM/DD/YYYY") && date.len() == 10 {
        return format!("{}/{}/{}", &date[5..7], &date[8..10], &date[0..4]);
    }
    date.to_string()
}

fn format_time(value: &Value) -> String {
    let Some(raw) = value.as_str().filter(|s| !s.is_empty()) else {
        return String::new();
    };
    if raw.len() >= 16 {
        return raw[11..16].to_string();
    }
    raw.to_string()
}

fn as_decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::Number(n) => Decimal::from_str(&n.to_string()).ok(),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return None;
            }
            Decimal::from_str(t).ok()
        }
        _ => None,
    }
}

/// Walk JSON using entity-aware aliases (`customer` → `customer_id` / `_expanded` / `customer_name`).
pub fn resolve_path(ctx: &Value, path: &str) -> Value {
    if path.is_empty() {
        return Value::Null;
    }
    let mut cur = ctx.clone();
    for raw_seg in path.split('.') {
        let seg = raw_seg.trim();
        if seg.is_empty() {
            return Value::Null;
        }
        cur = lookup_owned(&cur, seg);
        if cur.is_null() {
            return Value::Null;
        }
    }
    cur
}

fn lookup_owned(cur: &Value, seg: &str) -> Value {
    let Some(obj) = cur.as_object() else {
        return Value::Null;
    };
    if let Some(v) = obj.get(seg).filter(|v| !v.is_null()) {
        return v.clone();
    }
    if seg == "number" {
        for alt in ["doc_no", "name", "code", "title"] {
            if let Some(v) = obj.get(alt).filter(|v| !v.is_null()) {
                return v.clone();
            }
        }
    }
    let id_key = format!("{seg}_id");
    if let Some(expanded) = obj.get("_expanded").and_then(|v| v.as_object()) {
        if let Some(rel) = expanded.get(seg).or_else(|| expanded.get(&id_key)) {
            return rel.clone();
        }
    }
    if let Some(v) = obj.get(&id_key).filter(|v| v.is_object() || v.is_array()) {
        return v.clone();
    }
    if let Some(v) = obj.get(&format!("{seg}_name")).filter(|v| !v.is_null()) {
        return json!({ "name": v, "label": v });
    }
    Value::Null
}

fn eval_condition(ctx: &Value, header: &str) -> QefroResult<bool> {
    let expr = header.trim();
    if expr.is_empty() {
        return Err(QefroError::bad_request("empty condition"));
    }
    for op in [">=", "<=", "!=", "==", ">", "<"] {
        if let Some((left, right)) = expr.split_once(op) {
            let lv = resolve_path(ctx, left.trim());
            let rv = parse_literal(right.trim());
            return Ok(compare(&lv, op, &rv));
        }
    }
    Ok(is_truthy(&resolve_path(ctx, expr)))
}

fn parse_literal(raw: &str) -> Value {
    if raw.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if let Ok(n) = raw.parse::<i64>() {
        return json_num(n);
    }
    if let Ok(n) = raw.parse::<f64>() {
        return serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null);
    }
    let trimmed = raw.trim_matches('"').trim_matches('\'');
    Value::String(trimmed.to_string())
}

fn json_num(n: i64) -> Value {
    Value::Number(n.into())
}

fn compare(left: &Value, op: &str, right: &Value) -> bool {
    if let (Some(l), Some(r)) = (as_decimal(left), as_decimal(right)) {
        return match op {
            ">" => l > r,
            "<" => l < r,
            ">=" => l >= r,
            "<=" => l <= r,
            "==" => l == r,
            "!=" => l != r,
            _ => false,
        };
    }
    let ls = display_value(left);
    let rs = display_value(right);
    match op {
        "==" => ls == rs,
        "!=" => ls != rs,
        _ => false,
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => as_decimal(&Value::Number(n.clone()))
            .map(|d| !d.is_zero())
            .unwrap_or(false),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn parse_for(header: &str) -> QefroResult<(String, String)> {
    let mut parts = header.split_whitespace();
    let alias = parts
        .next()
        .ok_or_else(|| QefroError::bad_request("invalid loop"))?;
    let inn = parts
        .next()
        .ok_or_else(|| QefroError::bad_request("invalid loop"))?;
    if inn != "in" {
        return Err(QefroError::bad_request("invalid loop"));
    }
    let path = parts
        .next()
        .ok_or_else(|| QefroError::bad_request("invalid loop"))?;
    if parts.next().is_some() {
        return Err(QefroError::bad_request("invalid loop"));
    }
    if !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(QefroError::bad_request("invalid loop alias"));
    }
    Ok((alias.to_string(), path.to_string()))
}

fn tag_header<'a>(src: &'a str, kind: &str) -> QefroResult<String> {
    let prefix = format!("{{% {kind} ");
    let rest = src
        .strip_prefix(&prefix)
        .ok_or_else(|| QefroError::bad_request("invalid template tag"))?;
    let end = rest
        .find("%}")
        .ok_or_else(|| QefroError::bad_request("unclosed template tag"))?;
    Ok(rest[..end].trim().to_string())
}

fn split_block<'a>(src: &'a str, open: &str, close: &str) -> QefroResult<(&'a str, &'a str)> {
    let close_tag = format!("{{% {close} %}}");
    let open_tag = format!("{{% {open} ");
    let first_end = src
        .find("%}")
        .ok_or_else(|| QefroError::bad_request("unclosed template tag"))?;
    let body_start = first_end + 2;
    let mut depth = 1usize;
    let mut i = body_start;
    while i < src.len() {
        if src[i..].starts_with(&open_tag) {
            depth += 1;
            i += open_tag.len();
            continue;
        }
        if src[i..].starts_with(&close_tag) {
            depth -= 1;
            if depth == 0 {
                return Ok((&src[body_start..i], &src[i + close_tag.len()..]));
            }
            i += close_tag.len();
            continue;
        }
        i += 1;
    }
    Err(QefroError::bad_request(format!(
        "unclosed {open} template block"
    )))
}

/// Collect field/relation paths referenced by a template.
pub fn template_paths(src: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = src;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("}}") {
            let expr = after[..end].split('|').next().unwrap_or("").trim();
            if !expr.is_empty() {
                paths.push(expr.to_string());
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    rest = src;
    while let Some(start) = rest.find("{% for ") {
        if let Ok(header) = tag_header(&rest[start..], "for") {
            if let Ok((_, path)) = parse_for(&header) {
                paths.push(path);
            }
        }
        rest = &rest[start + 7..];
    }
    rest = src;
    while let Some(start) = rest.find("{% if ") {
        if let Ok(header) = tag_header(&rest[start..], "if") {
            let path = header
                .split(['>', '<', '=', '!'])
                .next()
                .unwrap_or("")
                .trim();
            if !path.is_empty() {
                paths.push(path.to_string());
            }
        }
        rest = &rest[start + 6..];
    }
    paths
}

/// Validate template paths against an entity (and nested relations).
pub fn validate_template_paths(src: &str, entity: &str, registry: &EntityRegistry) -> Vec<String> {
    let mut errors = Vec::new();
    if let Err(err) = reject_unsafe_template(src) {
        errors.push(err.to_string());
        return errors;
    }
    let Some(root) = registry.try_get(entity) else {
        errors.push(format!("unknown entity '{entity}'"));
        return errors;
    };
    for path in template_paths(src) {
        if path.starts_with('_') {
            continue;
        }
        if let Err(err) = validate_path(registry, &root.name, &path) {
            errors.push(err);
        }
    }
    errors
}

fn validate_path(registry: &EntityRegistry, entity: &str, path: &str) -> Result<(), String> {
    let mut current = entity.to_string();
    let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err("invalid template expression".into());
    }
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "number"
            || *seg == snake_case(&current)
            || seg.eq_ignore_ascii_case(current.as_str())
        {
            continue;
        }
        let def = registry
            .try_get(&current)
            .ok_or_else(|| format!("unknown entity '{current}'"))?;
        if def.get_field(seg).is_some() {
            if let Some(rel) = def.get_field(seg).and_then(|f| f.relation.as_ref()) {
                current = rel.target_entity.clone();
            }
            continue;
        }
        if def.get_field(&format!("{seg}_name")).is_some() {
            continue;
        }
        let id_name = format!("{seg}_id");
        if let Some(field) = def.get_field(&id_name) {
            if let Some(rel) = &field.relation {
                current = rel.target_entity.clone();
                continue;
            }
        }
        if def.fields.iter().any(|f| {
            f.is_child_table()
                && (f.name == *seg
                    || f.relation.as_ref().map(|r| r.target_entity.as_str()) == Some(seg))
        }) {
            if let Some(field) = def.get_field(seg) {
                if let Some(rel) = &field.relation {
                    current = rel.target_entity.clone();
                }
            }
            continue;
        }
        if i == 0 && seg.eq_ignore_ascii_case(&def.name) {
            continue;
        }
        return Err(format!("unknown field or relation '{seg}' on '{current}'"));
    }
    Ok(())
}

/// Nest a record under entity aliases for `{{ invoice.total }}` style paths.
pub fn wrap_record(entity_name: &str, record: Value, extras: HashMap<String, Value>) -> Value {
    let mut root = record;
    if let Some(obj) = root.as_object_mut() {
        for (k, v) in extras {
            obj.insert(k, v);
        }
        let snapshot = Value::Object(obj.clone());
        obj.insert(snake_case(entity_name), snapshot.clone());
        obj.insert(entity_name.to_ascii_lowercase(), snapshot);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityDef;
    use crate::field::FieldDef;
    use serde_json::json;

    fn opts() -> FormatOpts {
        FormatOpts::default()
    }

    #[test]
    fn interpolates_fields_and_missing_values() {
        let ctx = json!({"doc_no": "INV-1", "customer": {"name": "Ahmed"}});
        let out = render_template(
            "Invoice {{ doc_no }} for {{ customer.name }} {{ customer.phone }}",
            &ctx,
            &opts(),
        )
        .unwrap();
        assert_eq!(out, "Invoice INV-1 for Ahmed ");
        assert!(!out.contains("null"));
        assert!(!out.contains("undefined"));
    }

    #[test]
    fn number_alias_and_currency_filter() {
        let ctx = json!({"doc_no": "INV-10042", "total": "26.00"});
        let out = render_template("{{ number }} {{ total | currency }}", &ctx, &opts()).unwrap();
        assert!(out.contains("INV-10042"));
        assert!(out.contains("USD 26.00"));
    }

    #[test]
    fn loops_and_conditions() {
        let ctx = json!({
            "discount": 2,
            "items": [
                {"product": "Burger", "quantity": 2, "amount": 20},
                {"product": "Coffee", "quantity": 1, "amount": 4}
            ]
        });
        let src = "{% for row in items %}{{ row.product }} {{ row.quantity }}x\n{% endfor %}{% if discount > 0 %}Discount{% endif %}";
        let out = render_template(src, &ctx, &opts()).unwrap();
        assert!(out.contains("Burger 2x"));
        assert!(out.contains("Coffee 1x"));
        assert!(out.contains("Discount"));
    }

    #[test]
    fn rejects_javascript_sql_and_urls() {
        assert!(reject_unsafe_template("<script>alert(1)</script>").is_err());
        assert!(reject_unsafe_template("SELECT * FROM invoices").is_err());
        assert!(reject_unsafe_template("https://evil.example").is_err());
        assert!(render_template("{{ }}", &json!({}), &opts()).is_err());
    }

    #[test]
    fn validates_unknown_field_and_relation() {
        let mut registry = EntityRegistry::new();
        registry
            .register(
                EntityDef::new("Invoice")
                    .field(FieldDef::string("doc_no"))
                    .field(FieldDef::many_to_one("customer_id", "Customer"))
                    .build(),
            )
            .unwrap();
        registry
            .register(
                EntityDef::new("Customer")
                    .field(FieldDef::string("name"))
                    .build(),
            )
            .unwrap();
        let ok = validate_template_paths("{{ customer.name }} {{ number }}", "Invoice", &registry);
        assert!(ok.is_empty(), "{ok:?}");
        let bad = validate_template_paths("{{ missing }}", "Invoice", &registry);
        assert!(bad.iter().any(|e| e.contains("unknown field")));
        let bad_rel = validate_template_paths("{{ customer.bogus }}", "Invoice", &registry);
        assert!(bad_rel.iter().any(|e| e.contains("unknown field")));
    }

    #[test]
    fn customer_name_alias_resolves_without_a_relation_object() {
        let ctx = json!({"customer_name": "Ahmed Khan"});
        let out = render_template("{{ customer.name }}", &ctx, &opts()).unwrap();
        assert_eq!(out, "Ahmed Khan");
    }
}
