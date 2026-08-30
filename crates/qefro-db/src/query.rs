use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use qefro_core::{
    parse_money, quote_ident, EntityDef, FieldDef, FieldType, QefroError, QefroResult,
};
use qefro_search::{Filter, Query, SortDir};
use serde_json::{Map, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

pub fn table_ident(entity: &EntityDef) -> QefroResult<String> {
    quote_ident(&entity.table)
}

pub fn column_ident(entity: &EntityDef, name: &str) -> QefroResult<String> {
    let col = entity
        .get_field(name)
        .map(|f| f.column_name())
        .unwrap_or_else(|| name.to_string());
    if !entity.has_column(&col) && !entity.has_column(name) {
        return Err(QefroError::bad_request(format!(
            "unknown column '{name}' on {}",
            entity.name
        )));
    }
    quote_ident(&col)
}

/// Bind owned values by cloning into the query builder. Used when the Value
/// does not live as long as the builder.
pub fn push_bind_owned(
    qb: &mut QueryBuilder<'_, Postgres>,
    field: Option<&FieldDef>,
    value: &Value,
) {
    match value {
        Value::Null => push_null(qb, field),
        Value::Bool(b) => {
            qb.push_bind(*b);
        }
        Value::Number(n) => {
            if matches!(field.map(|f| &f.field_type), Some(FieldType::Integer)) {
                if let Some(i) = n.as_i64() {
                    qb.push_bind(i);
                    return;
                }
            }
            if matches!(field.map(|f| &f.field_type), Some(FieldType::Decimal)) {
                push_numeric(qb, &decimal_bind(value));
                return;
            }
            if let Some(i) = n.as_i64() {
                qb.push_bind(i);
            } else if let Some(f) = n.as_f64() {
                qb.push_bind(f);
            } else {
                qb.push_bind(n.to_string());
            }
        }
        Value::String(s) => match field.map(|f| &f.field_type) {
            Some(FieldType::Uuid) | Some(FieldType::Relation) => match Uuid::parse_str(s) {
                Ok(id) => {
                    qb.push_bind(id);
                }
                Err(_) => {
                    qb.push_bind(s.clone());
                }
            },
            Some(FieldType::DateTime) => match DateTime::parse_from_rfc3339(s) {
                Ok(dt) => {
                    qb.push_bind(dt.with_timezone(&Utc));
                }
                Err(_) => {
                    qb.push_bind(s.clone());
                }
            },
            Some(FieldType::Date) => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                Ok(d) => {
                    qb.push_bind(d);
                }
                Err(_) => {
                    qb.push_bind(s.clone());
                }
            },
            Some(FieldType::Time) => {
                let parsed = NaiveTime::parse_from_str(s, "%H:%M:%S")
                    .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
                    .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S%.f"));
                match parsed {
                    Ok(t) => {
                        qb.push_bind(t);
                    }
                    Err(_) => {
                        qb.push_bind(s.clone());
                    }
                }
            }
            Some(FieldType::Boolean) => {
                qb.push_bind(matches!(s.as_str(), "true" | "1" | "yes"));
            }
            Some(FieldType::Integer) => match s.parse::<i64>() {
                Ok(i) => {
                    qb.push_bind(i);
                }
                Err(_) => {
                    qb.push_bind(s.clone());
                }
            },
            Some(FieldType::Decimal) => {
                push_numeric(qb, &decimal_bind(value));
            }
            Some(FieldType::Json) => {
                qb.push_bind(sqlx::types::Json(value.clone()));
            }
            _ => {
                if let Ok(id) = Uuid::parse_str(s) {
                    qb.push_bind(id);
                } else {
                    qb.push_bind(s.clone());
                }
            }
        },
        Value::Array(_) | Value::Object(_) => {
            qb.push_bind(sqlx::types::Json(value.clone()));
        }
    }
}

fn decimal_bind(value: &Value) -> String {
    parse_money(value)
        .map(|d| d.normalize().to_string())
        .unwrap_or_else(|_| match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => "0".into(),
        })
}

/// Bind decimals as `NUMERIC`, not `float8` or `text`. `float8` overflows
/// `NUMERIC(18,6)` for values such as `0.1`; raw `text` is the wrong OID.
fn push_numeric(qb: &mut QueryBuilder<'_, Postgres>, digits: &str) {
    qb.push("CAST(");
    qb.push_bind(digits.to_string());
    qb.push(" AS NUMERIC)");
}

fn push_null(qb: &mut QueryBuilder<'_, Postgres>, field: Option<&FieldDef>) {
    match field.map(|f| &f.field_type) {
        Some(FieldType::Decimal) => {
            qb.push("CAST(");
            qb.push_bind(Option::<String>::None);
            qb.push(" AS NUMERIC)");
        }
        Some(FieldType::Uuid) | Some(FieldType::Relation) => {
            qb.push_bind(Option::<Uuid>::None);
        }
        Some(FieldType::DateTime) => {
            qb.push_bind(Option::<DateTime<Utc>>::None);
        }
        Some(FieldType::Date) => {
            qb.push_bind(Option::<NaiveDate>::None);
        }
        Some(FieldType::Time) => {
            qb.push_bind(Option::<NaiveTime>::None);
        }
        Some(FieldType::Boolean) => {
            qb.push_bind(Option::<bool>::None);
        }
        Some(FieldType::Integer) => {
            qb.push_bind(Option::<i64>::None);
        }
        _ => {
            qb.push_bind(Option::<String>::None);
        }
    }
}

pub fn apply_filters(
    qb: &mut QueryBuilder<'_, Postgres>,
    entity: &EntityDef,
    tenant_id: Option<Uuid>,
    query: &Query,
) -> QefroResult<()> {
    qb.push(" WHERE 1=1");
    if entity.tenant_owned {
        let tenant_id = tenant_id
            .ok_or_else(|| QefroError::internal("tenant_id required for tenant-owned entity"))?;
        qb.push(" AND ");
        qb.push(quote_ident("tenant_id")?);
        qb.push(" = ");
        qb.push_bind(tenant_id);
    }
    if entity.soft_delete {
        qb.push(" AND ");
        qb.push(quote_ident("deleted_at")?);
        qb.push(" IS NULL");
    }
    if entity.archives() && !query.include_archived {
        qb.push(" AND ");
        qb.push(quote_ident("archived_at")?);
        qb.push(" IS NULL");
    }
    for filter in &query.filters {
        qb.push(" AND ");
        apply_filter(qb, entity, filter)?;
    }
    if let Some(search) = &query.search {
        if !search.is_empty() {
            let searchable = entity.searchable_fields();
            if !searchable.is_empty() {
                qb.push(" AND (");
                let mut clause = 0usize;
                for field in &searchable {
                    if field.relation.is_some() {
                        continue;
                    }
                    if clause > 0 {
                        qb.push(" OR ");
                    }
                    qb.push(quote_ident(&field.column_name())?);
                    if field.search_exact {
                        qb.push("::text ILIKE ");
                        qb.push_bind(search.clone());
                    } else {
                        qb.push("::text ILIKE ");
                        qb.push_bind(format!("%{search}%"));
                    }
                    clause += 1;
                }
                if clause == 0 {
                    qb.push("TRUE");
                }
                qb.push(")");
            }
        }
    }
    Ok(())
}

fn apply_filter(
    qb: &mut QueryBuilder<'_, Postgres>,
    entity: &EntityDef,
    filter: &Filter,
) -> QefroResult<()> {
    let field_name = filter.field_name();
    let col = column_ident(entity, field_name)?;
    let field = entity.get_field(field_name);
    match filter {
        Filter::Eq { value, .. } => {
            qb.push(col);
            qb.push(" = ");
            push_bind_owned(qb, field, value);
        }
        Filter::Neq { value, .. } => {
            qb.push(col);
            qb.push(" <> ");
            push_bind_owned(qb, field, value);
        }
        Filter::Contains { value, .. } => {
            qb.push(col);
            qb.push(" ILIKE ");
            qb.push_bind(format!("%{value}%"));
        }
        Filter::StartsWith { value, .. } => {
            qb.push(col);
            qb.push(" ILIKE ");
            qb.push_bind(format!("{value}%"));
        }
        Filter::Gt { value, .. } => {
            qb.push(col);
            qb.push(" > ");
            push_bind_owned(qb, field, value);
        }
        Filter::Gte { value, .. } => {
            qb.push(col);
            qb.push(" >= ");
            push_bind_owned(qb, field, value);
        }
        Filter::Lt { value, .. } => {
            qb.push(col);
            qb.push(" < ");
            push_bind_owned(qb, field, value);
        }
        Filter::Lte { value, .. } => {
            qb.push(col);
            qb.push(" <= ");
            push_bind_owned(qb, field, value);
        }
        Filter::Between { from, to, .. } => {
            qb.push(col);
            qb.push(" BETWEEN ");
            push_bind_owned(qb, field, from);
            qb.push(" AND ");
            push_bind_owned(qb, field, to);
        }
        Filter::In { values, .. } => {
            qb.push(col);
            qb.push(" IN (");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                push_bind_owned(qb, field, v);
            }
            qb.push(")");
        }
        Filter::NotIn { values, .. } => {
            qb.push(col);
            qb.push(" NOT IN (");
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                push_bind_owned(qb, field, v);
            }
            qb.push(")");
        }
        Filter::Empty { .. } => {
            qb.push("(");
            qb.push(col.clone());
            qb.push(" IS NULL OR ");
            qb.push(col);
            qb.push("::text = '')");
        }
        Filter::NotEmpty { .. } => {
            qb.push("(");
            qb.push(col.clone());
            qb.push(" IS NOT NULL AND ");
            qb.push(col);
            qb.push("::text <> '')");
        }
    }
    Ok(())
}

pub fn apply_sort(
    qb: &mut QueryBuilder<'_, Postgres>,
    entity: &EntityDef,
    query: &Query,
) -> QefroResult<()> {
    if query.sort.is_empty() {
        qb.push(" ORDER BY ");
        qb.push(quote_ident("created_at")?);
        qb.push(" DESC");
        return Ok(());
    }
    qb.push(" ORDER BY ");
    for (i, sort) in query.sort.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(column_ident(entity, &sort.field)?);
        match sort.dir {
            SortDir::Asc => qb.push(" ASC"),
            SortDir::Desc => qb.push(" DESC"),
        };
    }
    Ok(())
}

pub fn strip_system_writes(entity: &EntityDef, obj: &mut Map<String, Value>) {
    for key in [
        "id",
        "tenant_id",
        "created_at",
        "updated_at",
        "deleted_at",
        "created_by",
        "updated_by",
    ] {
        obj.remove(key);
    }
    if let Some(wf) = &entity.workflow {
        let _ = wf;
        // Status is workflow-managed on update; create still allows initial default.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qefro_core::FieldDef;
    use qefro_search::parse_query;

    #[test]
    fn decimal_values_are_bound_as_numeric_not_float() {
        let entity = EntityDef::new("Line")
            .field(FieldDef::currency("debit"))
            .build();
        let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO t (debit) VALUES (");
        push_bind_owned(
            &mut qb,
            entity.get_field("debit"),
            &serde_json::json!("100.00"),
        );
        qb.push(")");
        let sql = qb.sql();
        assert!(sql.contains("CAST("), "{sql}");
        assert!(sql.contains("AS NUMERIC"), "{sql}");
    }

    #[test]
    fn query_sql_never_inlines_search_text() {
        let entity = EntityDef::new("Customer")
            .field(FieldDef::string("name").searchable())
            .field(FieldDef::string("email"))
            .build();
        let raw = vec![
            ("search".into(), "ahmed'; drop table customers;--".into()),
            ("sort".into(), "-created_at".into()),
        ];
        let q = parse_query(&entity, &raw).unwrap();
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(table_ident(&entity).unwrap());
        qb.push(" t");
        apply_filters(&mut qb, &entity, Some(Uuid::nil()), &q).unwrap();
        apply_sort(&mut qb, &entity, &q).unwrap();
        let sql = qb.sql();
        assert!(sql.contains("ILIKE"));
        assert!(!sql.contains("drop table"));
        assert!(sql.contains("$"));
    }
}
