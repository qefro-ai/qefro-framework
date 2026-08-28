use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, CardViewSpec, EntityDef, EntityViews, FieldDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_STAFF};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn test_db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

fn test_app() -> InstalledApp {
    let module = AppModule::new("ui_views_test")
        .entity(
            EntityDef::new("CardNote")
                .table_name("ui_card_notes")
                .slug_name("ui-card-notes")
                .field(FieldDef::string("title").required().searchable())
                .views(EntityViews {
                    card: Some(CardViewSpec {
                        enabled: true,
                        title: Some("title".into()),
                        subtitle: None,
                        image: None,
                        fields: vec!["title".into()],
                    }),
                    ..Default::default()
                })
                .build(),
        )
        .entity(
            EntityDef::new("PlainNote")
                .table_name("ui_plain_notes")
                .slug_name("ui-plain-notes")
                .field(FieldDef::string("title").required().searchable())
                .build(),
        )
        .build();
    InstalledApp::new(module)
        .permission(PermissionGrant::crud("Admin", "CardNote"))
        .permission(PermissionGrant::crud("Admin", "PlainNote"))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "CardNote",
            vec![Action::Read, Action::List],
        ))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "PlainNote",
            vec![Action::Read, Action::List],
        ))
}

async fn runtime() -> axum::Router {
    let url = test_db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(test_app());
    let (router, _) = rt.build().await.expect("build");
    router
}

async fn json(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice(&bytes).unwrap_or(json!({ "raw": String::from_utf8_lossy(&bytes) }))
    };
    (status, value)
}

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
}

fn post(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn patch(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut b = Request::builder()
        .method("PATCH")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

fn find_entity<'a>(ui: &'a Value, name: &str) -> &'a Value {
    ui["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == name)
        .unwrap()
}

#[tokio::test]
async fn views_card_round_trips_and_schema_stays_one() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("card-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("C-{suffix}"),
                "tenant_slug": format!("c-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["access_token"].as_str().unwrap();

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(token))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], "1");
    let card = find_entity(&ui, "CardNote");
    assert_eq!(card["views"]["card"]["title"], "title");
    assert_eq!(card["views"]["card"]["enabled"], true);
    let plain = find_entity(&ui, "PlainNote");
    assert!(plain["views"].get("card").is_none() || plain["views"]["card"].is_null());
}

#[tokio::test]
async fn entity_permissions_differ_and_unauthorized_patch_is_403() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let staff_email = format!("staff-{suffix}@example.com");
    let (status, admin_body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Admin",
                "email": format!("admin-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("P-{suffix}"),
                "tenant_slug": format!("p-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{admin_body}");
    let admin = admin_body["access_token"].as_str().unwrap();

    let (status, _) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(admin),
            json!({
                "name": "Staff",
                "email": staff_email,
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, staff_body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({ "email": staff_email, "password": "password123" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_body}");
    let staff = staff_body["access_token"].as_str().unwrap();

    let (status, admin_ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(admin))).await;
    assert_eq!(status, StatusCode::OK);
    let admin_note = find_entity(&admin_ui, "CardNote");
    assert_eq!(admin_note["permissions"]["create"], true);
    assert_eq!(admin_note["permissions"]["delete"], true);

    let (status, staff_ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(staff))).await;
    assert_eq!(status, StatusCode::OK);
    let staff_note = find_entity(&staff_ui, "CardNote");
    assert_eq!(staff_note["permissions"]["list"], true);
    assert_eq!(staff_note["permissions"]["read"], true);
    assert_eq!(staff_note["permissions"]["create"], false);
    assert_eq!(staff_note["permissions"]["update"], false);
    assert_eq!(staff_note["permissions"]["delete"], false);

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/ui-card-notes",
            Some(admin),
            json!({ "title": format!("n-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["_permissions"]["update"], true);
    assert_eq!(created["_permissions"]["delete"], true);

    let (status, got) = json(
        clone_router(&router),
        get(&format!("/api/v1/ui-card-notes/{id}"), Some(staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["_permissions"]["update"], false);
    assert_eq!(got["_permissions"]["delete"], false);

    let (status, denied) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/ui-card-notes/{id}"),
            Some(staff),
            json!({ "title": "nope" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
}
