use qefro_core::{QefroError, QefroResult};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

pub type DbPool = PgPool;

pub async fn connect(database_url: &str) -> QefroResult<DbPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
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
