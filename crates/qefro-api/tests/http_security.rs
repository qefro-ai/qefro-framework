use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn test_app() -> InstalledApp {
    let module = AppModule::new("security_test")
        .entity(
            EntityDef::new("Note")
                .table_name("sec_notes")
                .slug_name("sec-notes")
                .workflow("note")
                .field(FieldDef::string("title").required().searchable().unique())
                .field(
                    FieldDef::enum_values("status", vec!["Draft", "Published"])
                        .required()
                        .default_value(json!("Draft")),
                )
                .build(),
        )
        .build();
    InstalledApp::new(module)
        .workflow(
            WorkflowDef::new("note", "Note", "Draft").transition(
                TransitionDef::new("publish", "Draft", "Published").roles(&["Manager"]),
            ),
        )
        .permission(PermissionGrant::crud(ROLE_MANAGER, "Note"))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "Note",
            vec![Action::Read, Action::List, Action::Update],
        ))
}

async fn runtime() -> (axum::Router, String) {
    let url = test_db_url().expect("DATABASE_URL");
    let db = format!("{url}_sec_{}", &Uuid::new_v4().to_string()[..8]);
    // Use the same database; tables are shared. Tests isolate by tenant.
    let mut rt = QefroRuntime::new(Config {
        database_url: url.clone(),
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(test_app());
    let (router, _) = rt.build().await.expect("build");
    (router, db)
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

fn delete(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("DELETE").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

async fn register(router: axum::Router, email: &str, slug: &str) -> (axum::Router, String) {
    let (status, body) = json(
        router,
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "User",
                "email": email,
                "password": "password123",
                "tenant_name": slug,
                "tenant_slug": slug
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["access_token"].as_str().unwrap().to_string();
    // oneshot consumes router; rebuild is expensive. Tests that need multiple
    // requests reconstruct via clone before oneshot.
    (axum::Router::new(), token)
}

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
}

#[tokio::test]
async fn health_is_public() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;
    let (status, body) = json(router, get("/health", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body.get("database").is_none());
}

#[tokio::test]
async fn ready_requires_database() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;
    let (status, body) = json(router, get("/ready", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn unauthenticated_cannot_list_entities() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;
    let (status, _) = json(router, get("/api/v1/sec-notes", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn tenant_isolation_and_rbac_and_agent() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;

    let suffix = &Uuid::new_v4().to_string()[..8];
    let email_a = format!("a-{suffix}@example.com");
    let email_b = format!("b-{suffix}@example.com");

    let (status, body_a) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": email_a,
                "password": "password123",
                "tenant_name": format!("A-{suffix}"),
                "tenant_slug": format!("a-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body_a}");
    let token_a = body_a["access_token"].as_str().unwrap();

    let (status, body_b) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Bob",
                "email": email_b,
                "password": "password123",
                "tenant_name": format!("B-{suffix}"),
                "tenant_slug": format!("b-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body_b}");
    let token_b = body_b["access_token"].as_str().unwrap();

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/sec-notes",
            Some(token_a),
            json!({ "title": format!("secret-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, _) = json(
        clone_router(&router),
        get(&format!("/api/v1/sec-notes/{id}"), Some(token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/sec-notes", Some(token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"].as_array().unwrap().len(), 0);

    let (status, client_tenant) = json(
        clone_router(&router),
        post(
            "/api/v1/sec-notes",
            Some(token_b),
            json!({ "title": "nope", "tenant_id": created["tenant_id"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{client_tenant}");

    let (status, agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/get_note/invoke",
            Some(token_b),
            json!({ "id": id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{agent}");

    let (status, _) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/sec-notes/{id}"),
            Some(token_b),
            json!({ "title": "stolen" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = json(
        clone_router(&router),
        delete(&format!("/api/v1/sec-notes/{id}"), Some(token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = register;
}

#[tokio::test]
async fn staff_cannot_bypass_permissions_or_workflow() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_email = format!("admin-{suffix}@example.com");
    let staff_email = format!("staff-{suffix}@example.com");

    let (status, admin_body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Admin",
                "email": admin_email,
                "password": "password123",
                "tenant_name": format!("S-{suffix}"),
                "tenant_slug": format!("s-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{admin_body}");
    let admin = admin_body["access_token"].as_str().unwrap();

    let (status, user) = json(
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
    assert_eq!(status, StatusCode::CREATED, "{user}");

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

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/sec-notes",
            Some(admin),
            json!({ "title": format!("note-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["_workflow"]["current"], "Draft");

    let (status, denied) = json(
        clone_router(&router),
        post(
            "/api/v1/sec-notes",
            Some(staff),
            json!({ "title": format!("staff-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, agent_create) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/create_note/invoke",
            Some(staff),
            json!({ "title": format!("agent-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{agent_create}");

    let (status, tools) = json(clone_router(&router), get("/api/v1/tools", Some(staff))).await;
    assert_eq!(status, StatusCode::OK, "{tools}");
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(!names.contains(&"create_note"));
    assert!(names.contains(&"get_note"));

    let (status, invalid) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/sec-notes/{id}/transition"),
            Some(admin),
            json!({ "transition": "complete" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{invalid}");

    let (status, unauthorized) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/sec-notes/{id}/transition"),
            Some(staff),
            json!({ "transition": "publish" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{unauthorized}");

    let (status, published) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/sec-notes/{id}/transition"),
            Some(admin),
            json!({ "transition": "publish" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    assert_eq!(published["status"], "Published");
}

#[tokio::test]
async fn ui_metadata_includes_visibility_and_workflow() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (router, _) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("ui-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("U-{suffix}"),
                "tenant_slug": format!("u-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["access_token"].as_str().unwrap();

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(token))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    let note = ui["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == "Note")
        .unwrap();
    let title = note["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "title")
        .unwrap();
    assert_eq!(ui["schema_version"], "1");
    assert_eq!(title["list_visible"], true);
    assert_eq!(title["form_visible"], true);
    assert_eq!(note["workflow"], "note");

    let (status, tools) = json(clone_router(&router), get("/api/v1/tools", Some(token))).await;
    assert_eq!(status, StatusCode::OK, "{tools}");
    let create = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "create_note")
        .unwrap();
    assert_eq!(create["operation"], "create");
    assert_eq!(create["entity"], "Note");
}
