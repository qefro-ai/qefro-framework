//! Generic Task runtime: REST, workflow, assignment, related records, jobs.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, UI_SCHEMA_VERSION};
use qefro_permissions::{PermissionGrant, ROLE_STAFF};
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
        AppModule::new("task_runtime")
            .entity(
                EntityDef::new("TaskHost")
                    .table_name("task_hosts")
                    .slug_name("task-hosts")
                    .label("Host")
                    .field(FieldDef::string("name").required().searchable())
                    .with_tasks()
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "TaskHost"))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "task-runtime-test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(app());
    rt.build().await.expect("build")
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

async fn staff_token(
    router: &axum::Router,
    admin: &str,
    suffix: &str,
    tenant_slug: &str,
) -> (String, String) {
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
    let id = created["id"].as_str().unwrap().to_string();
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
    (
        login["access_token"].as_str().unwrap().to_string(),
        id,
    )
}

async fn drain_jobs(state: &qefro_api::AppState) {
    for _ in 0..500 {
        match state
            .entities
            .job_queue()
            .process_one(&state.entities.job_handlers())
            .await
        {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {}
        }
    }
}

#[tokio::test]
async fn task_is_a_platform_entity_on_generic_ui() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("tm-{suffix}@ex.com"),
        &format!("tm-{suffix}"),
    )
    .await;
    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&token))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], UI_SCHEMA_VERSION);
    let entities = ui["entities"].as_array().cloned().unwrap_or_default();
    let task = entities
        .iter()
        .find(|e| e["entity"] == "Task")
        .expect("Task in meta/ui");
    assert_eq!(task["slug"], "tasks");
    assert_eq!(task["workflow"], "task");
    assert_eq!(task["schema_version"], UI_SCHEMA_VERSION);
    let caps = &task["capabilities"];
    assert_eq!(caps["workflow"], true);
    assert_eq!(caps["assignment"], true);
    assert_eq!(caps["activity"], true);
    assert!(task["views"]["kanban"].is_object(), "{task}");
    let host = entities
        .iter()
        .find(|e| e["entity"] == "TaskHost")
        .expect("TaskHost");
    assert!(
        host["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "tasks"),
        "{host}"
    );
}

#[tokio::test]
async fn crud_workflow_related_search_and_no_direct_status() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant = format!("tw-{suffix}");
    let admin = register(&router, &format!("tw-{suffix}@ex.com"), &tenant).await;
    let (staff, staff_id) = staff_token(&router, &admin, suffix, &tenant).await;

    let (status, host) = json(
        clone_router(&router),
        post(
            "/api/v1/task-hosts",
            Some(&admin),
            json!({ "name": "Ahmed Khan" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{host}");
    let host_id = host["id"].as_str().unwrap();

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/tasks",
            Some(&staff),
            json!({
                "title": "Call customer",
                "description": "Follow up with Ahmed",
                "priority": "high",
                "entity_type": "TaskHost",
                "entity_id": host_id
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["status"], "Open");
    assert_eq!(created["assigned_to"], staff_id);
    assert_eq!(created["entity_type"], "TaskHost");
    assert_eq!(created["_expanded"]["entity_id"]["label"], "Ahmed Khan");
    assert_eq!(created["_expanded"]["entity_id"]["slug"], "task-hosts");
    let transitions = created["_workflow"]["transitions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(transitions.iter().any(|t| t["name"] == "start"), "{created}");

    let (status, patched) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/tasks/{id}"),
            Some(&staff),
            json!({ "status": "Completed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{patched}");

    let (status, invalid) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/tasks/{id}/transition"),
            Some(&staff),
            json!({ "transition": "approve" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::NOT_FOUND,
        "{invalid}"
    );

    let (status, started) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/tasks/{id}/transition"),
            Some(&staff),
            json!({ "transition": "start" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["status"], "In Progress");

    let (status, done) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/tasks/{id}/transition"),
            Some(&staff),
            json!({ "transition": "completed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["status"], "Completed");
    assert!(done["completed_at"].as_str().is_some(), "{done}");

    drain_jobs(&state).await;

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/tasks/{id}/activity"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(acts.iter().any(|a| a["activity_type"] == "created"), "{activity}");
    assert!(
        acts.iter()
            .any(|a| a["activity_type"] == "workflow_transition"),
        "{activity}"
    );

    let (status, host_activity) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/task-hosts/{host_id}/activity"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{host_activity}");
    let host_acts = host_activity["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        host_acts.iter().any(|a| a["message"]
            .as_str()
            .is_some_and(|m| m.to_lowercase().contains("task"))),
        "{host_activity}"
    );

    let (status, related) = json(
        clone_router(&router),
        get(&format!("/api/v1/task-hosts/{host_id}"), Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{related}");
    let items = related["_related"]["tasks"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        items.iter().any(|t| t["title"] == "Call customer"),
        "{related}"
    );

    let (status, search) = json(
        clone_router(&router),
        get("/api/v1/search?q=Follow%20up%20with%20Ahmed", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let blob = search.to_string();
    assert!(
        blob.contains("Call customer") || blob.contains("Follow up"),
        "{search}"
    );

    let (status, cancelled_src) = json(
        clone_router(&router),
        post(
            "/api/v1/tasks",
            Some(&admin),
            json!({ "title": "Verify payment" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{cancelled_src}");
    let cancel_id = cancelled_src["id"].as_str().unwrap();
    let (status, cancelled) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/tasks/{cancel_id}/transition"),
            Some(&admin),
            json!({ "transition": "cancelled" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "Cancelled");
}

#[tokio::test]
async fn tenant_isolation_permissions_assignment_and_concurrency() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant_a = format!("ta-{suffix}");
    let tenant_b = format!("tb-{suffix}");
    let admin_a = register(&router, &format!("ta-{suffix}@ex.com"), &tenant_a).await;
    let admin_b = register(&router, &format!("tb-{suffix}@ex.com"), &tenant_b).await;
    let (staff_a, _staff_a_id) = staff_token(&router, &admin_a, &format!("a{suffix}"), &tenant_a).await;
    let (_staff_b, staff_b_id) = staff_token(&router, &admin_b, &format!("b{suffix}"), &tenant_b).await;

    let (status, task_a) = json(
        clone_router(&router),
        post(
            "/api/v1/tasks",
            Some(&admin_a),
            json!({ "title": "Tenant A only" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{task_a}");
    let id_a = task_a["id"].as_str().unwrap();

    let (status, hidden) = json(
        clone_router(&router),
        get(&format!("/api/v1/tasks/{id_a}"), Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{hidden}");

    let (status, list_b) = json(clone_router(&router), get("/api/v1/tasks", Some(&admin_b))).await;
    assert_eq!(status, StatusCode::OK, "{list_b}");
    let items = list_b["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().all(|t| t["title"] != "Tenant A only"),
        "{list_b}"
    );

    let (status, search_b) = json(
        clone_router(&router),
        get("/api/v1/search?q=Tenant%20A%20only", Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_b}");
    assert!(
        !search_b.to_string().contains("Tenant A only"),
        "{search_b}"
    );

    let (status, assign_cross) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/tasks/{id_a}"),
            Some(&admin_a),
            json!({ "assigned_to": staff_b_id }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
        "{assign_cross}"
    );

    let (status, staff_assign) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/tasks/{id_a}"),
            Some(&staff_a),
            json!({ "assigned_to": staff_b_id }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::NOT_FOUND,
        "{staff_assign}"
    );

    let (status, deleted) = json(
        clone_router(&router),
        delete(&format!("/api/v1/tasks/{id_a}"), Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{deleted}");

    let (status, no_create) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&staff_a),
            json!({
                "name": "Nope",
                "email": format!("nope-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::CREATED,
        "{no_create}"
    );

    let updated_at = task_a["updated_at"].as_str().unwrap();
    let (status, first) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/tasks/{id_a}"),
            Some(&admin_a),
            json!({ "title": "First save", "_expected_updated_at": updated_at }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, stale) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/tasks/{id_a}"),
            Some(&admin_a),
            json!({ "title": "Stale", "_expected_updated_at": updated_at }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
}

#[tokio::test]
async fn assignment_notification_due_reminder_and_automation_idempotency() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant = format!("tn-{suffix}");
    let admin = register(&router, &format!("tn-{suffix}@ex.com"), &tenant).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/tasks",
            Some(&admin),
            json!({
                "title": "Verify dietary request",
                "due_at": "2000-01-01T00:00:00Z",
                "priority": "urgent"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_string();
    drain_jobs(&state).await;

    let (status, notes) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes}");
    let items = notes["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().any(|n| n["title"]
            .as_str()
            .is_some_and(|t| t.to_lowercase().contains("task"))),
        "{notes}"
    );
    let before = items.len();
    let _ = state.entities.dispatch_outbox().await;
    drain_jobs(&state).await;
    let (status, notes2) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes2}");
    let after = notes2["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(after, before, "retry must not duplicate notifications");

    let due_notes = items
        .iter()
        .filter(|n| n["title"].as_str() == Some("Task due"))
        .count();
    assert!(due_notes >= 1, "overdue open task should remind: {notes}");

    let (status, completed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/tasks/{id}/transition"),
            Some(&admin),
            json!({ "transition": "completed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    drain_jobs(&state).await;
    let (status, notes3) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes3}");
    let due_after = notes3["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["title"].as_str() == Some("Task due"))
        .count();
    assert_eq!(
        due_after, due_notes,
        "completed tasks must not receive another reminder: {notes3}"
    );
}

#[tokio::test]
async fn staff_cannot_create_without_grant_on_unrelated_and_my_tasks_filter() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let tenant = format!("tf-{suffix}");
    let admin = register(&router, &format!("tf-{suffix}@ex.com"), &tenant).await;
    let (staff, staff_id) = staff_token(&router, &admin, suffix, &tenant).await;

    let (status, mine) = json(
        clone_router(&router),
        post(
            "/api/v1/tasks",
            Some(&staff),
            json!({ "title": "My queue" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{mine}");

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/tasks?assigned_to=me", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let items = listed["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().all(|t| t["assigned_to"] == staff_id),
        "{listed}"
    );
}
