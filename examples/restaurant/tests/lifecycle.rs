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
