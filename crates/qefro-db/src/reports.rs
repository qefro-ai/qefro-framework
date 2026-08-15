//! Metadata-driven reports. Filters reuse `qefro-search`; aggregations are
//! allowlisted. Arbitrary SQL is rejected.

use crate::query::{apply_filters, column_ident, table_ident};
use qefro_core::{EntityDef, QefroError, QefroResult, ReportDef};
use qefro_search::{Filter, Query};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

const AGGS: &[&str] = &["COUNT", "SUM", "AVG", "MIN", "MAX"];

pub fn validate_report(entity: &EntityDef, report: &ReportDef) -> QefroResult<()> {
    for name in report.fields.iter().chain(report.group_by.iter()) {
        if entity.get_field(name).is_none() && !entity.has_column(name) {
            return Err(QefroError::forbidden(format!(
                "unknown or unauthorized report field '{name}'"
            )));
        }
        qefro_core::ident::assert_safe_ident(&entity.get_field(name).map(|f| f.column_name()).unwrap_or_else(|| name.clone()))?;
    }
    for (field, agg) in &report.aggregations {
        let agg = agg.to_ascii_uppercase();
        if !AGGS.contains(&agg.as_str()) {
            return Err(QefroError::bad_request(format!(
                "unsupported aggregation '{agg}'"
            )));
        }
        if field.contains(';') || field.contains(' ') || field.contains("--") {
            return Err(QefroError::bad_request("arbitrary SQL is not allowed in reports"));
        }
        if agg != "COUNT" {
            let def = entity.get_field(field).ok_or_else(|| {
                QefroError::forbidden(format!("unknown or unauthorized report field '{field}'"))
            })?;
            if !def.field_type.is_numeric() {
                return Err(QefroError::bad_request(format!(
                    "{agg}({}) is not valid for type {}",
                    field,
                    def.field_type.as_str()
                )));
            }
        } else if field != "*" && entity.get_field(field).is_none() && !entity.has_column(field) {
            return Err(QefroError::forbidden(format!(
                "unknown or unauthorized report field '{field}'"
            )));
        }
    }
    for name in &report.group_by {
        if entity.get_field(name).is_none() && !entity.has_column(name) {
            return Err(QefroError::forbidden(format!(
                "unknown or unauthorized group_by field '{name}'"
            )));
        }
    }
    Ok(())
}

pub fn filters_from_json(entity: &EntityDef, raw: &Value) -> QefroResult<Vec<Filter>> {
    let Some(items) = raw.as_array() else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for item in items {
        if item.get("sql").is_some() || item.get("query").is_some() {
            return Err(QefroError::bad_request(
                "arbitrary SQL is not allowed in report filters",
            ));
        }
        let op = item
            .get("op")
            .or_else(|| item.get("operator"))
            .and_then(|v| v.as_str())
            .unwrap_or("eq");
        let field = item
            .get("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("filter requires field"))?;
        if entity.get_field(field).is_none() && !entity.has_column(field) {
            return Err(QefroError::forbidden(format!(
                "unknown or unauthorized filter field '{field}'"
            )));
        }
        qefro_core::ident::assert_safe_ident(field)?;
        let value = item.get("value").cloned().unwrap_or(Value::Null);
        let filter = match op {
            "eq" | "equals" => Filter::Eq {
                field: field.into(),
                value,
            },
            "neq" | "not_equals" | "not equals" => Filter::Neq {
                field: field.into(),
                value,
            },
            "contains" => Filter::Contains {
                field: field.into(),
                value: value.as_str().unwrap_or("").into(),
            },
            "starts_with" | "starts with" => Filter::StartsWith {
                field: field.into(),
                value: value.as_str().unwrap_or("").into(),
            },
            "gt" | "greater_than" | "greater than" => Filter::Gt {
                field: field.into(),
                value,
            },
            "lt" | "less_than" | "less than" => Filter::Lt {
                field: field.into(),
                value,
            },
            "gte" => Filter::Gte {
                field: field.into(),
                value,
            },
            "lte" => Filter::Lte {
                field: field.into(),
                value,
            },
            "between" => Filter::Between {
                field: field.into(),
                from: item.get("from").cloned().unwrap_or(Value::Null),
                to: item.get("to").cloned().unwrap_or(Value::Null),
            },
            "in" => Filter::In {
                field: field.into(),
                values: item
                    .get("values")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
            },
            "not_in" | "not in" => Filter::NotIn {
                field: field.into(),
                values: item
                    .get("values")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
            },
            "empty" => Filter::Empty {
                field: field.into(),
            },
            "not_empty" | "not empty" => Filter::NotEmpty {
                field: field.into(),
            },
            _ => {
                return Err(QefroError::bad_request(format!(
                    "unsupported report filter '{op}'"
                )))
            }
        };
        out.push(filter);
    }
    Ok(out)
}

pub async fn execute_report(
    pool: &sqlx::PgPool,
    entity: &EntityDef,
    ctx_tenant: Option<Uuid>,
    report: &ReportDef,
    query: &Query,
) -> QefroResult<Vec<Value>> {
    validate_report(entity, report)?;
    let table = table_ident(entity)?;
    let mut qb = QueryBuilder::<Postgres>::new("SELECT ");
    let mut first = true;
    let mut select_fields: Vec<(String, String)> = Vec::new();
    for group in &report.group_by {
        if !first {
            qb.push(", ");
        }
        first = false;
        let ident = column_ident(entity, group)?;
        qb.push(ident);
        qb.push("::text AS ");
        qb.push(quote_alias(group)?);
        select_fields.push((group.clone(), "group".into()));
    }
    let mut agg_pairs: Vec<(String, String)> = report
        .aggregations
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    agg_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    if agg_pairs.is_empty() && report.group_by.is_empty() {
        qb.push("COUNT(*)::float8 AS count");
        select_fields.push(("count".into(), "COUNT".into()));
        first = false;
    }
    for (field, agg) in &agg_pairs {
        if !first {
            qb.push(", ");
        }
        first = false;
        let agg = agg.to_ascii_uppercase();
        match agg.as_str() {
            "COUNT" => {
                if field == "*" {
                    qb.push("COUNT(*)::float8");
                } else {
                    let ident = column_ident(entity, field)?;
                    qb.push("COUNT(");
                    qb.push(ident);
                    qb.push(")::float8");
                }
            }
            "SUM" | "AVG" | "MIN" | "MAX" => {
                let ident = column_ident(entity, field)?;
                qb.push("COALESCE(");
                qb.push(agg.clone());
                qb.push("(");
                qb.push(ident);
                qb.push("), 0)::float8");
            }
            _ => unreachable!(),
        }
        qb.push(" AS ");
        qb.push(quote_alias(field)?);
        select_fields.push((field.clone(), agg));
    }
    if first {
        qb.push("COUNT(*)::float8 AS count");
        select_fields.push(("count".into(), "COUNT".into()));
    }
    qb.push(" FROM ");
    qb.push(&table);
    apply_filters(&mut qb, entity, ctx_tenant, query)?;
    if !report.group_by.is_empty() {
        qb.push(" GROUP BY ");
        for (i, group) in report.group_by.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(column_ident(entity, group)?);
        }
        qb.push(" ORDER BY 1");
    }
    qb.push(" LIMIT 500");
    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        let mut obj = serde_json::Map::new();
        for (i, (name, kind)) in select_fields.iter().enumerate() {
            if kind == "group" {
                let v: Option<String> = sqlx::Row::try_get(&row, i)
                    .map_err(|e| QefroError::database(e.to_string()))?;
                obj.insert(name.clone(), json!(v.unwrap_or_else(|| "(empty)".into())));
            } else {
                let v: f64 = sqlx::Row::try_get(&row, i)
                    .map_err(|e| QefroError::database(e.to_string()))?;
                obj.insert(name.clone(), json!(v));
            }
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

fn quote_alias(name: &str) -> QefroResult<String> {
    qefro_core::quote_ident(name)
}

#[allow(unused_imports)]
use sqlx::Row;
