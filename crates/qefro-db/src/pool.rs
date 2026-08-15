use qefro_core::{QefroError, QefroResult};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

pub type DbPool = PgPool;

pub async fn connect(database_url: &str) -> QefroResult<DbPool> {
    let max = std::env::var("QEFRO_DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
        .clamp(2, 100);
    let acquire = std::env::var("QEFRO_DB_ACQUIRE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    PgPoolOptions::new()
        .max_connections(max)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(acquire))
        .idle_timeout(Duration::from_secs(600))
        .connect(database_url)
        .await
        .map_err(|e| QefroError::database(format!("failed to connect: {e}")))
}

pub async fn ping(pool: &DbPool) -> QefroResult<()> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}
