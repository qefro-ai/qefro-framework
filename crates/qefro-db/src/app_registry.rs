//! Persistent application registry (source of truth when PostgreSQL is available).

use chrono::{DateTime, Utc};
use qefro_core::{lifecycle_event_name, AppManifest, AppMigration, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppRegistryRow {
    pub name: String,
    pub version: String,
    pub label: String,
    pub description: String,
    pub source: String,
    pub status: String,
    pub framework_version: Option<String>,
    pub api_version: String,
    pub dependencies: sqlx::types::Json<Value>,
    pub package_sha256: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn upsert_app(
    pool: &PgPool,
    manifest: &AppManifest,
    status: &str,
    sha256: Option<&str>,
) -> QefroResult<()> {
    let deps = serde_json::to_value(&manifest.dependencies).unwrap_or(json!({}));
    sqlx::query(
        r#"
        INSERT INTO qefro_apps (
            name, version, label, description, source, status, framework_version,
            api_version, dependencies, package_sha256, installed_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, now(), now())
        ON CONFLICT (name) DO UPDATE SET
            version = EXCLUDED.version,
            label = EXCLUDED.label,
            description = EXCLUDED.description,
            source = EXCLUDED.source,
            status = EXCLUDED.status,
            framework_version = EXCLUDED.framework_version,
            api_version = EXCLUDED.api_version,
            dependencies = EXCLUDED.dependencies,
            package_sha256 = EXCLUDED.package_sha256,
            updated_at = now()
        "#,
    )
    .bind(&manifest.name)
    .bind(&manifest.version)
    .bind(&manifest.label)
    .bind(&manifest.description)
    .bind(if manifest.source.is_empty() {
        "catalog"
    } else {
        &manifest.source
    })
    .bind(status)
    .bind(&manifest.framework_version)
    .bind(&manifest.api_version)
    .bind(deps)
    .bind(sha256)
    .execute(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO qefro_app_versions (name, version, source, package_sha256, installed_at)
        VALUES ($1, $2, $3, $4, now())
        ON CONFLICT (name, version) DO NOTHING
        "#,
    )
    .bind(&manifest.name)
    .bind(&manifest.version)
    .bind(if manifest.source.is_empty() {
        "catalog"
    } else {
        &manifest.source
    })
    .bind(sha256)
    .execute(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

pub async fn set_status(pool: &PgPool, name: &str, status: &str) -> QefroResult<()> {
    let n = sqlx::query("UPDATE qefro_apps SET status = $2, updated_at = now() WHERE name = $1")
        .bind(name)
        .bind(status)
        .execute(pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .rows_affected();
    if n == 0 {
        return Err(QefroError::not_found(format!(
            "app '{name}' is not in the registry"
        )));
    }
    Ok(())
}

pub async fn uninstall(pool: &PgPool, name: &str) -> QefroResult<()> {
    sqlx::query("DELETE FROM qefro_apps WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

pub async fn get_app(pool: &PgPool, name: &str) -> QefroResult<Option<AppRegistryRow>> {
    sqlx::query_as::<_, AppRegistryRow>(
        r#"
        SELECT name, version, label, description, source, status, framework_version,
               api_version, dependencies, package_sha256, installed_at, updated_at
        FROM qefro_apps WHERE name = $1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))
}

pub async fn list_apps(pool: &PgPool) -> QefroResult<Vec<AppRegistryRow>> {
    sqlx::query_as::<_, AppRegistryRow>(
        r#"
        SELECT name, version, label, description, source, status, framework_version,
               api_version, dependencies, package_sha256, installed_at, updated_at
        FROM qefro_apps ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))
}

pub async fn version_history(
    pool: &PgPool,
    name: &str,
) -> QefroResult<Vec<(String, DateTime<Utc>)>> {
    sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "SELECT version, installed_at FROM qefro_app_versions WHERE name = $1 ORDER BY installed_at",
    )
    .bind(name)
    .fetch_all(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))
}

pub async fn record_event(
    pool: &PgPool,
    tenant_id: Option<Uuid>,
    user_id: Option<Uuid>,
    app: &str,
    version: Option<&str>,
    event: &str,
    payload: Value,
) -> QefroResult<()> {
    sqlx::query(
        r#"
        INSERT INTO qefro_app_events (id, tenant_id, user_id, app, version, event, payload, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(user_id)
    .bind(app)
    .bind(version)
    .bind(event)
    .bind(payload)
    .execute(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

pub async fn record_lifecycle(
    pool: &PgPool,
    tenant_id: Option<Uuid>,
    app: &str,
    version: Option<&str>,
    on: &str,
) -> QefroResult<()> {
    record_event(
        pool,
        tenant_id,
        None,
        app,
        version,
        lifecycle_event_name(on),
        json!({ "app": app, "version": version }),
    )
    .await
}

pub async fn applied_migrations(pool: &PgPool, app: &str) -> QefroResult<Vec<String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT version, name FROM qefro_app_migrations WHERE app = $1 AND status = 'applied'",
    )
    .bind(app)
    .fetch_all(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(rows.into_iter().map(|(v, n)| format!("{v}:{n}")).collect())
}

const MIGRATE_LOCK: i64 = 0x51EF_2001;

fn migration_checksum(m: &AppMigration) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(m.id.as_bytes());
    h.update(m.version.as_bytes());
    h.update(m.sql.as_bytes());
    hex::encode(h.finalize())
}

pub async fn apply_migration(
    pool: &PgPool,
    app: &str,
    migration: &AppMigration,
    allow_destructive: bool,
) -> QefroResult<()> {
    if migration.looks_destructive() && !allow_destructive {
        return Err(QefroError::bad_request(format!(
            "migration '{}' is destructive; pass --yes to apply",
            migration.id
        )));
    }
    // Session-level advisory locks must be acquired and released on the same
    // connection. Executing lock/unlock through the pool can pick different
    // connections and deadlock the next waiter.
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATE_LOCK)
        .execute(&mut *conn)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    let result = apply_migration_locked(&mut *conn, app, migration).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATE_LOCK)
        .execute(&mut *conn)
        .await;
    result
}

async fn apply_migration_locked(
    conn: &mut PgConnection,
    app: &str,
    migration: &AppMigration,
) -> QefroResult<()> {
    let checksum = migration_checksum(migration);
    let existing: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT status, checksum FROM qefro_app_migrations WHERE app = $1 AND version = $2 AND name = $3",
    )
    .bind(app)
    .bind(&migration.version)
    .bind(&migration.id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    if let Some((status, prev)) = existing {
        if status == "applied" {
            return Ok(());
        }
        if let Some(prev) = prev {
            if prev != checksum {
                return Err(QefroError::conflict(format!(
                    "migration '{}' failed previously with a different checksum; do not retry blindly",
                    migration.id
                )));
            }
        }
    }

    let mut tx = conn
        .begin()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    if !migration.sql.trim().is_empty() {
        if let Err(e) = sqlx::query(&migration.sql).execute(&mut *tx).await {
            let _ = tx.rollback().await;
            record_migration_failure(conn, app, migration, &checksum, &e.to_string()).await?;
            return Err(QefroError::database(format!(
                "migration {} failed; database left unchanged: {e}",
                migration.id
            )));
        }
    }
    sqlx::query(
        r#"
        INSERT INTO qefro_app_migrations (app, version, name, destructive, applied_at, checksum, status, error)
        VALUES ($1, $2, $3, $4, now(), $5, 'applied', NULL)
        ON CONFLICT (app, version, name) DO UPDATE
          SET status = 'applied', checksum = EXCLUDED.checksum, error = NULL, applied_at = now()
        "#,
    )
    .bind(app)
    .bind(&migration.version)
    .bind(&migration.id)
    .bind(migration.looks_destructive())
    .bind(&checksum)
    .execute(&mut *tx)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

async fn record_migration_failure(
    conn: &mut PgConnection,
    app: &str,
    migration: &AppMigration,
    checksum: &str,
    error: &str,
) -> QefroResult<()> {
    sqlx::query(
        r#"
        INSERT INTO qefro_app_migrations (app, version, name, destructive, applied_at, checksum, status, error)
        VALUES ($1, $2, $3, $4, now(), $5, 'failed', $6)
        ON CONFLICT (app, version, name) DO UPDATE
          SET status = 'failed', checksum = EXCLUDED.checksum, error = EXCLUDED.error, applied_at = now()
        "#,
    )
    .bind(app)
    .bind(&migration.version)
    .bind(&migration.id)
    .bind(migration.looks_destructive())
    .bind(checksum)
    .bind(error)
    .execute(&mut *conn)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

pub async fn pending_migrations(
    pool: &PgPool,
    app: &str,
    migrations: &[AppMigration],
) -> QefroResult<Vec<AppMigration>> {
    let applied = applied_migrations(pool, app).await?;
    Ok(migrations
        .iter()
        .filter(|m| {
            !applied
                .iter()
                .any(|a| a == &format!("{}:{}", m.version, m.id))
        })
        .cloned()
        .collect())
}

pub async fn enabled_tenant_count(pool: &PgPool, app: &str) -> QefroResult<i64> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM tenant_settings
        WHERE cardinality(enabled_apps) = 0 OR $1 = ANY(enabled_apps)
        "#,
    )
    .bind(app)
    .fetch_one(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(count)
}

pub async fn seed_applied(
    pool: &PgPool,
    tenant_id: Uuid,
    app: &str,
    kind: &str,
) -> QefroResult<bool> {
    let row: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as(
        "SELECT applied_at FROM qefro_app_seeds WHERE tenant_id = $1 AND app = $2 AND kind = $3",
    )
    .bind(tenant_id)
    .bind(app)
    .bind(kind)
    .fetch_optional(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(row.is_some())
}

pub async fn mark_seed_applied(
    pool: &PgPool,
    tenant_id: Uuid,
    app: &str,
    kind: &str,
) -> QefroResult<()> {
    sqlx::query(
        r#"
        INSERT INTO qefro_app_seeds (tenant_id, app, kind, applied_at)
        VALUES ($1, $2, $3, now())
        ON CONFLICT (tenant_id, app, kind) DO NOTHING
        "#,
    )
    .bind(tenant_id)
    .bind(app)
    .bind(kind)
    .execute(pool)
    .await
    .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}
