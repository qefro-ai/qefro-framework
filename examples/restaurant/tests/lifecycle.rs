use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, QefroRuntime};
use qefro_restaurant::installed;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
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
    let id = reservation["id"].as_str().unwrap();
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
