//! Scheduling runtime: conflicts, race, tenant isolation, availability.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    AppModule, EntityDef, FieldDef, SchedulingConfig, WorkingHours, UI_SCHEMA_VERSION,
};
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
        AppModule::new("sched_runtime")
            .entity(
                EntityDef::new("SchedRoom")
                    .table_name("sched_rooms")
                    .slug_name("sched-rooms")
                    .display_field("code")
                    .field(FieldDef::string("code").required())
                    .field(FieldDef::integer("seats").required())
                    .build(),
            )
            .entity(
                EntityDef::new("SchedBooking")
                    .table_name("sched_bookings")
                    .slug_name("sched-bookings")
                    .display_field("guest_name")
                    .field(FieldDef::string("guest_name").required().searchable())
                    .field(FieldDef::many_to_one("room_id", "SchedRoom").required())
                    .field(FieldDef::date("starts_on").required())
                    .field(FieldDef::time("start_time").required())
                    .field(FieldDef::time("end_time").nullable())
                    .field(FieldDef::integer("party_size").required())
                    .field(
                        FieldDef::enum_values(
                            "status",
                            vec!["Pending", "Confirmed", "Cancelled", "Completed"],
                        )
                        .required()
                        .default_value(json!("Pending")),
                    )
                    .scheduling(
                        SchedulingConfig::new("starts_on")
                            .time_field("start_time")
                            .end_time_field("end_time")
                            .resource("room_id")
                            .capacity("party_size", "seats")
                            .conflict()
                            .calendar()
                            .duration_minutes(60)
                            .slot_interval_minutes(30)
                            .working_hours(WorkingHours::everyday("09:00", "17:00")),
                    )
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "SchedRoom"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "SchedBooking"))
    .permission(PermissionGrant::new(
        ROLE_STAFF,
        "SchedBooking",
        vec![Action::Read],
    ))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "sched-runtime-test-secret".into(),
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

async fn create_room(router: &axum::Router, token: &str, code: &str, seats: i64) -> String {
    let (status, body) = json(
        clone_router(router),
        post(
            "/api/v1/sched-rooms",
            Some(token),
            json!({ "code": code, "seats": seats }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn scheduling_conflict_race_capacity_tenant_and_availability() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token_a = register(
        &router,
        &format!("a-{suffix}@sched.test"),
        &format!("sa-{suffix}"),
    )
    .await;
    let token_b = register(
        &router,
        &format!("b-{suffix}@sched.test"),
        &format!("sb-{suffix}"),
    )
    .await;

    let (status, meta) = json(
        clone_router(&router),
        get("/api/v1/meta/ui", Some(&token_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{meta}");
    assert_eq!(meta["schema_version"], UI_SCHEMA_VERSION);
    let booking_meta = meta["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == "SchedBooking")
        .cloned()
        .unwrap();
    assert_eq!(booking_meta["capabilities"]["scheduling"], true);
    assert_eq!(booking_meta["scheduling"]["conflict"], true);

    let room_a = create_room(&router, &token_a, "Room-A", 10).await;
    let room_b = create_room(&router, &token_b, "Room-B", 10).await;

    let booking = json!({
        "guest_name": "Ahmed",
        "room_id": room_a,
        "starts_on": "2026-08-31",
        "start_time": "10:00",
        "end_time": "11:00",
        "party_size": 4
    });
    let (status, created) = json(
        clone_router(&router),
        post("/api/v1/sched-bookings", Some(&token_a), booking.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let overlap = json!({
        "guest_name": "Sara",
        "room_id": room_a,
        "starts_on": "2026-08-31",
        "start_time": "10:30",
        "end_time": "11:30",
        "party_size": 2
    });
    let (status, conflict) = json(
        clone_router(&router),
        post("/api/v1/sched-bookings", Some(&token_a), overlap),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["error"], "scheduling_conflict");
    assert!(conflict["message"]
        .as_str()
        .unwrap()
        .contains("already booked"));

    let adjacent = json!({
        "guest_name": "Adjacent",
        "room_id": room_a,
        "starts_on": "2026-08-31",
        "start_time": "11:00",
        "end_time": "12:00",
        "party_size": 2
    });
    let (status, ok) = json(
        clone_router(&router),
        post("/api/v1/sched-bookings", Some(&token_a), adjacent),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ok}");

    let too_big = json!({
        "guest_name": "Crowd",
        "room_id": room_a,
        "starts_on": "2026-08-31",
        "start_time": "13:00",
        "end_time": "14:00",
        "party_size": 12
    });
    let (status, cap) = json(
        clone_router(&router),
        post("/api/v1/sched-bookings", Some(&token_a), too_big),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{cap}");
    assert_eq!(cap["error"], "scheduling_capacity");

    let (status, slots) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/sched-bookings/availability?date=2026-08-31&room_id={room_a}"),
            Some(&token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{slots}");
    let list = slots["slots"].as_array().unwrap();
    assert!(list
        .iter()
        .any(|s| s["start"] == "10:00" && s["available"] == false));
    assert!(list
        .iter()
        .any(|s| s["start"] == "09:00" && s["available"] == true));

    let (status, leak) = json(
        clone_router(&router),
        get("/api/v1/sched-bookings", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{leak}");
    assert_eq!(leak["items"].as_array().unwrap().len(), 0);

    let (status, other) = json(
        clone_router(&router),
        post(
            "/api/v1/sched-bookings",
            Some(&token_b),
            json!({
                "guest_name": "Other tenant",
                "room_id": room_b,
                "starts_on": "2026-08-31",
                "start_time": "10:00",
                "end_time": "11:00",
                "party_size": 2
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{other}");

    let room_race = create_room(&router, &token_a, "Room-Race", 4).await;
    let payload = json!({
        "guest_name": "Racer",
        "room_id": room_race,
        "starts_on": "2026-08-31",
        "start_time": "15:00",
        "end_time": "16:00",
        "party_size": 2
    });
    let req_one = clone_router(&router).oneshot(post(
        "/api/v1/sched-bookings",
        Some(&token_a),
        payload.clone(),
    ));
    let req_two =
        clone_router(&router).oneshot(post("/api/v1/sched-bookings", Some(&token_a), payload));
    let (one, two) = tokio::join!(req_one, req_two);
    let mut statuses = Vec::new();
    for response in [one.unwrap(), two.unwrap()] {
        statuses.push(response.status());
    }
    statuses.sort();
    assert!(
        statuses.contains(&StatusCode::CREATED) && statuses.contains(&StatusCode::CONFLICT),
        "expected one success and one conflict, got {statuses:?}"
    );
}
