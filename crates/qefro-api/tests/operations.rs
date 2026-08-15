use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{
    Config, InstalledApp, OperationCtx, OperationHandler, QefroRuntime,
};
use qefro_core::{AppModule, EntityDef, FieldDef, OperationDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

struct ConfirmBooking;
struct ExplodeBooking;

#[async_trait]
impl OperationHandler for ConfirmBooking {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx.uuid_field("room_id")?;
        let room = ctx.get("OpRoom", room_id).await?;
        if room.get("status").and_then(|v| v.as_str()) != Some("available") {
            return Err(OperationCtx::fail(
                "table_unavailable",
                "The selected room is not available",
            ));
        }
        ctx.update("OpRoom", room_id, json!({ "status": "reserved" }))
            .await?;
        ctx.apply_transition("confirm")?;
        ctx.emit("booking.confirmed", json!({ "entity_id": ctx.record_id()? }));
        ctx.enqueue_job(
            "notify_booking_confirmed",
            json!({ "entity": "OpBooking", "entity_id": ctx.record_id()? }),
        );
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for ExplodeBooking {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx.uuid_field("room_id")?;
        ctx.update("OpRoom", room_id, json!({ "status": "reserved" }))
            .await?;
        ctx.apply_transition("confirm")?;
        Err(OperationCtx::fail("forced_failure", "handler failed after mutation"))
    }
}

fn ops_app() -> InstalledApp {
    let module = AppModule::new("ops_test")
        .entity(
            EntityDef::new("OpRoom")
                .table_name("op_rooms")
                .slug_name("op-rooms")
                .field(FieldDef::string("code").required().unique())
                .field(
                    FieldDef::enum_values("status", vec!["available", "reserved", "occupied"])
                        .required()
                        .default_value(json!("available")),
                )
                .build(),
        )
        .entity(
            EntityDef::new("OpBooking")
                .table_name("op_bookings")
                .slug_name("op-bookings")
                .workflow("op_booking")
                .field(FieldDef::many_to_one("room_id", "OpRoom").required())
                .field(
                    FieldDef::enum_values("status", vec!["Pending", "Confirmed", "Cancelled"])
                        .required()
                        .default_value(json!("Pending")),
                )
                .build(),
        )
        .build();
    InstalledApp::new(module)
        .workflow(
            WorkflowDef::new("op_booking", "OpBooking", "Pending")
                .transition(
                    TransitionDef::new("confirm", "Pending", "Confirmed").roles(&["Manager", "Staff"]),
                )
                .transition(TransitionDef::new("cancel", "Pending", "Cancelled")),
        )
        .permission(PermissionGrant::crud(ROLE_MANAGER, "OpRoom"))
        .permission(PermissionGrant::crud(ROLE_MANAGER, "OpBooking"))
        .permission(PermissionGrant::crud(ROLE_STAFF, "OpRoom"))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "OpBooking",
            vec![Action::Read, Action::List, Action::Update],
        ))
        .operation(
            OperationDef::new("confirm", "OpBooking")
                .label("Confirm")
                .roles(&["Manager", "Staff"])
                .transition("confirm")
                .event("booking.confirmed")
                .job("notify_booking_confirmed"),
            ConfirmBooking,
        )
        .operation(
            OperationDef::new("explode", "OpBooking")
                .label("Explode")
                .roles(&["Manager"])
                .transition("confirm"),
            ExplodeBooking,
        )
}

async fn runtime() -> axum::Router {
    let url = test_db_url().expect("DATABASE_URL");
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
    });
    rt.install(ops_app());
    let (router, _) = rt.build().await.expect("build");
    router
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

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
}

#[tokio::test]
async fn operations_pipeline_transactions_events_and_agent() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];

    let (status, body_a) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("ops-a-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("OA-{suffix}"),
                "tenant_slug": format!("oa-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body_a}");
    let token_a = body_a["access_token"].as_str().unwrap();

    let (status, body_b) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Bob",
                "email": format!("ops-b-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("OB-{suffix}"),
                "tenant_slug": format!("ob-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body_b}");
    let token_b = body_b["access_token"].as_str().unwrap();

    let (status, staff_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(token_a),
            json!({
                "name": "Staff",
                "email": format!("ops-staff-{suffix}@example.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{staff_user}");
    let (status, staff_body) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("ops-staff-{suffix}@example.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_body}");
    let staff = staff_body["access_token"].as_str().unwrap();

    let (status, room) = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(token_a),
            json!({ "code": format!("R-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{room}");
    let room_id = room["id"].as_str().unwrap();

    let (status, booking) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(token_a),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["id"].as_str().unwrap();
    assert_eq!(booking["status"], "Pending");

    let (status, ops) = json(clone_router(&router), get("/api/v1/operations", Some(token_a))).await;
    assert_eq!(status, StatusCode::OK, "{ops}");
    let names: Vec<&str> = ops["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["name"].as_str().filter(|_| o["entity"] == "OpBooking"))
        .collect();
    assert!(names.contains(&"confirm"));

    let (status, actions) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/op-bookings/{booking_id}/actions"),
            Some(token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{actions}");
    assert!(actions["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["name"] == "confirm"));

    let (status, tenant_denied) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/confirm"),
            Some(token_b),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{tenant_denied}");

    let (status, exploded) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/explode"),
            Some(token_a),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{exploded}");
    assert_eq!(exploded["error"], "business_rule_failed");

    let (status, after_fail) = json(
        clone_router(&router),
        get(&format!("/api/v1/op-bookings/{booking_id}"), Some(token_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{after_fail}");
    assert_eq!(after_fail["status"], "Pending");
    let (_status, room_after_fail) = json(
        clone_router(&router),
        get(&format!("/api/v1/op-rooms/{room_id}"), Some(token_a)),
    )
    .await;
    assert_eq!(room_after_fail["status"], "available", "{room_after_fail}");

    let (status, events_after_fail) =
        json(clone_router(&router), get("/api/v1/events", Some(token_a))).await;
    assert_eq!(status, StatusCode::OK);
    let confirmed_before = events_after_fail["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "booking.confirmed" && e["entity_id"] == booking_id);
    assert!(!confirmed_before, "{events_after_fail}");

    let (status, confirmed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/confirm"),
            Some(staff),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "Confirmed");
    assert!(confirmed["_actions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["name"] != "confirm"));

    let (_status, room_after) = json(
        clone_router(&router),
        get(&format!("/api/v1/op-rooms/{room_id}"), Some(token_a)),
    )
    .await;
    assert_eq!(room_after["status"], "reserved", "{room_after}");

    let (status, invalid_state) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/confirm"),
            Some(token_a),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{invalid_state}");

    let (_status, events) = json(clone_router(&router), get("/api/v1/events", Some(token_a))).await;
    assert!(
        events["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "booking.confirmed" && e["entity_id"] == booking_id),
        "{events}"
    );

    let (status, tools) = json(clone_router(&router), get("/api/v1/tools", Some(token_a))).await;
    assert_eq!(status, StatusCode::OK, "{tools}");
    assert!(tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "confirm_op_booking"));

    let (status, room2) = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(token_a),
            json!({ "code": format!("R2-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{room2}");
    let (status, booking2) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(token_a),
            json!({ "room_id": room2["id"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking2}");
    let (status, agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/confirm_op_booking/invoke",
            Some(token_a),
            json!({ "id": booking2["id"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{agent}");
    assert_eq!(agent["data"]["status"], "Confirmed");

    let (status, booking3) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(token_a),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking3}");
    let (status, unavailable) = json(
        clone_router(&router),
        post(
            &format!(
                "/api/v1/op-bookings/{}/actions/confirm",
                booking3["id"].as_str().unwrap()
            ),
            Some(token_a),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{unavailable}");
    assert_eq!(unavailable["error"], "business_rule_failed");
}
