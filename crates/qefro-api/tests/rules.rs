//! Business rules runtime: REST authority, structured errors, workflow guards.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, ValidationRule};
use qefro_permissions::{PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

fn app() -> InstalledApp {
    InstalledApp::new(
        AppModule::new("rules_runtime")
            .entity(
                EntityDef::new("RuleTicket")
                    .table_name("rule_tickets")
                    .slug_name("rule-tickets")
                    .label("Ticket")
                    .workflow("rule_ticket")
                    .field(FieldDef::string("title").required().searchable())
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Submitted", "Completed"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .field(
                        FieldDef::enum_("contact_method", vec!["email", "phone"])
                            .nullable(),
                    )
                    .field(
                        FieldDef::string("email")
                            .nullable()
                            .email()
                            .required_when("contact_method", json!("email")),
                    )
                    .field(FieldDef::integer("qty").greater_than(0.0).nullable())
                    .field(FieldDef::decimal("rate").nullable().min(0.0).default_value(json!(10)))
                    .field(FieldDef::currency("total").computed("qty * rate"))
                    .field(
                        FieldDef::decimal("discount")
                            .nullable()
                            .min(0.0)
                            .default_value(json!(0))
                            .readonly_when("status", json!("Completed")),
                    )
                    .field(FieldDef::date("start_date").nullable())
                    .field(FieldDef::date("end_date").nullable())
                    .field(FieldDef::string("customer_id").nullable())
                    .validation_rule(ValidationRule::compare(
                        "end_date",
                        "greater_or_equal",
                        "start_date",
                    ))
                    .build(),
            )
            .build(),
    )
    .workflow(
        WorkflowDef::new("rule_ticket", "RuleTicket", "Draft")
            .state(StateDef::new("Submitted"))
            .state(StateDef::new("Completed").terminal())
            .transition(
                TransitionDef::new("submit", "Draft", "Submitted")
                    .label("Submit")
                    .roles(&["Staff", "Manager"])
                    .requires(&["customer_id"]),
            )
            .transition(
                TransitionDef::new("complete", "Submitted", "Completed")
                    .label("Complete")
                    .roles(&["Staff", "Manager"]),
            ),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "RuleTicket"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "RuleTicket"))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "rules-runtime-test-secret".into(),
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
    let res = router.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(json!({ "raw": String::from_utf8_lossy(&bytes) }));
    (status, body)
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
async fn rest_enforces_required_when_validation_computed_readonly_and_guards() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let token = register(&router, &format!("r-{suffix}@ex.com"), &format!("rt-{suffix}")).await;

    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&token),
            json!({ "title": "Bad qty", "qty": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"].as_str(), Some("validation_failed"));
    let fields = body["fields"].as_array().or(body["details"]["fields"].as_array());
    let fields = fields.expect("structured fields");
    assert!(
        fields.iter().any(|e| e["field"] == "qty" && e["code"] == "greater_than"),
        "{body}"
    );

    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&token),
            json!({ "title": "Need email", "contact_method": "email", "qty": 2 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let fields = body["fields"]
        .as_array()
        .or(body["details"]["fields"].as_array())
        .expect("fields");
    assert!(fields.iter().any(|e| e["field"] == "email" && e["code"] == "required"), "{body}");

    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&token),
            json!({
                "title": "Range",
                "qty": 2,
                "start_date": "2026-08-02",
                "end_date": "2026-08-01"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let fields = body["fields"]
        .as_array()
        .or(body["details"]["fields"].as_array())
        .expect("fields");
    assert!(
        fields.iter().any(|e| e["field"] == "end_date" && e["code"] == "invalid_range"),
        "{body}"
    );

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&token),
            json!({
                "title": "Ok",
                "qty": 2,
                "rate": 20,
                "total": 999999,
                "contact_method": "phone",
                "customer_id": "cust-1",
                "start_date": "2026-08-01",
                "end_date": "2026-08-02"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["total"].as_f64(), Some(40.0), "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, denied) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/rule-tickets/{id}/transition"),
            Some(&token),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    // customer_id is set, submit should work
    assert_eq!(status, StatusCode::OK, "{denied}");

    let (status, completed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/rule-tickets/{id}/transition"),
            Some(&token),
            json!({ "transition": "complete" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");

    let (status, bypass) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/rule-tickets/{id}"),
            Some(&token),
            json!({ "discount": 50 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bypass}");
    let fields = bypass["fields"]
        .as_array()
        .or(bypass["details"]["fields"].as_array())
        .expect("fields");
    assert!(fields.iter().any(|e| e["code"] == "readonly"), "{bypass}");
}

#[tokio::test]
async fn workflow_guard_blocks_submit_without_required_field() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let token = register(&router, &format!("g-{suffix}@ex.com"), &format!("rg-{suffix}")).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&token),
            json!({ "title": "No customer", "qty": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, blocked) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/rule-tickets/{id}/transition"),
            Some(&token),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    assert!(
        status.is_client_error(),
        "guard should block empty customer_id: {status} {blocked}"
    );
}

#[tokio::test]
async fn tenant_cannot_read_other_tenant_rule_ticket() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let a = register(&router, &format!("ta-{suffix}@ex.com"), &format!("ta-{suffix}")).await;
    let b = register(&router, &format!("tb-{suffix}@ex.com"), &format!("tb-{suffix}")).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/rule-tickets",
            Some(&a),
            json!({ "title": "A only", "qty": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, hidden) = json(
        clone_router(&router),
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/rule-tickets/{id}"))
            .header("authorization", format!("Bearer {b}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{hidden}");
}
