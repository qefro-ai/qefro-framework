use crate::query::{
    apply_filters, apply_sort, column_ident, push_bind_owned, strip_system_writes, table_ident,
};
use chrono::Utc;
use qefro_core::{quote_ident, EntityDef, OpContext, QefroError, QefroResult};
use qefro_search::Query;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub items: Vec<Value>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

pub struct EntityRepository {
    pool: PgPool,
}

impl EntityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn get(&self, entity: &EntityDef, ctx: &OpContext, id: Uuid) -> QefroResult<Value> {
        let table = table_ident(entity)?;
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(table);
        qb.push(" t WHERE ");
        qb.push(quote_ident("id")?);
        qb.push(" = ");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        let row = qb
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let row = row.ok_or_else(|| QefroError::not_found(format!("{} not found", entity.name)))?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        enforce_tenant(entity, ctx, &value)?;
        Ok(value)
    }

    pub async fn get_singleton(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
    ) -> QefroResult<Option<Value>> {
        let table = table_ident(entity)?;
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(table);
        qb.push(" t WHERE ");
        qb.push(quote_ident("tenant_id")?);
        qb.push(" = ");
        qb.push_bind(ctx.tenant_id);
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        qb.push(" ORDER BY created_at ASC LIMIT 1");
        let row = qb
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        match row {
            Some(row) => {
                let value: Value = row
                    .try_get(0)
                    .map_err(|e| QefroError::database(e.to_string()))?;
                enforce_tenant(entity, ctx, &value)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    pub async fn list(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        query: &Query,
    ) -> QefroResult<Page> {
        let table = table_ident(entity)?;
        let tenant = entity.tenant_owned.then_some(ctx.tenant_id);

        let mut count_qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ");
        count_qb.push(&table);
        apply_filters(&mut count_qb, entity, tenant, query)?;
        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;

        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(&table);
        qb.push(" t");
        apply_filters(&mut qb, entity, tenant, query)?;
        apply_sort(&mut qb, entity, query)?;
        qb.push(" LIMIT ");
        qb.push_bind(query.page_size as i64);
        qb.push(" OFFSET ");
        qb.push_bind(query.offset() as i64);

        let rows = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let mut items = Vec::new();
        for row in rows {
            let mut value: Value = row
                .try_get(0)
                .map_err(|e| QefroError::database(e.to_string()))?;
            enforce_tenant(entity, ctx, &value)?;
            if let Some(fields) = &query.fields {
                value = project_fields(value, fields);
            }
            items.push(value);
        }
        Ok(Page {
            items,
            page: query.page,
            page_size: query.page_size,
            total,
        })
    }

    pub async fn aggregate(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        query: &Query,
        metric: &str,
        field: Option<&str>,
    ) -> QefroResult<f64> {
        let table = table_ident(entity)?;
        let tenant = entity.tenant_owned.then_some(ctx.tenant_id);
        let mut qb = QueryBuilder::<Postgres>::new("SELECT ");
        match metric {
            "sum" | "avg" | "min" | "max" => {
                let name = field.ok_or_else(|| {
                    QefroError::bad_request(format!("dashboard {metric} metric requires a field"))
                })?;
                if let Some(def) = entity.get_field(name) {
                    if !def.field_type.is_numeric() {
                        return Err(QefroError::bad_request(format!(
                            "{metric} is not valid for field '{name}'"
                        )));
                    }
                }
                let ident = column_ident(entity, name)?;
                let agg = match metric {
                    "sum" => "SUM",
                    "avg" => "AVG",
                    "min" => "MIN",
                    _ => "MAX",
                };
                qb.push("COALESCE(");
                qb.push(agg);
                qb.push("(");
                qb.push(ident);
                qb.push("), 0)::float8");
            }
            _ => {
                qb.push("COUNT(*)::float8");
            }
        }
        qb.push(" FROM ");
        qb.push(&table);
        apply_filters(&mut qb, entity, tenant, query)?;
        qb.build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn aggregate_group(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        query: &Query,
        group_by: &str,
    ) -> QefroResult<Vec<Value>> {
        self.aggregate_group_with(entity, ctx, query, group_by, "count", None)
            .await
    }

    pub async fn aggregate_group_with(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        query: &Query,
        group_by: &str,
        metric: &str,
        field: Option<&str>,
    ) -> QefroResult<Vec<Value>> {
        let table = table_ident(entity)?;
        let col = column_ident(entity, group_by)?;
        let tenant = entity.tenant_owned.then_some(ctx.tenant_id);
        let mut qb = QueryBuilder::<Postgres>::new("SELECT ");
        qb.push(col.clone());
        qb.push("::text AS key, ");
        let metric = metric.to_ascii_lowercase();
        match metric.as_str() {
            "sum" | "avg" | "min" | "max" => {
                let name = field.ok_or_else(|| {
                    QefroError::bad_request(format!("{metric} aggregation requires a field"))
                })?;
                let ident = column_ident(entity, name)?;
                let agg = match metric.as_str() {
                    "sum" => "SUM",
                    "avg" => "AVG",
                    "min" => "MIN",
                    _ => "MAX",
                };
                qb.push("COALESCE(");
                qb.push(agg);
                qb.push("(");
                qb.push(ident);
                qb.push("), 0)::float8 AS value FROM ");
            }
            _ => {
                qb.push("COUNT(*)::float8 AS value FROM ");
            }
        }
        qb.push(&table);
        apply_filters(&mut qb, entity, tenant, query)?;
        qb.push(" GROUP BY ");
        qb.push(col);
        qb.push(" ORDER BY value DESC LIMIT 50");
        let rows: Vec<(Option<String>, f64)> = qb
            .build_query_as()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(key, value)| {
                json!({
                    "label": key.unwrap_or_else(|| "(empty)".into()),
                    "value": value,
                })
            })
            .collect())
    }

    pub async fn insert(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        mut data: Value,
    ) -> QefroResult<Value> {
        let obj = data
            .as_object_mut()
            .ok_or_else(|| QefroError::bad_request("record must be a JSON object"))?;
        strip_system_writes(entity, obj);

        let id = Uuid::new_v4();
        let now = Utc::now();

        let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ");
        qb.push(table_ident(entity)?);
        qb.push(" (");
        qb.push(quote_ident("id")?);
        if entity.tenant_owned {
            qb.push(", ");
            qb.push(quote_ident("tenant_id")?);
        }
        qb.push(", ");
        qb.push(quote_ident("created_at")?);
        qb.push(", ");
        qb.push(quote_ident("updated_at")?);
        qb.push(", ");
        qb.push(quote_ident("created_by")?);
        qb.push(", ");
        qb.push(quote_ident("updated_by")?);

        let stored: Vec<_> = entity.stored_fields();
        for field in &stored {
            if obj.contains_key(&field.name) || field.default.is_some() {
                qb.push(", ");
                qb.push(quote_ident(&field.column_name())?);
            }
        }
        qb.push(") VALUES (");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(", ");
            qb.push_bind(ctx.tenant_id);
        }
        qb.push(", ");
        qb.push_bind(now);
        qb.push(", ");
        qb.push_bind(now);
        qb.push(", ");
        qb.push_bind(ctx.user_id);
        qb.push(", ");
        qb.push_bind(ctx.user_id);

        for field in &stored {
            if let Some(value) = obj.get(&field.name) {
                qb.push(", ");
                push_bind_owned(&mut qb, Some(field), value);
            } else if let Some(default) = &field.default {
                qb.push(", ");
                push_bind_owned(&mut qb, Some(field), default);
            }
        }
        qb.push(") RETURNING to_jsonb(");
        qb.push(table_ident(entity)?);
        qb.push(".*)");

        let row = qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map_err(|e| map_db_err(e))?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(value)
    }

    pub async fn update(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        id: Uuid,
        mut patch: Value,
    ) -> QefroResult<Value> {
        let obj = patch
            .as_object_mut()
            .ok_or_else(|| QefroError::bad_request("record must be a JSON object"))?;
        strip_system_writes(entity, obj);
        if obj.is_empty() {
            return self.get(entity, ctx, id).await;
        }

        let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
        qb.push(table_ident(entity)?);
        qb.push(" SET ");
        qb.push(quote_ident("updated_at")?);
        qb.push(" = ");
        qb.push_bind(Utc::now());
        qb.push(", ");
        qb.push(quote_ident("updated_by")?);
        qb.push(" = ");
        qb.push_bind(ctx.user_id);

        for (key, value) in obj.iter() {
            let field = entity
                .get_field(key)
                .ok_or_else(|| QefroError::bad_request(format!("unknown field '{key}'")))?;
            if !field.stores_column() {
                continue;
            }
            qb.push(", ");
            qb.push(column_ident(entity, key)?);
            qb.push(" = ");
            push_bind_owned(&mut qb, Some(field), value);
        }

        qb.push(" WHERE ");
        qb.push(quote_ident("id")?);
        qb.push(" = ");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        qb.push(" RETURNING to_jsonb(");
        qb.push(table_ident(entity)?);
        qb.push(".*)");

        let row = qb
            .build()
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_err)?;
        let row = row.ok_or_else(|| QefroError::not_found(format!("{} not found", entity.name)))?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        enforce_tenant(entity, ctx, &value)?;
        Ok(value)
    }

    pub async fn delete(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<Value> {
        let existing = self.get(entity, ctx, id).await?;
        if entity.soft_delete {
            let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
            qb.push(table_ident(entity)?);
            qb.push(" SET ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" = ");
            qb.push_bind(Utc::now());
            qb.push(", ");
            qb.push(quote_ident("updated_at")?);
            qb.push(" = ");
            qb.push_bind(Utc::now());
            qb.push(" WHERE ");
            qb.push(quote_ident("id")?);
            qb.push(" = ");
            qb.push_bind(id);
            if entity.tenant_owned {
                qb.push(" AND ");
                qb.push(quote_ident("tenant_id")?);
                qb.push(" = ");
                qb.push_bind(ctx.tenant_id);
            }
            qb.build().execute(&self.pool).await.map_err(map_db_err)?;
        } else {
            let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM ");
            qb.push(table_ident(entity)?);
            qb.push(" WHERE ");
            qb.push(quote_ident("id")?);
            qb.push(" = ");
            qb.push_bind(id);
            if entity.tenant_owned {
                qb.push(" AND ");
                qb.push(quote_ident("tenant_id")?);
                qb.push(" = ");
                qb.push_bind(ctx.tenant_id);
            }
            qb.build().execute(&self.pool).await.map_err(map_db_err)?;
        }
        Ok(existing)
    }

    pub async fn exists_unique(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        field: &str,
        value: &Value,
        exclude_id: Option<Uuid>,
    ) -> QefroResult<bool> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ");
        qb.push(table_ident(entity)?);
        qb.push(" WHERE ");
        qb.push(column_ident(entity, field)?);
        qb.push(" = ");
        push_bind_owned(&mut qb, entity.get_field(field), value);
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        if let Some(id) = exclude_id {
            qb.push(" AND ");
            qb.push(quote_ident("id")?);
            qb.push(" <> ");
            qb.push_bind(id);
        }
        let count: i64 = qb
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_err)?;
        Ok(count > 0)
    }

    pub async fn list_by_ids(
        &self,
        entity: &EntityDef,
        ctx: &OpContext,
        ids: &[Uuid],
    ) -> QefroResult<Vec<Value>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let table = table_ident(entity)?;
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(table);
        qb.push(" t WHERE ");
        qb.push(quote_ident("id")?);
        qb.push(" IN (");
        for (i, id) in ids.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push_bind(*id);
        }
        qb.push(")");
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        let rows = qb.build().fetch_all(&self.pool).await.map_err(map_db_err)?;
        let mut items = Vec::new();
        for row in rows {
            let value: Value = row
                .try_get(0)
                .map_err(|e| QefroError::database(e.to_string()))?;
            enforce_tenant(entity, ctx, &value)?;
            items.push(value);
        }
        Ok(items)
    }

    pub async fn get_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        entity: &EntityDef,
        ctx: &OpContext,
        id: Uuid,
        lock: bool,
    ) -> QefroResult<Value> {
        let table = table_ident(entity)?;
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(table);
        qb.push(" t WHERE ");
        qb.push(quote_ident("id")?);
        qb.push(" = ");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        if lock {
            qb.push(" FOR UPDATE");
        }
        let row = qb
            .build()
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let row = row.ok_or_else(|| QefroError::not_found(format!("{} not found", entity.name)))?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        enforce_tenant(entity, ctx, &value)?;
        Ok(value)
    }

    pub async fn list_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        entity: &EntityDef,
        ctx: &OpContext,
        query: &Query,
    ) -> QefroResult<Page> {
        let table = table_ident(entity)?;
        let tenant = entity.tenant_owned.then_some(ctx.tenant_id);
        let mut count_qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ");
        count_qb.push(&table);
        apply_filters(&mut count_qb, entity, tenant, query)?;
        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
        qb.push(&table);
        qb.push(" t");
        apply_filters(&mut qb, entity, tenant, query)?;
        apply_sort(&mut qb, entity, query)?;
        qb.push(" LIMIT ");
        qb.push_bind(query.page_size as i64);
        qb.push(" OFFSET ");
        qb.push_bind(query.offset() as i64);
        let rows = qb
            .build()
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let mut items = Vec::new();
        for row in rows {
            let value: Value = row
                .try_get(0)
                .map_err(|e| QefroError::database(e.to_string()))?;
            enforce_tenant(entity, ctx, &value)?;
            items.push(value);
        }
        Ok(Page {
            items,
            page: query.page,
            page_size: query.page_size,
            total,
        })
    }

    pub async fn update_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        entity: &EntityDef,
        ctx: &OpContext,
        id: Uuid,
        mut patch: Value,
    ) -> QefroResult<Value> {
        let obj = patch
            .as_object_mut()
            .ok_or_else(|| QefroError::bad_request("record must be a JSON object"))?;
        strip_system_writes(entity, obj);
        if obj.is_empty() {
            return self.get_tx(tx, entity, ctx, id, false).await;
        }
        let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
        qb.push(table_ident(entity)?);
        qb.push(" SET ");
        qb.push(quote_ident("updated_at")?);
        qb.push(" = ");
        qb.push_bind(Utc::now());
        qb.push(", ");
        qb.push(quote_ident("updated_by")?);
        qb.push(" = ");
        qb.push_bind(ctx.user_id);
        for (key, value) in obj.iter() {
            let field = entity
                .get_field(key)
                .ok_or_else(|| QefroError::bad_request(format!("unknown field '{key}'")))?;
            if !field.stores_column() {
                continue;
            }
            qb.push(", ");
            qb.push(column_ident(entity, key)?);
            qb.push(" = ");
            push_bind_owned(&mut qb, Some(field), value);
        }
        qb.push(" WHERE ");
        qb.push(quote_ident("id")?);
        qb.push(" = ");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(" AND ");
            qb.push(quote_ident("tenant_id")?);
            qb.push(" = ");
            qb.push_bind(ctx.tenant_id);
        }
        if entity.soft_delete {
            qb.push(" AND ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" IS NULL");
        }
        qb.push(" RETURNING to_jsonb(");
        qb.push(table_ident(entity)?);
        qb.push(".*)");
        let row = qb
            .build()
            .fetch_optional(&mut **tx)
            .await
            .map_err(map_db_err)?;
        let row = row.ok_or_else(|| QefroError::not_found(format!("{} not found", entity.name)))?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        enforce_tenant(entity, ctx, &value)?;
        Ok(value)
    }

    pub async fn insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        entity: &EntityDef,
        ctx: &OpContext,
        mut data: Value,
    ) -> QefroResult<Value> {
        let obj = data
            .as_object_mut()
            .ok_or_else(|| QefroError::bad_request("record must be a JSON object"))?;
        strip_system_writes(entity, obj);
        let id = Uuid::new_v4();
        let now = Utc::now();
        let mut qb = QueryBuilder::<Postgres>::new("INSERT INTO ");
        qb.push(table_ident(entity)?);
        qb.push(" (");
        qb.push(quote_ident("id")?);
        if entity.tenant_owned {
            qb.push(", ");
            qb.push(quote_ident("tenant_id")?);
        }
        qb.push(", ");
        qb.push(quote_ident("created_at")?);
        qb.push(", ");
        qb.push(quote_ident("updated_at")?);
        qb.push(", ");
        qb.push(quote_ident("created_by")?);
        qb.push(", ");
        qb.push(quote_ident("updated_by")?);
        let stored: Vec<_> = entity.stored_fields();
        for field in &stored {
            if obj.contains_key(&field.name) || field.default.is_some() {
                qb.push(", ");
                qb.push(quote_ident(&field.column_name())?);
            }
        }
        qb.push(") VALUES (");
        qb.push_bind(id);
        if entity.tenant_owned {
            qb.push(", ");
            qb.push_bind(ctx.tenant_id);
        }
        qb.push(", ");
        qb.push_bind(now);
        qb.push(", ");
        qb.push_bind(now);
        qb.push(", ");
        qb.push_bind(ctx.user_id);
        qb.push(", ");
        qb.push_bind(ctx.user_id);
        for field in &stored {
            if let Some(value) = obj.get(&field.name) {
                qb.push(", ");
                push_bind_owned(&mut qb, Some(field), value);
            } else if let Some(default) = &field.default {
                qb.push(", ");
                push_bind_owned(&mut qb, Some(field), default);
            }
        }
        qb.push(") RETURNING to_jsonb(");
        qb.push(table_ident(entity)?);
        qb.push(".*)");
        let row = qb.build().fetch_one(&mut **tx).await.map_err(map_db_err)?;
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(value)
    }

    pub async fn delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        entity: &EntityDef,
        ctx: &OpContext,
        id: Uuid,
    ) -> QefroResult<Value> {
        let existing = self.get_tx(tx, entity, ctx, id, true).await?;
        if entity.soft_delete {
            let mut qb = QueryBuilder::<Postgres>::new("UPDATE ");
            qb.push(table_ident(entity)?);
            qb.push(" SET ");
            qb.push(quote_ident("deleted_at")?);
            qb.push(" = ");
            qb.push_bind(Utc::now());
            qb.push(", ");
            qb.push(quote_ident("updated_at")?);
            qb.push(" = ");
            qb.push_bind(Utc::now());
            qb.push(" WHERE ");
            qb.push(quote_ident("id")?);
            qb.push(" = ");
            qb.push_bind(id);
            if entity.tenant_owned {
                qb.push(" AND ");
                qb.push(quote_ident("tenant_id")?);
                qb.push(" = ");
                qb.push_bind(ctx.tenant_id);
            }
            qb.build().execute(&mut **tx).await.map_err(map_db_err)?;
        } else {
            let mut qb = QueryBuilder::<Postgres>::new("DELETE FROM ");
            qb.push(table_ident(entity)?);
            qb.push(" WHERE ");
            qb.push(quote_ident("id")?);
            qb.push(" = ");
            qb.push_bind(id);
            if entity.tenant_owned {
                qb.push(" AND ");
                qb.push(quote_ident("tenant_id")?);
                qb.push(" = ");
                qb.push_bind(ctx.tenant_id);
            }
            qb.build().execute(&mut **tx).await.map_err(map_db_err)?;
        }
        Ok(existing)
    }
}

fn project_fields(value: Value, fields: &[String]) -> Value {
    let Some(obj) = value.as_object() else {
        return value;
    };
    let mut out = Map::new();
    for f in fields {
        if let Some(v) = obj.get(f) {
            out.insert(f.clone(), v.clone());
        }
    }
    if let Some(id) = obj.get("id") {
        out.entry("id".to_string()).or_insert(id.clone());
    }
    Value::Object(out)
}

fn enforce_tenant(entity: &EntityDef, ctx: &OpContext, value: &Value) -> QefroResult<()> {
    if !entity.tenant_owned {
        return Ok(());
    }
    let Some(tid) = value.get("tenant_id").and_then(|v| v.as_str()) else {
        return Err(QefroError::internal("record missing tenant_id"));
    };
    let tid = Uuid::parse_str(tid).map_err(|e| QefroError::internal(e.to_string()))?;
    qefro_tenant::assert_tenant(tid, ctx.tenant_id)
}

fn map_db_err(err: sqlx::Error) -> QefroError {
    if let sqlx::Error::Database(db) = &err {
        if db.code().as_deref() == Some("23505") {
            return QefroError::conflict("unique constraint violated");
        }
        if db.code().as_deref() == Some("23503") {
            return QefroError::bad_request("related record does not exist");
        }
    }
    QefroError::database(err.to_string())
}

pub fn record_id(value: &Value) -> QefroResult<Uuid> {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| QefroError::internal("record missing id"))
}
