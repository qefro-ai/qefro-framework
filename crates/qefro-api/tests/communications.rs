//! Communication runtime: templates, jobs, tenant isolation, idempotency.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    AppModule, CommunicationDef, EntityDef, FieldDef, CHANNEL_EMAIL, CHANNEL_IN_APP,
    PURPOSE_TRANSACTIONAL, UI_SCHEMA_VERSION,
};
use qefro_events::DomainEvent;
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
        AppModule::new("comm_runtime")
            .entity(
                EntityDef::new("CommCustomer")
                    .table_name("comm_customers")
                    .slug_name("comm-customers")
                    .field(FieldDef::string("name").required())
                    .field(FieldDef::string("email").email().nullable())
                    .field(FieldDef::string("phone").nullable())
                    .field(
                        FieldDef::enum_values(
                            "communication_channel",
                            vec!["in_app", "email", "sms", "whatsapp", "none"],
                        )
                        .nullable(),
                    )
                    .field(FieldDef::boolean("marketing_opt_out").nullable())
                    .build(),
            )
            .entity(
                EntityDef::new("CommOrder")
                    .table_name("comm_orders")
                    .slug_name("comm-orders")
                    .field(FieldDef::string("doc_no").nullable())
                    .field(FieldDef::many_to_one("customer_id", "CommCustomer").required())
                    .field(FieldDef::currency("total").nullable())
                    .build(),
            )
            .communication(
                CommunicationDef::new("comm_order_confirmed", "order.confirmed", "CommOrder")
                    .channels(&[CHANNEL_EMAIL, CHANNEL_IN_APP])
                    .purpose(PURPOSE_TRANSACTIONAL)
                    .subject("Order {{ number }}")
                    .body("Hello {{ customer.name }}, order {{ number }} total {{ total | currency }}")
                    .recipient_path("customer")
                    .preferred_channel_field("communication_channel")
                    .opt_out_field("marketing_opt_out")
                    .module("comm_runtime"),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "CommCustomer"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "CommOrder"))
    .permission(PermissionGrant::new(ROLE_STAFF, "CommOrder", vec![Action::Read]))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "comm-runtime-test-secret".into(),
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
    body["access_token"].as_str().unwrap().to_string()
}

async fn drain_jobs(state: &qefro_api::AppState) {
    let _ = state.entities.dispatch_outbox().await;
    for _ in 0..200 {
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
async fn communication_runtime_tenant_jobs_and_idempotency() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token_a = register(
        &router,
        &format!("ca-{suffix}@example.com"),
        &format!("ca-{suffix}"),
    )
    .await;
    let token_b = register(
        &router,
        &format!("cb-{suffix}@example.com"),
        &format!("cb-{suffix}"),
    )
    .await;

    let (status, ui) = json(
        clone_router(&router),
        get("/api/v1/meta/ui", Some(&token_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], UI_SCHEMA_VERSION);
    let order_meta = ui["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == "CommOrder")
        .expect("CommOrder");
    assert_eq!(order_meta["capabilities"]["communication"], true);
    assert!(
        order_meta["communications"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "comm_order_confirmed"),
        "{order_meta}"
    );

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/comm-customers",
            Some(&token_a),
            json!({
                "name": "Ahmed",
                "email": format!("ahmed-{suffix}@example.com"),
                "phone": "+10000000000",
                "communication_channel": "email"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    let customer_id = customer["id"].as_str().unwrap();

    let (status, order) = json(
        clone_router(&router),
        post(
            "/api/v1/comm-orders",
            Some(&token_a),
            json!({
                "doc_no": "ORD-2026-00001",
                "customer_id": customer_id,
                "total": 18.50
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    let order_id = order["id"].as_str().unwrap();
    let order_uuid = Uuid::parse_str(order_id).unwrap();

    let (status, sent) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/comm-orders/{order_id}/actions/send_communication"),
            Some(&token_a),
            json!({ "template": "comm_order_confirmed", "channel": "email" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert!(!sent["queued"].as_array().unwrap().is_empty(), "{sent}");

    drain_jobs(&state).await;

    let (status, logs) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/comm-orders/{order_id}/communications"),
            Some(&token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{logs}");
    let items = logs["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{logs}");
    assert_eq!(items[0]["channel"], "email");
    assert_eq!(items[0]["status"], "sent");
    assert!(items[0].get("body").is_none());
    assert!(!items[0]["recipient"].as_str().unwrap_or("").is_empty());

    let (status, isolated) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/comm-orders/{order_id}/communications"),
            Some(&token_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{isolated}");

    let (status, search_b) = json(
        clone_router(&router),
        get("/api/v1/communications?entity=CommOrder", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_b}");
    assert!(
        search_b["items"].as_array().unwrap().is_empty(),
        "{search_b}"
    );

    let me = json(
        clone_router(&router),
        get("/api/v1/auth/me", Some(&token_a)),
    )
    .await;
    let tenant_id = Uuid::parse_str(me.1["tenant_id"].as_str().unwrap()).unwrap();
    let event = DomainEvent::new(
        "order.confirmed",
        "CommOrder",
        order_uuid,
        tenant_id,
        json!({ "entity_id": order_id }),
    );
    let def = state
        .communications_live()
        .into_iter()
        .find(|d| d.name == "comm_order_confirmed")
        .unwrap();
    let ctx = qefro_core::OpContext::worker(tenant_id, Uuid::nil());
    let first = qefro_db::enqueue_communication(
        &state.entities.job_queue(),
        &state.communications,
        &state.entities,
        &ctx,
        &def,
        Some(&event),
        order_uuid,
        Some("email"),
    )
    .await
    .unwrap();
    let second = qefro_db::enqueue_communication(
        &state.entities.job_queue(),
        &state.communications,
        &state.entities,
        &ctx,
        &def,
        Some(&event),
        order_uuid,
        Some("email"),
    )
    .await
    .unwrap();
    assert_eq!(first.len(), 1, "first enqueue should insert");
    assert!(second.is_empty(), "retried event must not duplicate");

    let (status, preview) = json(
        clone_router(&router),
        get(
            "/api/v1/studio/communications/comm_order_confirmed/preview",
            Some(&token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["sent"], false);
    assert!(
        preview["body"].as_str().unwrap().contains("Ahmed"),
        "{preview}"
    );
}
