//! Live PostgreSQL security regressions for the 3.7 audit.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, RowPolicy};
use qefro_permissions::{Action, PermissionGrant, ROLE_STAFF};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

fn app() -> InstalledApp {
    InstalledApp::new(
        AppModule::new("sec_audit")
            .entity(
                EntityDef::new("Ticket")
                    .table_name("sec_audit_tickets")
                    .slug_name("sec-tickets")
                    .row_policy(RowPolicy::AssignedTo)
                    .field(FieldDef::string("title").required().searchable())
                    .field(FieldDef::assigned_to())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::new(
        ROLE_STAFF,
        "Ticket",
        vec![
            Action::Create,
            Action::Read,
            Action::List,
            Action::Update,
            Action::Delete,
            Action::Export,
        ],
    ))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "security-audit-secret".into(),
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

async fn register(router: &axum::Router, email: &str, slug: &str) -> String {
    let (status, body) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Admin",
                "email": email,
                "password": "password123",
                "tenant_name": slug,
                "tenant_slug": slug
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn logout_revokes_the_session() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("lo-{suffix}@ex.com"),
        &format!("lo-{suffix}"),
    )
    .await;

    let (status, _) = json(
        clone_router(&router),
        post("/api/v1/auth/logout", Some(&token), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, me) = json(clone_router(&router), get("/api/v1/auth/me", Some(&token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{me}");
}

#[tokio::test]
async fn jwt_none_algorithm_is_rejected() {
    let router = runtime().await;
    let token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIwMDAwMDAwMC0wMDAwLTAwMDAtMDAwMC0wMDAwMDAwMDAwMDAiLCJ0aWQiOiIwMDAwMDAwMC0wMDAwLTAwMDAtMDAwMC0wMDAwMDAwMDAwMDAiLCJzaWQiOiIwMDAwMDAwMC0wMDAwLTAwMDAtMDAwMC0wMDAwMDAwMDAwMDAiLCJyb2xlcyI6WyJBZG1pbiJdLCJleHAiOjk5OTk5OTk5OTksImlhdCI6MX0.";
    let (status, body) = json(clone_router(&router), get("/api/v1/auth/me", Some(token))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn production_rejects_default_jwt_and_star_cors() {
    let mut cfg = Config::default();
    cfg.env = "production".into();
    assert!(cfg.validate().is_err());
    cfg.jwt_secret = "a-sufficiently-long-secret".into();
    assert!(cfg.validate().is_err());
    cfg.database_url = "postgres://app:unique-not-default@127.0.0.1:5432/qefro".into();
    assert!(cfg.validate().is_ok());
    cfg.cors_origins = vec!["*".into()];
    assert!(cfg.validate().is_err());
}

#[tokio::test]
async fn registration_can_be_disabled() {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "security-audit-secret".into(),
        bind: "127.0.0.1:0".into(),
        allow_register: false,
        ..Config::default()
    });
    rt.install(app());
    let router = rt.build().await.expect("build").0;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, body) = json(
        router,
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("nr-{suffix}@ex.com"),
                "password": "password123",
                "tenant_name": format!("nr-{suffix}"),
                "tenant_slug": format!("nr-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn row_policy_applies_to_search_aggregates_and_idor() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("rp-{suffix}@ex.com"),
        &format!("rp-{suffix}"),
    )
    .await;

    let (status, user_a) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff A",
                "email": format!("sa-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user_a}");
    let id_a = user_a["id"].as_str().unwrap().to_string();

    let (status, user_b) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff B",
                "email": format!("sb-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user_b}");

    let (status, login_a) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("sa-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("rp-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login_a}");
    let staff_a = login_a["access_token"].as_str().unwrap().to_string();

    let (status, login_b) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("sb-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("rp-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login_b}");
    let staff_b = login_b["access_token"].as_str().unwrap().to_string();

    let marker = format!("secret-ticket-{suffix}");
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/sec-tickets",
            Some(&admin),
            json!({ "title": marker, "assigned_to": id_a }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let ticket_id = created["id"].as_str().unwrap().to_string();

    let (status, get_b) = json(
        clone_router(&router),
        get(&format!("/api/v1/sec-tickets/{ticket_id}"), Some(&staff_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{get_b}");

    let (status, patch_b) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/sec-tickets/{ticket_id}"),
            Some(&staff_b),
            json!({ "title": "hijack" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{patch_b}");

    let (status, delete_b) = json(
        clone_router(&router),
        delete(&format!("/api/v1/sec-tickets/{ticket_id}"), Some(&staff_b)),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN,
        "{delete_b} {status}"
    );

    let (status, search_b) = json(
        clone_router(&router),
        get(&format!("/api/v1/search?q={marker}"), Some(&staff_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_b}");
    let hits = search_b["results"]
        .as_array()
        .or_else(|| search_b["groups"].as_array())
        .cloned()
        .unwrap_or_default();
    let leaked = serde_json::to_string(&search_b).unwrap();
    assert!(!leaked.contains(&ticket_id), "{search_b}");
    let _ = hits;

    let (status, search_a) = json(
        clone_router(&router),
        get(&format!("/api/v1/search?q={marker}"), Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_a}");
    let found = serde_json::to_string(&search_a).unwrap();
    assert!(
        found.contains(&ticket_id) || found.contains(&marker),
        "{search_a}"
    );

    let (status, agg_b) = json(
        clone_router(&router),
        get(
            "/api/v1/sec-tickets/aggregates?group_by=title&metric=count",
            Some(&staff_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{agg_b}");
    let series = agg_b["series"].as_array().cloned().unwrap_or_default();
    let total: f64 = series
        .iter()
        .filter_map(|row| row.get("value").and_then(|v| v.as_f64()))
        .sum();
    assert_eq!(total, 0.0, "{agg_b}");

    let (status, sql) = json(
        clone_router(&router),
        get(
            "/api/v1/sec-tickets?title=x;%20drop%20table%20sec_audit_tickets",
            Some(&staff_a),
        ),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST
            || status == StatusCode::OK
            || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{sql} {status}"
    );
    if status == StatusCode::OK {
        let items = sql["items"].as_array().cloned().unwrap_or_default();
        assert!(
            items.is_empty() || items.iter().all(|i| i["title"] != marker),
            "{sql}"
        );
    }
}

#[tokio::test]
async fn self_role_escalation_is_rejected() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("es-{suffix}@ex.com"),
        &format!("es-{suffix}"),
    )
    .await;
    let (status, user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("es-s-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    let user_id = user["id"].as_str().unwrap().to_string();
    let (status, login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("es-s-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("es-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let staff = login["access_token"].as_str().unwrap().to_string();
    let (status, me) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&staff),
            json!({ "roles": ["Admin"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{me}");
}

#[tokio::test]
async fn unknown_user_login_is_invalid_credentials() {
    let router = runtime().await;
    let (status, body) = json(
        router,
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": "missing-user@example.com",
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["message"], "invalid credentials");
}
