use qefro_core::{quote_ident, OpContext, QefroError, QefroResult};
use qefro_permissions::Action;
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};
use std::collections::BTreeMap;

use crate::service::EntityService;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub entity: String,
    pub slug: String,
    pub id: String,
    pub label: String,
    pub snippet: String,
    pub score: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchGroup {
    pub entity: String,
    pub label: String,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchHit>,
    pub groups: Vec<SearchGroup>,
}

impl EntityService {
    pub async fn global_search(
        &self,
        ctx: &OpContext,
        q: &str,
        limit: usize,
    ) -> QefroResult<Vec<SearchHit>> {
        Ok(self.global_search_grouped(ctx, q, limit).await?.results)
    }

    pub async fn global_search_grouped(
        &self,
        ctx: &OpContext,
        q: &str,
        limit: usize,
    ) -> QefroResult<SearchResponse> {
        let q = q.trim();
        if q.is_empty() {
            return Ok(SearchResponse {
                results: Vec::new(),
                groups: Vec::new(),
            });
        }
        if q.chars().count() > 200 {
            return Err(QefroError::bad_request("search query too long"));
        }
        let per = limit.clamp(1, 50);
        let needle = search_needle(q);
        let mut hits = Vec::new();
        for entity in self.registry().list() {
            if entity.is_child() || entity.singleton {
                continue;
            }
            if !ctx.allows_app(entity.module.as_deref()) {
                continue;
            }
            if self
                .permissions()
                .check(ctx, &entity.name, Action::List)
                .is_err()
            {
                continue;
            }
            if entity.name == qefro_core::USER_ENTITY {
                if let Some(auth) = self.identity_service() {
                    if let Ok((items, _)) = auth
                        .list_tenant_users(ctx.tenant_id, Some(q), 1, per as u32)
                        .await
                    {
                        for value in items {
                            let id = value
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let label = entity.display_label(&value);
                            let snippet = value
                                .get("email")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&label)
                                .to_string();
                            let score = rank_text(&needle, &label, 8)
                                .max(rank_text(&needle, &snippet, 4));
                            hits.push(SearchHit {
                                entity: entity.name.clone(),
                                slug: entity.slug.clone(),
                                id,
                                label: label.clone(),
                                snippet,
                                score,
                            });
                        }
                    }
                }
                continue;
            }
            if entity.skip_ddl {
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
            for (i, field) in searchable.iter().enumerate() {
                if i > 0 {
                    qb.push(" OR ");
                }
                qb.push(quote_ident(&field.column_name())?);
                if field.search_exact {
                    qb.push("::text ILIKE ");
                    qb.push_bind(needle.clone());
                } else {
                    qb.push("::text ILIKE ");
                    qb.push_bind(like_pattern(q));
                }
            }
            qb.push(") LIMIT ");
            qb.push_bind(per as i64);
            let rows = qb
                .build()
                .fetch_all(self.pool())
                .await
                .map_err(|e| QefroError::database(e.to_string()))?;
            for row in rows {
                let mut value: Value = row
                    .try_get(0)
                    .map_err(|e| QefroError::database(e.to_string()))?;
                qefro_core::strip_secrets(Some(&entity), &mut value);
                self.strip_search_fields(ctx, &entity, &mut value);
                let id = value
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let label = entity.display_label(&value);
                let mut score = rank_text(&needle, &label, 12);
                let mut snippet = label.clone();
                for field in &searchable {
                    let text = value
                        .get(&field.name)
                        .and_then(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .or_else(|| (!v.is_null()).then(|| v.to_string().trim_matches('"').to_string()))
                        })
                        .unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    let field_score = rank_text(&needle, &text, field.search_weight.max(1));
                    if field_score > 0 && (snippet == label || field_score > score) {
                        snippet = text.clone();
                    }
                    score = score.max(field_score);
                }
                if let Some(expanded) = value.get("_expanded").and_then(|v| v.as_object()) {
                    for rel in expanded.values() {
                        if let Some(rel_label) = rel.get("label").and_then(|v| v.as_str()) {
                            score = score.max(rank_text(&needle, rel_label, 6));
                        }
                    }
                }
                hits.push(SearchHit {
                    entity: entity.name.clone(),
                    slug: entity.slug.clone(),
                    id,
                    label,
                    snippet,
                    score,
                });
            }
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.label.cmp(&b.label)));
        hits.truncate(per.saturating_mul(4));
        let mut grouped: BTreeMap<String, SearchGroup> = BTreeMap::new();
        for hit in &hits {
            let entry = grouped.entry(hit.entity.clone()).or_insert_with(|| {
                let label = self
                    .registry()
                    .try_get(&hit.entity)
                    .map(|e| e.label_plural.clone())
                    .unwrap_or_else(|| hit.entity.clone());
                SearchGroup {
                    entity: hit.entity.clone(),
                    label,
                    hits: Vec::new(),
                }
            });
            entry.hits.push(hit.clone());
        }
        let groups: Vec<SearchGroup> = grouped.into_values().collect();
        Ok(SearchResponse {
            results: hits,
            groups,
        })
    }

    fn strip_search_fields(
        &self,
        ctx: &OpContext,
        entity: &qefro_core::EntityDef,
        record: &mut Value,
    ) {
        qefro_core::strip_secrets(Some(entity), record);
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

fn search_needle(q: &str) -> String {
    if q.starts_with('"') && q.ends_with('"') && q.len() > 1 {
        q.trim_matches('"').to_string()
    } else if q.ends_with('*') {
        q.trim_end_matches('*').to_string()
    } else {
        q.to_string()
    }
}

fn like_pattern(q: &str) -> String {
    if q.starts_with('"') && q.ends_with('"') && q.len() > 1 {
        q.trim_matches('"').to_string()
    } else if q.ends_with('*') {
        format!("{}%", q.trim_end_matches('*'))
    } else {
        format!("%{q}%")
    }
}

fn rank_text(needle: &str, haystack: &str, weight: i32) -> i32 {
    let n = needle.to_lowercase();
    let h = haystack.to_lowercase();
    if n.is_empty() || h.is_empty() {
        return 0;
    }
    if h == n {
        return weight * 8;
    }
    if h.starts_with(&n) {
        return weight * 4;
    }
    if h.contains(&n) {
        return weight * 2;
    }
    0
}
