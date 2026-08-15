use anyhow::Result;
use qefro_api::{Config, QefroRuntime};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn,tower_http=info".into()),
        )
        .init();

    let mut runtime = QefroRuntime::new(Config::from_env()?);
    runtime.install(qefro_restaurant::installed());
    runtime.serve().await?;
    Ok(())
}
