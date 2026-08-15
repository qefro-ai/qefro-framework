use qefro_core::{EntityDef, FieldDef, OpContext};
use qefro_db::{apply_schema, connect, EntityService};
use qefro_events::InProcessEventBus;
use qefro_permissions::{Action, PermissionGrant, PermissionRegistry, ROLE_STAFF};
use qefro_workflow::WorkflowRegistry;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[tokio::test]
async fn cross_tenant_repository_access_fails() {
    let Some(url) = db_url() else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };

    let mut registry = qefro_core::EntityRegistry::new();
    registry
        .register(
            EntityDef::new("Memo")
                .table_name("sec_memos")
                .slug_name("sec-memos")
                .field(FieldDef::string("title").required().unique())
                .build(),
        )
        .unwrap();

    let pool = connect(&url).await.unwrap();
    apply_schema(&pool, &registry).await.unwrap();

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let user_a = Uuid::new_v4();
    let user_b = Uuid::new_v4();

    sqlx::query("INSERT INTO tenants (id, name, slug, created_at) VALUES ($1,$2,$3,now()), ($4,$5,$6,now())")
        .bind(tenant_a)
        .bind("A")
        .bind(format!("memo-a-{}", &tenant_a.to_string()[..8]))
        .bind(tenant_b)
        .bind("B")
        .bind(format!("memo-b-{}", &tenant_b.to_string()[..8]))
        .execute(&pool)
        .await
        .unwrap();

    let mut perms = PermissionRegistry::new();
    perms.grant(PermissionGrant::crud("Admin", "Memo"));
    perms.grant(PermissionGrant::new(
        ROLE_STAFF,
        "Memo",
        vec![Action::Read, Action::List],
    ));

    let service = EntityService::new(
        pool,
        Arc::new(registry),
        Arc::new(perms),
        Arc::new(WorkflowRegistry::new()),
        Arc::new(qefro_core::HookRegistry::new()),
        InProcessEventBus::new(),
    );

    let ctx_a = OpContext::new(tenant_a, user_a, vec!["Admin".into()]);
    let ctx_b = OpContext::new(tenant_b, user_b, vec!["Admin".into()]);
    let created = service
        .create(
            &ctx_a,
            "Memo",
            json!({ "title": format!("m-{}", Uuid::new_v4()) }),
        )
        .await
        .unwrap();
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let err = service.get(&ctx_b, "Memo", id).await.unwrap_err();
    assert_eq!(err.status_code(), 404);

    let staff = OpContext::new(tenant_a, user_a, vec!["Staff".into()]);
    let denied = service
        .create(&staff, "Memo", json!({ "title": "nope" }))
        .await
        .unwrap_err();
    assert_eq!(denied.status_code(), 403);
}
