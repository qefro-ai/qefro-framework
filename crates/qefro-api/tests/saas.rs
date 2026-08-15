use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef};
use qefro_permissions::{PermissionGrant, ROLE_MANAGER};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn saas_apps() -> (InstalledApp, InstalledApp) {
    let restaurant = InstalledApp::new(
        AppModule::new("restaurant")
            .label("Restaurant")
            .entity(
                EntityDef::new("Reservation")
                    .table_name("saas_reservations")
                    .slug_name("reservations")
                    .field(FieldDef::string("name").required())
                    .build(),
            )
            .dashboard(
                qefro_core::DashboardDef::new("restaurant-ops", "Restaurant").module("restaurant"),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_MANAGER, "Reservation"));
    let crm = InstalledApp::new(
        AppModule::new("crm")
            .label("CRM")
            .entity(
                EntityDef::new("Lead")
                    .table_name("saas_leads")
                    .slug_name("leads")
                    .field(FieldDef::string("title").required())
                    .build(),
            )
            .dashboard(qefro_core::DashboardDef::new("crm-ops", "CRM").module("crm"))
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_MANAGER, "Lead"));
    (restaurant, crm)
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

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
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

fn patch(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn register(router: &axum::Router, tag: &str, suffix: &str) -> (String, String) {
    let (status, body) = json(
        clone_router(router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": tag,
                    "email": format!("{tag}-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": tag,
                    "tenant_slug": format!("{}-{suffix}", tag.to_ascii_lowercase())
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["access_token"].as_str().unwrap().to_string(),
        body["tenant_id"].as_str().unwrap_or("").to_string(),
    )
}

#[tokio::test]
async fn saas_tenants_are_isolated_and_entitled() {
    let Some(url) = db_url() else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    let (restaurant, crm) = saas_apps();
    rt.install(restaurant);
    rt.install(crm);
    let (router, _) = rt.build().await.unwrap();
    let suffix = &Uuid::new_v4().to_string()[..8];

    let (token_a, _) = register(&router, "Seeni Bhai", suffix).await;
    let (token_b, _) = register(&router, "ABC Traders", suffix).await;

    let (status, _) = json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/apps",
            &token_a,
            json!({ "enabled_apps": ["restaurant"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/branding",
            &token_a,
            json!({
                "company_name": "Seeni Bhai",
                "primary_color": "#9a3412",
                "logo": "https://example.test/seeni.png"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    json(
        clone_router(&router),
        patch(
            "/api/v1/tenants/me/config",
            &token_a,
            json!({
                "branding": { "company_name": "Seeni Bhai", "primary_color": "#9a3412" },
                "ui_config": {
                    "navigation": ["reservations"],
                    "terminology": { "Reservation": "Booking" },
                    "default_dashboard": "restaurant-ops"
                },
                "enabled_apps": ["restaurant"],
                "business": { "timezone": "Asia/Kolkata", "currency": "INR", "locale": "en-IN" },
                "features": { "flags": { "agent_actions": true } }
            }),
        ),
    )
    .await;

    json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/apps",
            &token_b,
            json!({ "enabled_apps": ["crm"] }),
        ),
    )
    .await;
    json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/branding",
            &token_b,
            json!({
                "company_name": "ABC Traders",
                "primary_color": "#1d4ed8"
            }),
        ),
    )
    .await;

    let (status, ui_a) = json(clone_router(&router), get("/api/v1/meta/ui", &token_a)).await;
    assert_eq!(status, StatusCode::OK, "{ui_a}");
    let slugs_a: Vec<&str> = ui_a["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["slug"].as_str())
        .collect();
    assert!(slugs_a.contains(&"reservations"), "{ui_a}");
    assert!(!slugs_a.contains(&"leads"), "{ui_a}");
    assert_eq!(ui_a["branding"]["company_name"], "Seeni Bhai");
    let reservation = ui_a["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["slug"] == "reservations")
        .unwrap();
    assert_eq!(reservation["label"], "Booking");

    let (_status, ui_b) = json(clone_router(&router), get("/api/v1/meta/ui", &token_b)).await;
    let slugs_b: Vec<&str> = ui_b["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["slug"].as_str())
        .collect();
    assert!(slugs_b.contains(&"leads"), "{ui_b}");
    assert!(!slugs_b.contains(&"reservations"), "{ui_b}");
    assert_eq!(ui_b["branding"]["company_name"], "ABC Traders");

    let (status, brand_b) =
        json(clone_router(&router), get("/api/v1/tenant/branding", &token_a)).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(brand_b["company_name"], "ABC Traders");

    let (status, leaked) = json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/branding",
            &token_a,
            json!({ "company_name": "hack", "tenant_id": Uuid::new_v4() }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{leaked}");

    let (_status, dash_a) =
        json(clone_router(&router), get("/api/v1/meta/dashboards", &token_a)).await;
    let names: Vec<&str> = dash_a["dashboards"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert!(names.contains(&"restaurant-ops"));
    assert!(!names.contains(&"crm-ops"));

    let (status, crm_dash) =
        json(clone_router(&router), get("/api/v1/dashboards/crm-ops", &token_a)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{crm_dash}");

    let (status, created) = json(
        clone_router(&router),
        post("/api/v1/reservations", &token_a, json!({ "name": "Table 1" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, denied) = json(
        clone_router(&router),
        post("/api/v1/leads", &token_a, json!({ "title": "Nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{denied}");

    let (_status, tools_a) = json(clone_router(&router), get("/api/v1/tools", &token_a)).await;
    let tool_names: Vec<&str> = tools_a["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(tool_names.iter().any(|n| n.contains("reservation")));
    assert!(!tool_names.iter().any(|n| n.contains("lead")));

    let (status, spoof) = json(
        clone_router(&router),
        Request::builder()
            .method("GET")
            .uri("/api/v1/tenant/branding")
            .header("authorization", format!("Bearer {token_a}"))
            .header("x-tenant-id", Uuid::new_v4().to_string())
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{spoof}");
    assert_eq!(spoof["company_name"], "Seeni Bhai");

    let (status, qspoof) = json(
        clone_router(&router),
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/reservations?tenant_id={}", Uuid::new_v4()))
            .header("authorization", format!("Bearer {token_a}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{qspoof}");

    json(
        clone_router(&router),
        patch(
            "/api/v1/tenant/features",
            &token_a,
            json!({ "flags": { "agent_actions": false } }),
        ),
    )
    .await;
    let (status, agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/create_reservation/invoke",
            &token_a,
            json!({ "name": "x" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{agent}");

    let (status, tenants) = json(clone_router(&router), get("/api/v1/tenants", &token_a)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tenants.as_array().unwrap().len(), 1);

    let started = std::time::Instant::now();
    let mut samples = Vec::new();
    for _ in 0..8 {
        let t0 = std::time::Instant::now();
        let _ = json(clone_router(&router), get("/api/v1/tenant", &token_a)).await;
        samples.push(t0.elapsed().as_millis() as u64);
    }
    samples.sort_unstable();
    eprintln!(
        "saas_tenant_context p50={}ms p99={}ms total_ms={}",
        samples[samples.len() / 2],
        samples[samples.len() - 1],
        started.elapsed().as_millis()
    );
}
