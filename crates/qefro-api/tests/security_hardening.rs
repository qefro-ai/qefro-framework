//! Security hardening v2 regressions: RowPolicy activity, tenant matrix,
//! privilege / workflow / secrets / refresh / production gates.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    looks_sensitive, validate_http_url, AppModule, DashboardCard, DashboardDef, EntityDef,
    FieldDef, RowPolicy,
};
use qefro_permissions::{Action, PermissionGrant, ROLE_STAFF};
use qefro_workflow::{TransitionDef, WorkflowDef};
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
        AppModule::new("sec_hard")
            .entity(
                EntityDef::new("HardTicket")
                    .table_name("sec_hard_tickets")
                    .slug_name("hard-tickets")
                    .row_policy(RowPolicy::AssignedTo)
                    .workflow("hard_ticket")
                    .field(FieldDef::string("title").required().searchable())
                    .field(
                        FieldDef::enum_values("status", vec!["Draft", "Open", "Done"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .field(FieldDef::assigned_to())
                    .build(),
            )
            .dashboard(
                DashboardDef::new("hard-home", "Home").card(DashboardCard::activity(
                    "Recent",
                    "HardTicket",
                    20,
                )),
            )
            .build(),
    )
    .workflow(
        WorkflowDef::new("hard_ticket", "HardTicket", "Draft")
            .transition(TransitionDef::new("open", "Draft", "Open").roles(&["Staff", "Admin"]))
            .transition(TransitionDef::new("done", "Open", "Done").roles(&["Admin"])),
    )
    .permission(PermissionGrant::new(
        ROLE_STAFF,
        "HardTicket",
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
        jwt_secret: "security-hardening-secret".into(),
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

fn assert_no_secrets(value: &Value) {
    let blob = value.to_string();
    for key in [
        "password_hash",
        "session_hash",
        "token_hash",
        "storage_key",
        "secret_env",
        "provider_secret",
    ] {
        assert!(
            !blob.contains(&format!("\"{key}\"")),
            "secret {key} leaked: {value}"
        );
    }
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

async fn staff_token(
    router: &axum::Router,
    admin: &str,
    email: &str,
    slug: &str,
) -> (String, String) {
    let (status, user) = json(
        clone_router(router),
        post(
            "/api/v1/users",
            Some(admin),
            json!({
                "name": "Staff",
                "email": email,
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    let id = user["id"].as_str().unwrap().to_string();
    let (status, login) = json(
        clone_router(router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": email,
                "password": "password123",
                "tenant_slug": slug
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    (login["access_token"].as_str().unwrap().to_string(), id)
}

#[tokio::test]
async fn activity_respects_row_policy() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug = format!("hact-{suffix}");
    let admin = register(&router, &format!("hact-{suffix}@ex.com"), &slug).await;
    let (staff_a, id_a) =
        staff_token(&router, &admin, &format!("hact-a-{suffix}@ex.com"), &slug).await;
    let (staff_b, _) =
        staff_token(&router, &admin, &format!("hact-b-{suffix}@ex.com"), &slug).await;

    let marker = format!("hidden-ticket-{suffix}");
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/hard-tickets",
            Some(&admin),
            json!({ "title": marker, "assigned_to": id_a }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let ticket_id = created["id"].as_str().unwrap().to_string();

    let (status, comment) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/hard-tickets/{ticket_id}/comments"),
            Some(&admin),
            json!({ "message": format!("secret note {marker}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{comment}");

    let (status, b_get) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/hard-tickets/{ticket_id}/activity"),
            Some(&staff_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{b_get}");

    let (status, dash_b) = json(
        clone_router(&router),
        get("/api/v1/dashboards/hard-home", Some(&staff_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dash_b}");
    let leaked = serde_json::to_string(&dash_b).unwrap();
    assert!(!leaked.contains(&ticket_id), "{dash_b}");
    assert!(!leaked.contains(&marker), "{dash_b}");
    let total = dash_b["cards"]
        .as_array()
        .and_then(|cards| cards.first())
        .and_then(|c| c.get("total").or_else(|| c.get("value")))
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|n| n as u64)))
        .unwrap_or(0);
    assert_eq!(total, 0, "{dash_b}");

    let (status, dash_a) = json(
        clone_router(&router),
        get("/api/v1/dashboards/hard-home", Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dash_a}");
    let visible = serde_json::to_string(&dash_a).unwrap();
    assert!(
        visible.contains(&ticket_id) || visible.contains(&marker),
        "{dash_a}"
    );
}

#[tokio::test]
async fn tenant_isolation_matrix() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("hta-{suffix}@ex.com"),
        &format!("hta-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("htb-{suffix}@ex.com"),
        &format!("htb-{suffix}"),
    )
    .await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/hard-tickets",
            Some(&admin_a),
            json!({ "title": format!("tenant-a-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    for (method, req) in [
        (
            "GET",
            get(&format!("/api/v1/hard-tickets/{id}"), Some(&admin_b)),
        ),
        (
            "PATCH",
            patch(
                &format!("/api/v1/hard-tickets/{id}"),
                Some(&admin_b),
                json!({ "title": "hijack" }),
            ),
        ),
        (
            "DELETE",
            delete(&format!("/api/v1/hard-tickets/{id}"), Some(&admin_b)),
        ),
        (
            "ACTIVITY",
            get(
                &format!("/api/v1/hard-tickets/{id}/activity"),
                Some(&admin_b),
            ),
        ),
    ] {
        let (status, body) = json(clone_router(&router), req).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {body}");
    }

    let (status, search) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/search?q=tenant-a-{suffix}"),
            Some(&admin_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let blob = serde_json::to_string(&search).unwrap();
    assert!(!blob.contains(id), "{search}");

    let (status, export) = json(
        clone_router(&router),
        get("/api/v1/hard-tickets/export", Some(&admin_b)),
    )
    .await;
    assert!(
        status == StatusCode::OK || status == StatusCode::NOT_FOUND,
        "{export} {status}"
    );
    if status == StatusCode::OK {
        let raw = export.to_string();
        assert!(!raw.contains(id), "{export}");
    }

    let (status, jobs) = json(clone_router(&router), get("/api/v1/jobs", Some(&admin_b))).await;
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::UNAUTHORIZED,
        "{jobs} {status}"
    );
}

#[tokio::test]
async fn privilege_and_self_escalation() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug = format!("hpr-{suffix}");
    let admin = register(&router, &format!("hpr-{suffix}@ex.com"), &slug).await;
    let (staff, user_id) =
        staff_token(&router, &admin, &format!("hpr-s-{suffix}@ex.com"), &slug).await;

    let (status, roles) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&staff),
            json!({ "roles": ["Admin"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{roles}");

    let (status, bulk) = json(
        clone_router(&router),
        post(
            "/api/v1/users/bulk",
            Some(&staff),
            json!({
                "action": "update",
                "ids": [user_id],
                "fields": { "roles": ["Admin"] }
            }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{bulk} {status}"
    );

    let (status, audit) = json(clone_router(&router), get("/api/v1/audit", Some(&staff))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{audit}");

    let (status, studio) = json(
        clone_router(&router),
        get("/api/v1/studio/overview", Some(&staff)),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
        "{studio} {status}"
    );
}

#[tokio::test]
async fn workflow_cannot_be_patched() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug = format!("hwf-{suffix}");
    let admin = register(&router, &format!("hwf-{suffix}@ex.com"), &slug).await;
    let (staff, id_s) =
        staff_token(&router, &admin, &format!("hwf-s-{suffix}@ex.com"), &slug).await;
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/hard-tickets",
            Some(&staff),
            json!({ "title": "wf", "assigned_to": id_s }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, patched) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/hard-tickets/{id}"),
            Some(&staff),
            json!({ "status": "Done" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::UNPROCESSABLE_ENTITY
            || status == StatusCode::BAD_REQUEST,
        "{patched} {status}"
    );

    let (status, bulk) = json(
        clone_router(&router),
        post(
            "/api/v1/hard-tickets/bulk",
            Some(&staff),
            json!({
                "action": "update",
                "ids": [id],
                "fields": { "status": "Done" }
            }),
        ),
    )
    .await;
    if status == StatusCode::OK {
        let item = &bulk["results"][0];
        assert_eq!(item["ok"], false, "{bulk}");
    } else {
        assert!(
            status == StatusCode::CONFLICT
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::BAD_REQUEST
                || status == StatusCode::UNPROCESSABLE_ENTITY,
            "{bulk} {status}"
        );
    }
}

#[tokio::test]
async fn secrets_are_stripped_from_responses() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("hsec-{suffix}@ex.com"),
        &format!("hsec-{suffix}"),
    )
    .await;
    for path in [
        "/api/v1/auth/me",
        "/api/v1/users",
        "/api/v1/meta/ui",
        "/api/v1/webhooks",
    ] {
        let (status, body) = json(clone_router(&router), get(path, Some(&token))).await;
        assert!(status.is_success(), "{path} {status} {body}");
        assert_no_secrets(&body);
    }
}

#[tokio::test]
async fn refresh_reissues_access_token() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("href-{suffix}@ex.com"),
        &format!("href-{suffix}"),
    )
    .await;
    let (status, body) = json(
        clone_router(&router),
        post("/api/v1/auth/refresh", Some(&token), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let next = body["access_token"].as_str().unwrap();
    assert!(!next.is_empty());
    let (status, me) = json(clone_router(&router), get("/api/v1/auth/me", Some(next))).await;
    assert_eq!(status, StatusCode::OK, "{me}");
}

#[tokio::test]
async fn production_rejects_insecure_defaults() {
    let mut cfg = Config::default();
    cfg.env = "production".into();
    assert!(cfg.validate().is_err());
    cfg.jwt_secret = "a-sufficiently-long-secret".into();
    assert!(cfg.validate().is_err());
    cfg.database_url = "postgres://app:unique-not-default@127.0.0.1:5432/qefro".into();
    assert!(cfg.validate().is_ok());
    cfg.log_level = "debug".into();
    assert!(cfg.validate().is_err());
    cfg.log_level = "info".into();
    cfg.cors_origins = vec!["*".into()];
    assert!(cfg.validate().is_err());
}

#[test]
fn ssrf_policy_rejects_private_targets() {
    assert!(validate_http_url("http://127.0.0.1/").is_err());
    assert!(validate_http_url("http://169.254.169.254/latest").is_err());
    assert!(validate_http_url("https://hooks.example.com/ok").is_ok());
}

#[test]
fn log_redaction_detects_bearer_without_printing_values() {
    assert!(looks_sensitive("Authorization: Bearer redacted"));
    assert!(!looks_sensitive(
        "authenticated request path=/api/v1/hard-tickets"
    ));
}

#[tokio::test]
async fn huge_in_filter_is_rejected() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("hin-{suffix}@ex.com"),
        &format!("hin-{suffix}"),
    )
    .await;
    let ids = (0..120)
        .map(|_| Uuid::new_v4().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (status, body) = json(
        clone_router(&router),
        get(&format!("/api/v1/hard-tickets?id.in={ids}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn metadata_cannot_execute_sql() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("hsql-{suffix}@ex.com"),
        &format!("hsql-{suffix}"),
    )
    .await;
    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/validate",
            Some(&token),
            json!({
                "kind": "entity",
                "target": "HardTicket",
                "payload": { "sql": "DROP TABLE users" }
            }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST
            || status == StatusCode::UNPROCESSABLE_ENTITY
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::OK,
        "{body} {status}"
    );
    let blob = serde_json::to_string(&body).unwrap();
    assert!(!blob.to_ascii_lowercase().contains("drop table"));
}

#[tokio::test]
async fn switch_tenant_rejects_cross_tenant_and_revokes_on_success() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token_a = register(
        &router,
        &format!("hsw-{suffix}@ex.com"),
        &format!("hsw-{suffix}"),
    )
    .await;
    let token_b = register(
        &router,
        &format!("hswb-{suffix}@ex.com"),
        &format!("hswb-{suffix}"),
    )
    .await;
    let (status, me_b) = json(clone_router(&router), get("/api/v1/auth/me", Some(&token_b))).await;
    assert_eq!(status, StatusCode::OK, "{me_b}");
    let tenant_b = me_b["tenant_id"].as_str().unwrap();
    let (status, me_a) = json(clone_router(&router), get("/api/v1/auth/me", Some(&token_a))).await;
    assert_eq!(status, StatusCode::OK, "{me_a}");
    let tenant_a = me_a["tenant_id"].as_str().unwrap().to_string();

    let (status, cross) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/switch-tenant",
            Some(&token_a),
            json!({ "tenant_id": tenant_b }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{cross}");
    let (status, still) = json(clone_router(&router), get("/api/v1/auth/me", Some(&token_a))).await;
    assert_eq!(status, StatusCode::OK, "{still}");

    let (status, rotated) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/switch-tenant",
            Some(&token_a),
            json!({ "tenant_id": tenant_a }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let next = rotated["access_token"].as_str().unwrap();
    assert!(!next.is_empty());
    let (status, old) = json(clone_router(&router), get("/api/v1/auth/me", Some(&token_a))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{old}");
    let (status, me) = json(clone_router(&router), get("/api/v1/auth/me", Some(next))).await;
    assert_eq!(status, StatusCode::OK, "{me}");
}
