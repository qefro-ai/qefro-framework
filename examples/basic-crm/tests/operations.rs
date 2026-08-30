use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, QefroRuntime};
use qefro_crm::installed;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
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

fn post(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
}

#[tokio::test]
async fn crm_operations_without_framework_core_changes() {
    let url = db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(installed());
    let (router, _) = rt.build().await.unwrap();
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, auth) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ada",
                    "email": format!("crm-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": format!("C-{suffix}"),
                    "tenant_slug": format!("c-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();

    let lead = json(
        clone_router(&router),
        post(
            "/api/v1/leads",
            token,
            json!({ "title": "Acme", "company": "Acme Inc", "email": format!("lead-{suffix}@acme.test") }),
        ),
    )
    .await
    .1;
    let lead_id = lead["id"].as_str().unwrap();
    let (status, contacted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/leads/{lead_id}/transition"),
            token,
            json!({ "transition": "contact" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{contacted}");

    let (status, converted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/leads/{lead_id}/actions/convert"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{converted}");
    assert_eq!(converted["status"], "Qualified");
    assert_eq!(converted["_operation"]["status"], "completed");
    let (status, tasks) = json(
        clone_router(&router),
        get("/api/v1/tasks", token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tasks}");
    assert!(
        tasks["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["title"].as_str().unwrap_or("").contains("Onboard")),
        "convert should create a follow-up task: {tasks}"
    );

    let lead2 = json(
        clone_router(&router),
        post(
            "/api/v1/leads",
            token,
            json!({ "title": "Beta", "company": "Beta" }),
        ),
    )
    .await
    .1;
    json(
        clone_router(&router),
        post(
            &format!("/api/v1/leads/{}/transition", lead2["id"].as_str().unwrap()),
            token,
            json!({ "transition": "contact" }),
        ),
    )
    .await;
    let (status, qualified) = json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/leads/{}/actions/qualify",
                lead2["id"].as_str().unwrap()
            ),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{qualified}");
    assert_eq!(qualified["status"], "Qualified");

    let opp = json(
        clone_router(&router),
        post(
            "/api/v1/opportunities",
            token,
            json!({ "name": "Deal", "amount": 1000 }),
        ),
    )
    .await
    .1;
    json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/opportunities/{}/transition",
                opp["id"].as_str().unwrap()
            ),
            token,
            json!({ "transition": "qualify" }),
        ),
    )
    .await;
    let (status, won) = json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/opportunities/{}/actions/win",
                opp["id"].as_str().unwrap()
            ),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{won}");
    assert_eq!(won["status"], "Won");

    let opp2 = json(
        clone_router(&router),
        post(
            "/api/v1/opportunities",
            token,
            json!({ "name": "Lost deal" }),
        ),
    )
    .await
    .1;
    let (status, lost) = json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/opportunities/{}/actions/lose",
                opp2["id"].as_str().unwrap()
            ),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lost}");
    assert_eq!(lost["status"], "Lost");

    let activity = json(
        clone_router(&router),
        post(
            "/api/v1/activities",
            token,
            json!({ "kind": "call", "subject": "Follow up" }),
        ),
    )
    .await
    .1;
    let (status, done) = json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/activities/{}/actions/complete",
                activity["id"].as_str().unwrap()
            ),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["done"], true);
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn customer_created_records_activity_via_automation() {
    let url = db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(installed());
    let (router, _) = rt.build().await.unwrap();
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, auth) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ada",
                    "email": format!("crm-auto-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": format!("CA-{suffix}"),
                    "tenant_slug": format!("ca-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/crm-customers",
            token,
            json!({ "name": "Acme Foods", "email": format!("acme-{suffix}@ex.com") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    let id = customer["id"].as_str().unwrap();

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/crm-customers/{id}/activity"), token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        acts.iter()
            .any(|a| a["message"].as_str() == Some("Customer created")),
        "CRM automation should record activity: {activity}"
    );
}
