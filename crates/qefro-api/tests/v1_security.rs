//! V1.0 cross-tenant and public-surface regression suite.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{EntityDef, FieldDef, PublicFormDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_PUBLIC, ROLE_STAFF};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn app() -> InstalledApp {
    InstalledApp::new(
        qefro_core::AppModule::new("v1_sec")
            .entity(
                EntityDef::new("Memo")
                    .table_name("v1_memos")
                    .slug_name("v1-memos")
                    .attachments()
                    .field(FieldDef::string("title").required().searchable())
                    .build(),
            )
            .entity(
                EntityDef::new("Visit")
                    .table_name("v1_visits")
                    .slug_name("v1-visits")
                    .public_form(
                        PublicFormDef::new("visit")
                            .fields(&["guest_name"])
                            .success_message("ok"),
                    )
                    .field(FieldDef::string("guest_name").required().searchable())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "Memo"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "Visit"))
    .permission(PermissionGrant::new(
        ROLE_PUBLIC,
        "Visit",
        vec![Action::Create],
    ))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url().expect("DATABASE_URL"),
        jwt_secret: "test-secret-v1-security".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(app());
    rt.build().await.expect("build").0
}

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
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

async fn register(router: &axum::Router, suffix: &str) -> String {
    let (status, auth) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "User",
                "email": format!("v1-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("V-{suffix}"),
                "tenant_slug": format!("v-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    auth["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn version_and_metrics_are_public() {
    if db_url().is_none() {
        return;
    }
    let router = runtime().await;
    let (status, body) = json(clone_router(&router), get("/api/v1/meta/version", None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["api"], "v1");
    assert_eq!(body["metadata_schema"], 1);
    let (status, metrics) = json(clone_router(&router), get("/metrics", None)).await;
    assert_eq!(status, StatusCode::OK, "{metrics}");
    assert!(metrics.get("http_requests").is_some());
    assert!(metrics.get("tenant_id").is_none());
}

#[tokio::test]
async fn unauthenticated_uses_stable_error_code() {
    if db_url().is_none() {
        return;
    }
    let router = runtime().await;
    let (status, body) = json(router, get("/api/v1/v1-memos", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "unauthenticated");
    assert!(body["message"].as_str().unwrap().len() > 0);
}

#[tokio::test]
async fn tenant_a_cannot_access_tenant_b() {
    if db_url().is_none() {
        return;
    }
    let router = runtime().await;
    let a = &Uuid::new_v4().to_string()[..8];
    let b = &Uuid::new_v4().to_string()[..8];
    let token_a = register(&router, a).await;
    let token_b = register(&router, b).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/v1-memos",
            Some(&token_a),
            json!({ "title": format!("secret-{a}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, _) = json(
        clone_router(&router),
        get(&format!("/api/v1/v1-memos/{id}"), Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/v1-memos/{id}"),
            Some(&token_b),
            json!({ "title": "stolen" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = json(
        clone_router(&router),
        delete(&format!("/api/v1/v1-memos/{id}"), Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/v1-memos", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"].as_array().unwrap().len(), 0);

    let (status, search) = json(
        clone_router(&router),
        get(&format!("/api/v1/search?q=secret-{a}"), Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let results = search["results"].as_array().cloned().unwrap_or_default();
    assert!(results.is_empty(), "{search}");

    let (status, notes) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes}");
    let items = notes["items"]
        .as_array()
        .cloned()
        .or_else(|| notes.as_array().cloned())
        .unwrap_or_default();
    assert!(items.iter().all(|n| n.get("tenant_id") != created.get("tenant_id")));

    let (status, atts) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/v1-memos/{id}/attachments"),
            Some(&token_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{atts}");

    let (status, agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/get_memo/invoke",
            Some(&token_b),
            json!({ "id": id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{agent}");
}

#[tokio::test]
async fn public_form_cannot_switch_tenant_or_inject_fields() {
    if db_url().is_none() {
        return;
    }
    let router = runtime().await;
    let a = &Uuid::new_v4().to_string()[..8];
    let b = &Uuid::new_v4().to_string()[..8];
    let _token_a = register(&router, a).await;
    let token_b = register(&router, b).await;
    let slug_a = format!("v-{a}");

    let (status, created) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/public/{slug_a}/visit"),
            None,
            json!({
                "guest_name": "Public Guest",
                "tenant_id": Uuid::new_v4(),
                "title": "injected",
                "roles": ["Admin"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["ok"], true);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/v1-visits", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn search_query_too_long_is_rejected() {
    if db_url().is_none() {
        return;
    }
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let q = "x".repeat(201);
    let (status, body) = json(
        clone_router(&router),
        get(&format!("/api/v1/v1-memos?search={q}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
