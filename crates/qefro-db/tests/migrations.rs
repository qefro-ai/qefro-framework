use qefro_core::AppMigration;
use qefro_db::{apply_schema, connect};
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

#[tokio::test]
async fn failed_migration_is_recorded_and_not_applied() {
    let url = db_url();
    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &qefro_core::EntityRegistry::new())
        .await
        .unwrap();

    let app = format!("mig-{}", &Uuid::new_v4().to_string()[..8]);
    let bad = AppMigration {
        id: "001_bad".into(),
        version: "1.0.0".into(),
        description: "fails".into(),
        destructive: false,
        sql: "SELECT 1 FROM qefro_this_table_does_not_exist".into(),
    };
    let err = qefro_db::app_registry::apply_migration(&pool, &app, &bad, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("failed"));

    let applied = qefro_db::app_registry::applied_migrations(&pool, &app)
        .await
        .unwrap();
    assert!(applied.is_empty(), "{applied:?}");

    let status: String =
        sqlx::query_scalar("SELECT status FROM qefro_app_migrations WHERE app = $1 AND name = $2")
            .bind(&app)
            .bind(&bad.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "failed");

    let changed = AppMigration {
        id: "001_bad".into(),
        version: "1.0.0".into(),
        description: "changed".into(),
        destructive: false,
        sql: "SELECT 1".into(),
    };
    let conflict = qefro_db::app_registry::apply_migration(&pool, &app, &changed, false)
        .await
        .unwrap_err();
    assert!(conflict.to_string().contains("checksum"), "{conflict}");

    qefro_db::app_registry::apply_migration(&pool, &app, &bad, false)
        .await
        .expect_err("same failed SQL still fails");
}

#[tokio::test]
async fn successful_migration_is_idempotent() {
    let url = db_url();
    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &qefro_core::EntityRegistry::new())
        .await
        .unwrap();
    let app = format!("migok-{}", &Uuid::new_v4().to_string()[..8]);
    let ok = AppMigration {
        id: "001_ok".into(),
        version: "1.0.1".into(),
        description: "noop".into(),
        destructive: false,
        sql: "SELECT 1".into(),
    };
    qefro_db::app_registry::apply_migration(&pool, &app, &ok, false)
        .await
        .unwrap();
    qefro_db::app_registry::apply_migration(&pool, &app, &ok, false)
        .await
        .unwrap();
    let applied = qefro_db::app_registry::applied_migrations(&pool, &app)
        .await
        .unwrap();
    assert_eq!(applied.len(), 1);
}
