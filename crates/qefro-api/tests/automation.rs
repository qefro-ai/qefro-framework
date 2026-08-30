//! Automation runtime: validation DSL, events, AutomationDef, jobs, tenancy.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    AppModule, AutomationAction, AutomationDef, AutomationTrigger, Condition, EntityDef, FieldDef,
    NotificationDef, ValidationRule, WebhookDef, WhenClause,
};
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
        AppModule::new("auto_runtime")
            .entity(
                EntityDef::new("AutoTicket")
                    .table_name("auto_tickets")
                    .slug_name("auto-tickets")
                    .label("Ticket")
                    .workflow("auto_ticket")
                    .field(FieldDef::string("title").required().searchable())
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Submitted", "Approved"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .field(FieldDef::string("first_name").nullable())
                    .field(FieldDef::string("last_name").nullable())
                    .field(
                        FieldDef::string("full_name")
                            .computed(r#"first_name + " " + last_name"#),
                    )
                    .field(FieldDef::integer("qty").greater_than(0.0).nullable())
                    .field(FieldDef::assigned_to())
                    .validation_rule(ValidationRule {
                        when: Some(WhenClause {
                            field: "status".into(),
                            equals: Some(json!("Submitted")),
                            not_equals: None,
                        }),
                        require: vec!["title".into()],
                        ..Default::default()
                    })
                    .build(),
            )
            .notification(
                NotificationDef::new("ticket_ready", "")
                    .title("Ticket submitted")
                    .recipients(&["Admin", "Staff", "Manager"]),
            )
            .webhook(
                WebhookDef::new("ticket-ready", "", "test://ticket-ready").module("auto_runtime"),
            )
            .automation(
                AutomationDef::new(
                    "ticket_submitted_notify",
                    AutomationTrigger::event("workflow.transitioned"),
                )
                .conditions(Condition::all(vec![
                    Condition::field_equals("entity", "AutoTicket"),
                    Condition::field_equals("to_state", "Submitted"),
                ]))
                .action(AutomationAction::Notify {
                    notify: qefro_core::NotifyAction {
                        notification: Some("ticket_ready".into()),
                        ..Default::default()
                    },
                })
                .action(AutomationAction::create_activity("Ticket submitted"))
                .action(AutomationAction::SendWebhook {
                    send_webhook: qefro_core::WebhookAction {
                        webhook: Some("ticket-ready".into()),
                        name: None,
                    },
                }),
            )
            .automation(
                AutomationDef::new(
                    "ticket_created_activity",
                    AutomationTrigger::event("entity.created"),
                )
                .conditions(Condition::field_equals("entity", "AutoTicket"))
                .action(AutomationAction::create_activity("Ticket opened")),
            )
            .automation(
                AutomationDef::new(
                    "daily_ping",
                    AutomationTrigger::scheduled("* * * * *"),
                )
                .action(AutomationAction::notify("Staff")),
            )
            .build(),
    )
    .workflow(
        WorkflowDef::new("auto_ticket", "AutoTicket", "Draft")
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
                    .roles(&["Manager"]),
            ),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "AutoTicket"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "AutoTicket"))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "automation-runtime-test-secret".into(),
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

async fn drain_jobs(state: &qefro_api::AppState) {
    for _ in 0..1000 {
        match state
            .entities
            .job_queue()
            .process_one(&state.entities.job_handlers())
            .await
        {
            Ok(true) => {}
            _ => break,
        }
    }
}

#[tokio::test]
async fn validation_greater_than_and_computed_name() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let token = register(&router, &format!("a-{suffix}@ex.com"), &format!("at-{suffix}")).await;
    let (status, body) = json(
        clone_router(&router),
        post(
            "/api/v1/auto-tickets",
            Some(&token),
            json!({ "title": "Hello", "qty": 0, "first_name": "Ada", "last_name": "Lovelace" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"].as_str(), Some("validation_failed"));

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/auto-tickets",
            Some(&token),
            json!({ "title": "Hello", "qty": 2, "first_name": "Ada", "last_name": "Lovelace" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["full_name"], json!("Ada Lovelace"));
}

#[tokio::test]
async fn workflow_automation_notify_activity_webhook_and_idempotency() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let token = register(&router, &format!("b-{suffix}@ex.com"), &format!("bt-{suffix}")).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/auto-tickets",
            Some(&token),
            json!({ "title": "Need prep", "qty": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, moved) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/auto-tickets/{id}/transition"),
            Some(&token),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    drain_jobs(&state).await;

    let (status, notes) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes}");
    let items = notes["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().any(|n| n["title"].as_str() == Some("Ticket submitted")),
        "{notes}"
    );

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/auto-tickets/{id}/activity"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        acts.iter()
            .any(|a| a["message"].as_str() == Some("Ticket opened")),
        "entity.created automation: {activity}"
    );
    assert!(
        acts.iter()
            .any(|a| a["message"].as_str() == Some("Ticket submitted")),
        "{activity}"
    );

    let (status, events) = json(
        clone_router(&router),
        get("/api/v1/events", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{events}");
    let ev = events["items"].as_array().cloned().unwrap_or_default();
    assert!(ev.iter().any(|e| {
        e["event_type"] == "workflow.transitioned" && e.get("record_id").is_some()
    }));

    let before = items.len();
    let _ = state.entities.dispatch_outbox().await;
    drain_jobs(&state).await;
    let (status, notes2) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes2}");
    let after = notes2["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(after, before, "retry must not duplicate notifications");

    drain_jobs(&state).await;
    let (status, deliveries) = json(
        clone_router(&router),
        get("/api/v1/webhooks/ticket-ready/deliveries", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deliveries}");
    let rows = deliveries["deliveries"].as_array().cloned().unwrap_or_default();
    assert!(
        !rows.is_empty(),
        "webhook should be delivered via JobQueue: {deliveries}"
    );
}

#[tokio::test]
async fn tenant_isolation_and_studio_inspect() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let a = register(&router, &format!("c-{suffix}@ex.com"), &format!("ct-a-{suffix}")).await;
    let b = register(&router, &format!("d-{suffix}@ex.com"), &format!("ct-b-{suffix}")).await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/auto-tickets",
            Some(&a),
            json!({ "title": "A only", "qty": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    let _ = json(
        clone_router(&router),
        post(
            &format!("/api/v1/auto-tickets/{id}/transition"),
            Some(&a),
            json!({ "transition": "submit" }),
        ),
    )
    .await;
    drain_jobs(&state).await;

    let (status, notes_b) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes_b}");
    let items = notes_b["items"].as_array().cloned().unwrap_or_default();
    assert!(
        !items.iter().any(|n| n["title"].as_str() == Some("Ticket submitted")),
        "tenant B must not see tenant A notifications: {notes_b}"
    );

    let (status, studio) = json(
        clone_router(&router),
        get("/api/v1/studio/automations", Some(&a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{studio}");
    let autos = studio["automations"].as_array().cloned().unwrap_or_default();
    assert!(autos
        .iter()
        .any(|d| d["name"] == "ticket_submitted_notify"));
    let blob = studio.to_string();
    for key in ["password", "password_hash", "secret_env", "storage_key", "session"] {
        assert!(!blob.contains(key), "secret leaked in studio automations: {studio}");
    }

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&a))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], json!("1"));
    let entities = ui["entities"].as_array().cloned().unwrap_or_default();
    let ticket = entities
        .iter()
        .find(|e| e["entity"] == "AutoTicket")
        .expect("entity");
    assert_eq!(ticket["capabilities"]["actions"], json!(true));

    let n = state.automation.enqueue_scheduled().await.expect("sched");
    assert!(n >= 1, "scheduled automations should enqueue");
}

#[tokio::test]
async fn workflow_status_cannot_be_patched() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let token = register(&router, &format!("e-{suffix}@ex.com"), &format!("et-{suffix}")).await;
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/auto-tickets",
            Some(&token),
            json!({ "title": "Nope", "qty": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    let (status, body) = json(
        clone_router(&router),
        Request::builder()
            .method("PATCH")
            .uri(&format!("/api/v1/auto-tickets/{id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(json!({ "status": "Approved" }).to_string()))
            .unwrap(),
    )
    .await;
    assert!(status.is_client_error(), "{body}");
}
