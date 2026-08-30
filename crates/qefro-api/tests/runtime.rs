//! Qefro 2.0 runtime: bulk, archive, export, optimistic lock, workflow guards.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};
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
        AppModule::new("runtime20")
            .entity(
                EntityDef::new("RtTicket")
                    .table_name("rt_tickets")
                    .slug_name("rt-tickets")
                    .label("Ticket")
                    .workflow("rt_ticket")
                    .with_archive()
                    .field(FieldDef::string("title").required().searchable())
                    .field(FieldDef::uuid("customer_id").nullable())
                    .field(FieldDef::assigned_to())
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Submitted", "Approved"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .build(),
            )
            .build(),
    )
    .workflow(
        WorkflowDef::new("rt_ticket", "RtTicket", "Draft")
            .state(StateDef::new("Submitted"))
            .state(StateDef::new("Approved").terminal())
            .transition(
                TransitionDef::new("submit", "Draft", "Submitted")
                    .label("Submit")
                    .roles(&["Staff", "Manager"])
                    .requires(&["customer_id"]),
            )
            .transition(
                TransitionDef::new("approve", "Submitted", "Approved")
                    .label("Approve")
                    .roles(&["Manager"]),
            ),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "RtTicket"))
    .permission(PermissionGrant::new(
        ROLE_STAFF,
        "RtTicket",
        vec![Action::Export],
    ))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "RtTicket"))
    .permission(PermissionGrant::new(
        ROLE_MANAGER,
        "RtTicket",
        vec![Action::Export],
    ))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "runtime20-test-secret".into(),
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

async fn bytes(router: axum::Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
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
async fn bulk_archive_hides_from_list_and_export_respects_permission() {
    let router = runtime().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let token = register(
        &router,
        &format!("rt-{suffix}@ex.com"),
        &format!("rt-{suffix}"),
    )
    .await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/rt-tickets",
            Some(&token),
            json!({ "title": "Need prep" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, bulk) = json(
        clone_router(&router),
        post(
            "/api/v1/rt-tickets/bulk",
            Some(&token),
            json!({ "action": "archive", "ids": [id] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bulk}");
    assert_eq!(bulk["succeeded"], 1);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/rt-tickets", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let items = listed["items"].as_array().cloned().unwrap_or_default();
    assert!(items.iter().all(|row| row["id"] != id), "{listed}");

    let (status, got) = json(
        clone_router(&router),
        get(&format!("/api/v1/rt-tickets/{id}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert!(got.get("archived_at").is_some());

    let (status, restored) = json(
        clone_router(&router),
        post(
            "/api/v1/rt-tickets/bulk",
            Some(&token),
            json!({ "action": "restore", "ids": [id] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["succeeded"], 1);

    let (status, csv) = bytes(
        clone_router(&router),
        get("/api/v1/rt-tickets/export", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8_lossy(&csv);
    assert!(text.contains("Title") || text.contains("title"), "{text}");
    assert!(text.contains("Need prep"), "{text}");
}

#[tokio::test]
async fn optimistic_lock_and_workflow_guard() {
    let router = runtime().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let token = register(
        &router,
        &format!("lock-{suffix}@ex.com"),
        &format!("lock-{suffix}"),
    )
    .await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/rt-tickets",
            Some(&token),
            json!({ "title": "Lock me" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    let updated_at = created["updated_at"].as_str().unwrap();

    let (status, _) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/rt-tickets/{id}"),
            Some(&token),
            json!({ "title": "Changed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, conflict) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/rt-tickets/{id}"),
            Some(&token),
            json!({ "title": "Stale", "_expected_updated_at": updated_at }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert!(
        conflict["message"]
            .as_str()
            .unwrap_or("")
            .contains("another user"),
        "{conflict}"
    );

    let (status, blank) = json(
        clone_router(&router),
        post(
            "/api/v1/rt-tickets",
            Some(&token),
            json!({ "title": "Needs customer" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{blank}");
    let blank_id = blank["id"].as_str().unwrap();
    let (status, blocked) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/rt-tickets/{blank_id}/transition"),
            Some(&token),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT || status == StatusCode::BAD_REQUEST,
        "{blocked}"
    );
    assert!(
        blocked["message"]
            .as_str()
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains("required"),
        "{blocked}"
    );
}

#[tokio::test]
async fn ui_capabilities_include_archive_and_export() {
    let router = runtime().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let token = register(
        &router,
        &format!("ui-{suffix}@ex.com"),
        &format!("ui-{suffix}"),
    )
    .await;
    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&token))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    let entities = ui["entities"].as_array().cloned().unwrap_or_default();
    let ticket = entities
        .iter()
        .find(|e| e["entity"] == "RtTicket")
        .expect("RtTicket");
    assert_eq!(ticket["capabilities"]["archive"], true);
    assert_eq!(ticket["capabilities"]["assignment"], true);
    assert_eq!(ticket["capabilities"]["bulk"], true);
    assert_eq!(ticket["permissions"]["export"], true);
}
