use chrono::{DateTime, Utc};
use qefro_core::{QefroError, QefroResult, TenantConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

pub struct TenantService {
    pool: PgPool,
}

impl TenantService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let row = sqlx::query_as::<_, TenantSettingsRow>(
            r#"
            SELECT branding, ui_config, enabled_apps, business_config
            FROM tenant_settings WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(row.map(|r| r.into_config()).unwrap_or_default())
    }

    pub async fn upsert_config(&self, tenant_id: Uuid, config: &TenantConfig) -> QefroResult<TenantConfig> {
        sqlx::query(
            r#"
            INSERT INTO tenant_settings (tenant_id, branding, ui_config, enabled_apps, business_config, updated_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (tenant_id) DO UPDATE SET
                branding = EXCLUDED.branding,
                ui_config = EXCLUDED.ui_config,
                enabled_apps = EXCLUDED.enabled_apps,
                business_config = EXCLUDED.business_config,
                updated_at = now()
            "#,
        )
        .bind(tenant_id)
        .bind(serde_json::to_value(&config.branding).unwrap_or(json!({})))
        .bind(serde_json::to_value(&config.ui_config).unwrap_or(json!({})))
        .bind(&config.enabled_apps)
        .bind(config.business_config.clone())
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        self.get_config(tenant_id).await
    }
}

#[derive(sqlx::FromRow)]
struct TenantSettingsRow {
    branding: sqlx::types::Json<serde_json::Value>,
    ui_config: sqlx::types::Json<serde_json::Value>,
    enabled_apps: Vec<String>,
    business_config: sqlx::types::Json<serde_json::Value>,
}

impl TenantSettingsRow {
    fn into_config(self) -> TenantConfig {
        TenantConfig {
            branding: serde_json::from_value(self.branding.0).unwrap_or_default(),
            ui_config: serde_json::from_value(self.ui_config.0).unwrap_or_default(),
            enabled_apps: self.enabled_apps,
            business_config: self.business_config.0,
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
}
