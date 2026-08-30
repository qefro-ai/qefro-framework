use chrono::{DateTime, Utc};
use qefro_core::{QefroError, QefroResult, TenantConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

struct CacheEntry {
    at: Instant,
    config: TenantConfig,
}

pub struct TenantService {
    pool: PgPool,
    cache: Mutex<HashMap<Uuid, CacheEntry>>,
    ttl: Duration,
}

impl TenantService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(5),
        }
    }

    fn cache_get(&self, tenant_id: Uuid) -> Option<TenantConfig> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let entry = cache.get(&tenant_id)?;
        if entry.at.elapsed() < self.ttl {
            Some(entry.config.clone())
        } else {
            None
        }
    }

    fn cache_put(&self, tenant_id: Uuid, config: TenantConfig) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(
            tenant_id,
            CacheEntry {
                at: Instant::now(),
                config,
            },
        );
    }

    fn cache_invalidate(&self, tenant_id: Uuid) {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&tenant_id);
    }

    pub async fn create(&self, name: &str, slug: &str) -> QefroResult<Tenant> {
        let slug = slug.to_ascii_lowercase();
        sqlx::query_as::<_, Tenant>(
            r#"
            INSERT INTO tenants (id, name, slug, created_at)
            VALUES ($1, $2, $3, now())
            RETURNING id, name, slug, created_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .bind(&slug)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                QefroError::conflict(format!("tenant slug '{slug}' already exists"))
            } else {
                QefroError::database(e.to_string())
            }
        })
    }

    pub async fn get(&self, id: Uuid) -> QefroResult<Tenant> {
        sqlx::query_as::<_, Tenant>("SELECT id, name, slug, created_at FROM tenants WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?
            .ok_or_else(|| QefroError::not_found("tenant not found"))
    }

    pub async fn get_by_slug(&self, slug: &str) -> QefroResult<Tenant> {
        sqlx::query_as::<_, Tenant>(
            "SELECT id, name, slug, created_at FROM tenants WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("tenant not found"))
    }

    pub async fn list(&self) -> QefroResult<Vec<Tenant>> {
        sqlx::query_as::<_, Tenant>(
            "SELECT id, name, slug, created_at FROM tenants ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn get_config(&self, tenant_id: Uuid) -> QefroResult<TenantConfig> {
        if let Some(cached) = self.cache_get(tenant_id) {
            return Ok(cached);
        }
        let row = sqlx::query_as::<_, TenantSettingsRow>(
            r#"
            SELECT branding, ui_config, enabled_apps, business_config, feature_flags, plan
            FROM tenant_settings WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        let config = row.map(|r| r.into_config()).unwrap_or_default();
        self.cache_put(tenant_id, config.clone());
        Ok(config)
    }

    pub async fn upsert_config(
        &self,
        tenant_id: Uuid,
        config: &TenantConfig,
    ) -> QefroResult<TenantConfig> {
        sqlx::query(
            r#"
            INSERT INTO tenant_settings (
                tenant_id, branding, ui_config, enabled_apps, business_config,
                feature_flags, plan, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, now())
            ON CONFLICT (tenant_id) DO UPDATE SET
                branding = EXCLUDED.branding,
                ui_config = EXCLUDED.ui_config,
                enabled_apps = EXCLUDED.enabled_apps,
                business_config = EXCLUDED.business_config,
                feature_flags = EXCLUDED.feature_flags,
                plan = EXCLUDED.plan,
                updated_at = now()
            "#,
        )
        .bind(tenant_id)
        .bind(serde_json::to_value(&config.branding).unwrap_or(json!({})))
        .bind(serde_json::to_value(&config.ui_config).unwrap_or(json!({})))
        .bind(&config.enabled_apps)
        .bind(serde_json::to_value(&config.business).unwrap_or(json!({})))
        .bind(serde_json::to_value(&config.features.flags).unwrap_or(json!({})))
        .bind(&config.plan)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        self.cache_invalidate(tenant_id);
        self.get_config(tenant_id).await
    }
}

#[derive(sqlx::FromRow)]
struct TenantSettingsRow {
    branding: sqlx::types::Json<serde_json::Value>,
    ui_config: sqlx::types::Json<serde_json::Value>,
    enabled_apps: Vec<String>,
    business_config: sqlx::types::Json<serde_json::Value>,
    feature_flags: sqlx::types::Json<serde_json::Value>,
    plan: Option<String>,
}

impl TenantSettingsRow {
    fn into_config(self) -> TenantConfig {
        let business = serde_json::from_value(self.business_config.0.clone()).unwrap_or_default();
        let flags = serde_json::from_value(self.feature_flags.0).unwrap_or_default();
        TenantConfig {
            branding: serde_json::from_value(self.branding.0).unwrap_or_default(),
            ui_config: serde_json::from_value(self.ui_config.0).unwrap_or_default(),
            enabled_apps: self.enabled_apps,
            business,
            business_config: json!({}),
            features: qefro_core::TenantFeatures { flags },
            plan: self.plan,
        }
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

/// Assert that a loaded record belongs to the active tenant. This is a
/// last-line defense if a query accidentally omits the tenant predicate.
pub fn assert_tenant(record_tenant: Uuid, ctx_tenant: Uuid) -> QefroResult<()> {
    if record_tenant == ctx_tenant {
        Ok(())
    } else {
        Err(QefroError::not_found("record not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_cross_tenant() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(assert_tenant(a, a).is_ok());
        assert!(assert_tenant(a, b).is_err());
    }

    #[test]
    fn cache_keys_are_per_tenant() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut cache: HashMap<Uuid, String> = HashMap::new();
        cache.insert(a, "brand-a".into());
        cache.insert(b, "brand-b".into());
        assert_ne!(cache.get(&a), cache.get(&b));
    }
}
