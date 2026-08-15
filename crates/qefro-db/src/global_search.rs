use qefro_core::{quote_ident, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};

use crate::service::EntityService;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub entity: String,
    pub slug: String,
    pub id: String,
    pub label: String,
    pub snippet: String,
}

impl EntityService {
    pub async fn global_search(
        &self,
        ctx: &OpContext,
        q: &str,
        limit: usize,
    ) -> QefroResult<Vec<SearchHit>> {
        let q = q.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        if q.chars().count() > 200 {
            return Err(QefroError::bad_request("search query too long"));
        }
        let per = limit.clamp(1, 50);
        let mut hits = Vec::new();
        for entity in self.registry().list() {
            if entity.is_child() || entity.singleton {
                continue;
            }
            if !ctx.allows_app(entity.module.as_deref()) {
                continue;
            }
            if self.permissions().check(ctx, &entity.name, Action::List).is_err() {
                continue;
            }
            let searchable: Vec<_> = entity
                .searchable_fields()
                .into_iter()
                .filter(|f| {
                    self.permissions()
                        .can_read_field(ctx, &entity.name, f.permission_level)
                })
                .collect();
            if searchable.is_empty() {
                continue;
            }
            let table = quote_ident(&entity.table)?;
            let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
            qb.push(table);
            qb.push(" t WHERE ");
            if entity.tenant_owned {
                qb.push(quote_ident("tenant_id")?);
                qb.push(" = ");
                qb.push_bind(ctx.tenant_id);
                qb.push(" AND ");
            }
            if entity.soft_delete {
                qb.push(quote_ident("deleted_at")?);
                qb.push(" IS NULL AND ");
            }
            qb.push("(");
            let like = if q.starts_with('"') && q.ends_with('"') && q.len() > 1 {
                q.trim_matches('"').to_string()
            } else if q.ends_with('*') {
                format!("{}%", q.trim_end_matches('*'))
            } else {
                format!("%{q}%")
            };
            for (i, field) in searchable.iter().enumerate() {
                if i > 0 {
                    qb.push(" OR ");
                }
                qb.push(quote_ident(&field.column_name())?);
                qb.push("::text ILIKE ");
                qb.push_bind(like.clone());
            }
            qb.push(") LIMIT ");
            qb.push_bind(per as i64);
            let rows = qb
                .build()
                .fetch_all(self.pool())
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            for row in rows {
                let mut value: Value = row.try_get(0).map_err(|e| QefroError::database(e.to_string()))?;
                self.strip_search_fields(ctx, &entity, &mut value);
                let id = value
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let label = entity.display_label(&value);
                let snippet = searchable
                    .iter()
                    .filter_map(|f| {
                        value.get(&f.name).and_then(|v| v.as_str()).map(|s| s.to_string())
                    })
                    .find(|s| !s.is_empty())
                    .unwrap_or_else(|| label.clone());
                hits.push(SearchHit {
                    entity: entity.name.clone(),
                    slug: entity.slug.clone(),
                    id,
                    label,
                    snippet,
                });
            }
        }
        hits.truncate(per.saturating_mul(4));
        Ok(hits)
    }

    fn strip_search_fields(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) {
        let Some(obj) = record.as_object_mut() else {
            return;
        };
        for field in &entity.fields {
            if field.permission_level > 0
                && !self
                    .permissions()
                    .can_read_field(ctx, &entity.name, field.permission_level)
            {
                obj.remove(&field.name);
            }
        }
        let _ = json!(null);
    }
}
