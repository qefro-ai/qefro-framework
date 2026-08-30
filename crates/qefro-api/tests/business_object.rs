//! Qefro 1.2 business object runtime: workflow, activity, audit, attachments,
//! notifications, and agent EntityOps on the existing EntityService path.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, NotificationDef};
use qefro_permissions::{PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
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
        AppModule::new("bo_runtime")
            .entity(
                EntityDef::new("BoTicket")
                    .table_name("bo_tickets")
                    .slug_name("bo-tickets")
                    .label("Ticket")
                    .workflow("bo_ticket")
                    .attachments()
                    .with_party()
                    .field(FieldDef::string("title").required().searchable())
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Submitted", "Approved"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .build(),
            )
            .notification(
                NotificationDef::new("ticket_created", "entity.created")
                    .title("Ticket created")
                    .recipients(&["Admin", "Staff", "Manager"]),
            )
            .notification(
                NotificationDef::new("ticket_moved", "workflow.transitioned")
                    .title("Ticket moved")
                    .recipients(&["Admin", "Staff", "Manager"]),
            )
            .build(),
    )
    .workflow(
        WorkflowDef::new("bo_ticket", "BoTicket", "Draft")
            .state(StateDef::new("Submitted"))
            .state(StateDef::new("Approved").terminal())
            .transition(
                TransitionDef::new("submit", "Draft", "Submitted")
                    .label("Submit")
                    .roles(&["Staff", "Manager"]),
            )
            .transition(
                TransitionDef::new("approve", "Submitted", "Approved")
                    .label("Approve")
                    .roles(&["Manager"])
                    .confirm("Approve this ticket?"),
            )
            .transition(
                TransitionDef::new("reject", "Submitted", "Draft")
                    .label("Reject")
                    .roles(&["Manager"]),
            ),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "BoTicket"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "BoTicket"))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "business-object-test-secret".into(),
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

fn multipart_file(
    path: &str,
    token: &str,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Request<Body> {
    let boundary = "----qefroBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
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
    suffix: &str,
    tenant_slug: &str,
) -> String {
    let email = format!("staff-{suffix}@ex.com");
    let (status, created) = json(
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
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, login) = json(
        clone_router(router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": email,
                "password": "password123",
                "tenant_slug": tenant_slug
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    login["access_token"].as_str().unwrap().to_string()
}

fn assert_no_secrets(value: &Value) {
    let blob = value.to_string();
    for key in [
        "password_hash",
        "password",
        "storage_key",
        "session_token",
        "jwt",
        "reset_token",
    ] {
        assert!(
            !blob.contains(&format!("\"{key}\"")),
            "secret {key} leaked: {value}"
        );
    }
}

#[tokio::test]
async fn workflow_transition_activity_and_no_direct_status_patch() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant = format!("bo-{suffix}");
    let admin = register(&router, &format!("bo-{suffix}@ex.com"), &tenant).await;
    let staff = staff_token(&router, &admin, suffix, &tenant).await;

    let (status, ticket) = json(
        clone_router(&router),
        post(
            "/api/v1/bo-tickets",
            Some(&staff),
            json!({ "title": "Window seating" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ticket}");
    let id = ticket["id"].as_str().unwrap();
    assert_eq!(ticket["status"], "Draft");
    let transitions = ticket["_workflow"]["transitions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        transitions.iter().any(|t| t["name"] == "submit"),
        "{ticket}"
    );

    let (status, patched) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/bo-tickets/{id}"),
            Some(&staff),
            json!({ "status": "Approved" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{patched}");

    let (status, invalid) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/bo-tickets/{id}/transition"),
            Some(&staff),
            json!({ "transition": "approve" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::FORBIDDEN,
        "{invalid}"
    );

    let (status, submitted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/bo-tickets/{id}/transition"),
            Some(&staff),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(submitted["status"], "Submitted");
    let staff_next = submitted["_workflow"]["transitions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        staff_next.iter().all(|t| t["name"] != "approve"),
        "{submitted}"
    );

    let (status, as_admin) = json(
        clone_router(&router),
        get(&format!("/api/v1/bo-tickets/{id}"), Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{as_admin}");
    let next = as_admin["_workflow"]["transitions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(next.iter().any(|t| t["name"] == "approve"), "{as_admin}");
    assert_eq!(
        next.iter().find(|t| t["name"] == "approve").unwrap()["confirmation"],
        true
    );

    let (status, forbidden) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/bo-tickets/{id}/transition"),
            Some(&staff),
            json!({ "transition": "approve" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{forbidden}");

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/bo-tickets/{id}/activity"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let items = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().any(|i| i["activity_type"] == "created"),
        "{activity}"
    );
    assert!(
        items
            .iter()
            .any(|i| i["activity_type"] == "workflow_transition"),
        "{activity}"
    );
    assert_no_secrets(&activity);
}

#[tokio::test]
async fn activity_comments_are_tenant_scoped() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("aa-{suffix}@ex.com"),
        &format!("aa-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("ab-{suffix}@ex.com"),
        &format!("ab-{suffix}"),
    )
    .await;

    let (status, ticket) = json(
        clone_router(&router),
        post(
            "/api/v1/bo-tickets",
            Some(&admin_a),
            json!({ "title": "Isolation" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ticket}");
    let id = ticket["id"].as_str().unwrap();

    let (status, comment) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/bo-tickets/{id}/comments"),
            Some(&admin_a),
            json!({ "message": "Customer requested window seating." }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{comment}");
    assert_eq!(comment["activity_type"], "comment");
    assert_eq!(comment["message"], "Customer requested window seating.");
    assert_eq!(comment["actor_name"], "Admin");

    let (status, timeline) = json(
        clone_router(&router),
        get(&format!("/api/v1/bo-tickets/{id}/activity"), Some(&admin_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{timeline}");
    assert!(timeline["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["activity_type"] == "comment"));

    let (status, cross) = json(
        clone_router(&router),
        get(&format!("/api/v1/bo-tickets/{id}/activity"), Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{cross}");
}

#[tokio::test]
async fn audit_records_changes_strips_secrets_and_requires_admin() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant = format!("au-{suffix}");
    let admin = register(&router, &format!("au-{suffix}@ex.com"), &tenant).await;
    let staff = staff_token(&router, &admin, suffix, &tenant).await;

    let (status, ticket) = json(
        clone_router(&router),
        post(
            "/api/v1/bo-tickets",
            Some(&admin),
            json!({ "title": "Audit me" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ticket}");
    let id = ticket["id"].as_str().unwrap();

    let (status, updated) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/bo-tickets/{id}"),
            Some(&admin),
            json!({ "title": "Audited" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");

    let (status, staff_audit) = json(
        clone_router(&router),
        get("/api/v1/audit?entity=BoTicket", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_audit}");

    let (status, audit) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/audit?entity=BoTicket&entity_id={id}"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    let items = audit["items"].as_array().cloned().unwrap_or_default();
    assert!(!items.is_empty(), "{audit}");
    assert_no_secrets(&audit);
    let update = items.iter().find(|i| i["action"] == "update");
    assert!(update.is_some(), "{audit}");
    let changes = &update.unwrap()["changes"];
    assert_eq!(changes["title"]["old"], "Audit me");
    assert_eq!(changes["title"]["new"], "Audited");
}

#[tokio::test]
async fn attachments_are_tenant_and_permission_scoped() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("fa-{suffix}@ex.com"),
        &format!("fa-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("fb-{suffix}@ex.com"),
        &format!("fb-{suffix}"),
    )
    .await;

    let (status, ticket) = json(
        clone_router(&router),
        post(
            "/api/v1/bo-tickets",
            Some(&admin_a),
            json!({ "title": "Files" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ticket}");
    let id = ticket["id"].as_str().unwrap();

    let (status, uploaded) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/bo-tickets/{id}/attachments"),
            &admin_a,
            "invoice.pdf",
            "application/pdf",
            b"%PDF-1.4 test",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uploaded}");
    assert_eq!(uploaded["filename"], "invoice.pdf");
    assert!(uploaded.get("storage_key").is_none(), "{uploaded}");
    let file_id = uploaded["id"].as_str().unwrap();

    let (status, listed) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/bo-tickets/{id}/attachments"),
            Some(&admin_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["items"][0]["filename"], "invoice.pdf");
    assert!(listed["items"][0].get("storage_key").is_none());

    let (status, cross_list) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/bo-tickets/{id}/attachments"),
            Some(&admin_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{cross_list}");

    let (status, cross_get) = json(
        clone_router(&router),
        get(&format!("/api/v1/attachments/{file_id}"), Some(&admin_b)),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN,
        "{cross_get}"
    );
}

#[tokio::test]
async fn notifications_are_tenant_scoped_and_agents_use_entity_ops() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("na-{suffix}@ex.com"),
        &format!("na-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("nb-{suffix}@ex.com"),
        &format!("nb-{suffix}"),
    )
    .await;

    let (status, ticket) = json(
        clone_router(&router),
        post(
            "/api/v1/bo-tickets",
            Some(&admin_a),
            json!({ "title": "Notify" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ticket}");
    let id = ticket["id"].as_str().unwrap();

    for _ in 0..6 {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let (_, notes) = json(
            clone_router(&router),
            get("/api/v1/notifications", Some(&admin_a)),
        )
        .await;
        if notes["items"]
            .as_array()
            .map(|items| !items.is_empty())
            .unwrap_or(false)
        {
            break;
        }
    }

    let (status, notes_a) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&admin_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes_a}");
    let items_a = notes_a["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items_a.iter().any(|n| n["entity"] == "BoTicket"),
        "{notes_a}"
    );

    let (status, notes_b) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes_b}");
    let items_b = notes_b["items"].as_array().cloned().unwrap_or_default();
    assert!(items_b.iter().all(|n| n["record_id"] != id), "{notes_b}");

    let (status, agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/list_activity_bo_ticket/invoke",
            Some(&admin_a),
            json!({ "id": id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{agent}");

    let (status, cross_agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/get_bo_ticket/invoke",
            Some(&admin_b),
            json!({ "id": id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{cross_agent}");

    let (status, comment) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/comment_bo_ticket/invoke",
            Some(&admin_a),
            json!({ "id": id, "message": "Noted by agent" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{comment}");

    let (status, timeline) = json(
        clone_router(&router),
        get(&format!("/api/v1/bo-tickets/{id}/activity"), Some(&admin_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{timeline}");
    let items = timeline["items"].as_array().cloned().unwrap_or_default();
    let agent_comment = items.iter().find(|i| i["message"] == "Noted by agent");
    assert!(agent_comment.is_some(), "{timeline}");
    assert_eq!(agent_comment.unwrap()["actor_name"], "Qefro Agent");
}
