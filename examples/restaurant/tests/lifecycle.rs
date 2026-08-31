use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, QefroRuntime};
use qefro_restaurant::installed;
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
async fn reservation_confirm_seat_complete_and_cancel() {
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
                    "email": format!("rest-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": format!("R-{suffix}"),
                    "tenant_slug": format!("r-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();

    let restaurant = json(
        clone_router(&router),
        post(
            "/api/v1/restaurants",
            token,
            json!({ "name": "Demo", "timezone": "UTC" }),
        ),
    )
    .await
    .1;
    assert!(
        restaurant.get("id").and_then(|v| v.as_str()).is_some(),
        "restaurant create failed: {restaurant}"
    );
    let branch = json(
        clone_router(&router),
        post(
            "/api/v1/branches",
            token,
            json!({
                "name": "Main",
                "restaurant_id": restaurant["id"]
            }),
        ),
    )
    .await
    .1;
    assert!(
        branch.get("id").and_then(|v| v.as_str()).is_some(),
        "branch create failed: {branch}"
    );
    let table = json(
        clone_router(&router),
        post(
            "/api/v1/tables",
            token,
            json!({
                "code": format!("T-{suffix}"),
                "branch_id": branch["id"],
                "seats": 4
            }),
        ),
    )
    .await
    .1;
    assert!(
        table.get("id").and_then(|v| v.as_str()).is_some(),
        "table create failed: {table}"
    );
    let customer = json(
        clone_router(&router),
        post(
            "/api/v1/customers",
            token,
            json!({
                "name": "Pat",
                "email": format!("pat-{suffix}@example.com")
            }),
        ),
    )
    .await
    .1;
    assert!(
        customer.get("id").and_then(|v| v.as_str()).is_some(),
        "customer create failed: {customer}"
    );
    let reservation = json(
        clone_router(&router),
        post(
            "/api/v1/reservations",
            token,
            json!({
                "customer_id": customer["id"],
                "table_id": table["id"],
                "reservation_date": "2026-08-20",
                "reservation_time": "19:00",
                "party_size": 2
            }),
        ),
    )
    .await
    .1;
    let id = reservation["id"]
        .as_str()
        .unwrap_or_else(|| panic!("reservation create failed: {reservation}"));
    assert_eq!(reservation["status"], "Pending");

    let (status, confirmed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/reservations/{id}/actions/confirm"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "Confirmed");

    let table_id = table["id"].as_str().unwrap();
    let reserved = json(
        clone_router(&router),
        Request::builder()
            .method("GET")
            .uri(&format!("/api/v1/tables/{table_id}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .1;
    assert_eq!(reserved["status"], "reserved");

    let (status, seated) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/reservations/{id}/actions/seat_customer"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{seated}");
    assert_eq!(seated["status"], "Seated");

    let (status, completed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/reservations/{id}/actions/complete"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["status"], "Completed");

    let (status, too_late) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/reservations/{id}/actions/cancel"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{too_late}");

    let reservation2 = json(
        clone_router(&router),
        post(
            "/api/v1/reservations",
            token,
            json!({
                "customer_id": customer["id"],
                "table_id": table["id"],
                "reservation_date": "2026-08-21",
                "reservation_time": "18:00",
                "party_size": 2
            }),
        ),
    )
    .await
    .1;
    let id2 = reservation2["id"].as_str().unwrap();
    let (status, cancelled) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/reservations/{id2}/actions/cancel"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "Cancelled");
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn drain_jobs(state: &qefro_api::AppState) {
    for _ in 0..32 {
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
async fn order_ready_automation_notifies_and_webhooks() {
    let url = db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(installed());
    let (router, state) = rt.build().await.unwrap();
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
                    "email": format!("kit-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": format!("K-{suffix}"),
                    "tenant_slug": format!("k-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();

    let restaurant = json(
        clone_router(&router),
        post(
            "/api/v1/restaurants",
            token,
            json!({ "name": "Kitchen", "timezone": "UTC" }),
        ),
    )
    .await
    .1;
    let category = json(
        clone_router(&router),
        post(
            "/api/v1/menu-categories",
            token,
            json!({ "name": "Mains", "restaurant_id": restaurant["id"] }),
        ),
    )
    .await
    .1;
    let menu = json(
        clone_router(&router),
        post(
            "/api/v1/menu-items",
            token,
            json!({
                "name": "Pizza",
                "category_id": category["id"],
                "price": 12,
                "available": true
            }),
        ),
    )
    .await
    .1;
    assert!(menu.get("id").is_some(), "{menu}");

    let (status, order) = json(
        clone_router(&router),
        post(
            "/api/v1/orders",
            token,
            json!({
                "order_date": "2026-08-30",
                "order_type": "Takeaway",
                "items": [{
                    "menu_item_id": menu["id"],
                    "quantity": 1,
                    "unit_price": 12
                }]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    assert_eq!(order["status"], "Draft");
    let id = order["id"].as_str().unwrap();

    let (status, confirmed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{id}/actions/confirm"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "Confirmed");

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/orders/{id}/activity"), token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        acts.iter()
            .any(|a| a["message"].as_str() == Some("Kitchen: order confirmed")),
        "{activity}"
    );

    let (status, notes) = json(clone_router(&router), get("/api/v1/notifications", token)).await;
    assert_eq!(status, StatusCode::OK, "{notes}");
    let items = notes["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items
            .iter()
            .any(|n| n["title"].as_str() == Some("Order confirmed")),
        "{notes}"
    );

    let (status, prep) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{id}/actions/start_preparation"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{prep}");
    assert_eq!(prep["status"], "Preparing");

    let (status, ready) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{id}/actions/mark_ready"),
            token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["status"], "Ready");
    drain_jobs(&state).await;

    let (status, notes2) = json(clone_router(&router), get("/api/v1/notifications", token)).await;
    assert_eq!(status, StatusCode::OK, "{notes2}");
    let items2 = notes2["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items2
            .iter()
            .any(|n| n["title"].as_str() == Some("Order is ready")),
        "{notes2}"
    );

    let (status, deliveries) = json(
        clone_router(&router),
        get("/api/v1/webhooks/order-ready/deliveries", token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deliveries}");
    let rows = deliveries["deliveries"].as_array().cloned().unwrap_or_default();
    assert!(!rows.is_empty(), "order-ready webhook: {deliveries}");
}

#[tokio::test]
async fn empty_tenant_branding_picks_up_restaurant_defaults() {
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
    let tenant_name = format!("Harbor Table {suffix}");

    let (status, auth) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ada",
                    "email": format!("brand-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": tenant_name,
                    "tenant_slug": format!("brand-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", token)).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], "1");
    assert_eq!(ui["branding"]["company_name"], tenant_name);
    assert_eq!(ui["branding"]["app_name"], "Qefro Kitchen");
    assert_eq!(ui["branding"]["primary_color"], "#9a3412");
    assert_eq!(ui["branding"]["accent_color"], "#c2410c");
    assert_eq!(ui["branding"]["secondary_color"], "#f4f1ea");
    assert!(ui["branding"]["logo"]
        .as_str()
        .is_some_and(|s| s.starts_with("data:image/svg+xml")));
    assert_eq!(ui["default_dashboard"], "restaurant-ops");
    let nav: Vec<&str> = ui["navigation"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        &nav[..5],
        ["orders", "reservations", "tables", "menu-items", "customers"]
    );
    assert!(nav.contains(&"sales-orders"), "{nav:?}");
    assert!(nav.contains(&"products"), "{nav:?}");
    let hidden: Vec<&str> = ui["hidden_entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(hidden.contains(&"people"));
    assert!(hidden.contains(&"users"));
}

async fn register(router: &axum::Router, prefix: &str) -> String {
    let suffix = &Uuid::new_v4().to_string()[..8];
    let (status, auth) = json(
        clone_router(router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ada",
                    "email": format!("{prefix}-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": format!("{prefix}-{suffix}"),
                    "tenant_slug": format!("{prefix}-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    auth["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn takeaway_walk_in_and_scheduled_pickup() {
    let url = db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(installed());
    let (router, _) = rt.build().await.unwrap();
    let token = register(&router, "takeaway").await;

    let restaurant = json(
        clone_router(&router),
        post(
            "/api/v1/restaurants",
            &token,
            json!({ "name": "Demo", "timezone": "UTC" }),
        ),
    )
    .await
    .1;
    let branch = json(
        clone_router(&router),
        post(
            "/api/v1/branches",
            &token,
            json!({ "name": "Main", "restaurant_id": restaurant["id"] }),
        ),
    )
    .await
    .1;
    let table = json(
        clone_router(&router),
        post(
            "/api/v1/tables",
            &token,
            json!({
                "code": format!("T-{}", &Uuid::new_v4().to_string()[..8]),
                "branch_id": branch["id"],
                "seats": 2
            }),
        ),
    )
    .await
    .1;
    let category = json(
        clone_router(&router),
        post(
            "/api/v1/menu-categories",
            &token,
            json!({ "name": "Mains", "restaurant_id": restaurant["id"] }),
        ),
    )
    .await
    .1;
    let menu = json(
        clone_router(&router),
        post(
            "/api/v1/menu-items",
            &token,
            json!({
                "name": "Burger",
                "category_id": category["id"],
                "price": 12
            }),
        ),
    )
    .await
    .1;
    let items = json!([{
        "menu_item_id": menu["id"],
        "quantity": 1,
        "unit_price": 12
    }]);

    let dine_in = json(
        clone_router(&router),
        post("/api/v1/orders", &token, json!({ "items": items })),
    )
    .await
    .1;
    assert_eq!(dine_in["order_type"], "Dine-in");
    assert_eq!(dine_in["status"], "Draft");
    let dine_id = dine_in["id"].as_str().unwrap();
    let (status, no_table) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{dine_id}/actions/confirm"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{no_table}");

    let (status, patched) = json(
        clone_router(&router),
        Request::builder()
            .method("PATCH")
            .uri(&format!("/api/v1/orders/{dine_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(json!({ "table_id": table["id"] }).to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    let (status, confirmed_dine) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{dine_id}/actions/confirm"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed_dine}");
    assert_eq!(confirmed_dine["status"], "Confirmed");

    let walk_in = json(
        clone_router(&router),
        post(
            "/api/v1/orders",
            &token,
            json!({
                "order_type": "Takeaway",
                "items": items
            }),
        ),
    )
    .await
    .1;
    assert_eq!(walk_in["order_type"], "Takeaway");
    let walk_id = walk_in["id"].as_str().unwrap();
    let (status, too_soon) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{walk_id}/actions/schedule"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{too_soon}");
    let (status, confirmed_walk) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{walk_id}/actions/confirm"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed_walk}");
    assert_eq!(confirmed_walk["status"], "Confirmed");
    assert!(confirmed_walk
        .get("table_id")
        .and_then(|v| v.as_str())
        .is_none());

    let booked = json(
        clone_router(&router),
        post(
            "/api/v1/orders",
            &token,
            json!({
                "order_type": "Takeaway",
                "pickup_at": "2026-08-28T18:30:00Z",
                "items": items
            }),
        ),
    )
    .await
    .1;
    let booked_id = booked["id"].as_str().unwrap();
    let (status, scheduled) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{booked_id}/actions/schedule"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{scheduled}");
    assert_eq!(scheduled["status"], "Scheduled");
    assert!(scheduled["pickup_at"].as_str().unwrap().contains("18:30"));

    let (status, confirmed_booked) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{booked_id}/actions/confirm"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed_booked}");
    assert_eq!(confirmed_booked["status"], "Confirmed");
    let (status, preparing) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{booked_id}/actions/start_preparation"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preparing}");
    let (status, ready) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{booked_id}/actions/mark_ready"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["status"], "Ready");
    let (status, completed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/orders/{booked_id}/actions/complete"),
            &token,
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["status"], "Completed");
    let (status, follow_ups) = json(
        clone_router(&router),
        get("/api/v1/tasks", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{follow_ups}");
    assert!(
        follow_ups["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["entity_id"] == booked_id && t["entity_type"] == "Order"),
        "complete order should create a related task: {follow_ups}"
    );
}
