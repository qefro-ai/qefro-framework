use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, OperationCtx, OperationHandler, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, OperationDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn test_db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

struct ConfirmBooking;
struct ExplodeBooking;
struct SeatBooking;
struct CompleteBooking;
struct CancelBooking;
struct StampNote;
struct TouchRoom;
struct CycleA;
struct CycleB;
struct CrossTenant;

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
        ctx.emit(
            "booking.confirmed",
            json!({ "entity_id": ctx.record_id()? }),
        );
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
        let booking_id = ctx.record_id()?;
        ctx.create(
            "OpNote",
            json!({ "booking_id": booking_id, "body": "should roll back" }),
        )
        .await?;
        ctx.update("OpRoom", room_id, json!({ "status": "reserved" }))
            .await?;
        ctx.apply_transition("confirm")?;
        Err(OperationCtx::fail(
            "forced_failure",
            "handler failed after mutation",
        ))
    }
}

#[async_trait]
impl OperationHandler for SeatBooking {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx.uuid_field("room_id")?;
        ctx.update("OpRoom", room_id, json!({ "status": "occupied" }))
            .await?;
        ctx.apply_transition("seat")?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CompleteBooking {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx.uuid_field("room_id")?;
        ctx.update("OpRoom", room_id, json!({ "status": "available" }))
            .await?;
        ctx.apply_transition("complete")?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CancelBooking {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        match ctx.status() {
            "Pending" => {
                ctx.apply_transition("cancel")?;
            }
            "Confirmed" => {
                ctx.apply_transition("cancel_confirmed")?;
                let room_id = ctx.uuid_field("room_id")?;
                ctx.update("OpRoom", room_id, json!({ "status": "available" }))
                    .await?;
            }
            _ => {
                return Err(OperationCtx::fail(
                    "invalid_state",
                    "Booking cannot be cancelled in the current state",
                ));
            }
        };
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for StampNote {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        ctx.create(
            "OpNote",
            json!({
                "booking_id": ctx.record_id()?,
                "body": ctx.input.get("body").cloned().unwrap_or(json!("stamped")),
            }),
        )
        .await?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for TouchRoom {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx.uuid_field("room_id")?;
        let expected = ctx.input.get("_expected_updated_at").cloned();
        let mut patch = json!({ "code": ctx.input.get("code").cloned().unwrap_or(json!("touched")) });
        if let Some(ts) = expected {
            patch
                .as_object_mut()
                .unwrap()
                .insert("_expected_updated_at".into(), ts);
        }
        ctx.update("OpRoom", room_id, patch).await?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CycleA {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let id = ctx.record_id()?;
        ctx.execute("OpBooking", id, "cycle_b", json!({})).await?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CycleB {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let id = ctx.record_id()?;
        ctx.execute("OpBooking", id, "cycle_a", json!({})).await?;
        Ok(ctx.record.clone())
    }
}

#[async_trait]
impl OperationHandler for CrossTenant {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let room_id = ctx
            .input
            .get("room_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| OperationCtx::fail("missing_room", "room_id required"))?;
        ctx.get("OpRoom", room_id).await?;
        Ok(ctx.record.clone())
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
                    FieldDef::enum_values(
                        "status",
                        vec!["Pending", "Confirmed", "Seated", "Completed", "Cancelled"],
                    )
                    .required()
                    .default_value(json!("Pending")),
                )
                .build(),
        )
        .entity(
            EntityDef::new("OpNote")
                .table_name("op_notes")
                .slug_name("op-notes")
                .field(FieldDef::many_to_one("booking_id", "OpBooking").required())
                .field(FieldDef::string("body").required())
                .build(),
        )
        .build();
    InstalledApp::new(module)
        .workflow(
            WorkflowDef::new("op_booking", "OpBooking", "Pending")
                .transition(
                    TransitionDef::new("confirm", "Pending", "Confirmed")
                        .roles(&["Manager", "Staff"]),
                )
                .transition(
                    TransitionDef::new("seat", "Confirmed", "Seated").roles(&["Manager", "Staff"]),
                )
                .transition(
                    TransitionDef::new("complete", "Seated", "Completed")
                        .roles(&["Manager", "Staff"]),
                )
                .transition(TransitionDef::new("cancel", "Pending", "Cancelled"))
                .transition(
                    TransitionDef::new("cancel_confirmed", "Confirmed", "Cancelled")
                        .roles(&["Manager"]),
                ),
        )
        .permission(PermissionGrant::crud(ROLE_MANAGER, "OpRoom"))
        .permission(PermissionGrant::crud(ROLE_MANAGER, "OpBooking"))
        .permission(PermissionGrant::crud(ROLE_MANAGER, "OpNote"))
        .permission(PermissionGrant::crud(ROLE_STAFF, "OpRoom"))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "OpBooking",
            vec![Action::Read, Action::List, Action::Update],
        ))
        .permission(PermissionGrant::new(
            ROLE_STAFF,
            "OpNote",
            vec![Action::Read, Action::List],
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
        .operation(
            OperationDef::new("seat_customer", "OpBooking")
                .label("Seat")
                .roles(&["Manager", "Staff"])
                .transition("seat")
                .event("booking.seated"),
            SeatBooking,
        )
        .operation(
            OperationDef::new("complete", "OpBooking")
                .label("Complete")
                .roles(&["Manager", "Staff"])
                .transition("complete")
                .event("booking.completed"),
            CompleteBooking,
        )
        .operation(
            OperationDef::new("cancel", "OpBooking")
                .label("Cancel")
                .event("booking.cancelled"),
            CancelBooking,
        )
        .operation(
            OperationDef::new("stamp_note", "OpBooking")
                .label("Stamp Note")
                .roles(&["Manager", "Staff"])
                .input_schema(json!({
                    "type": "object",
                    "properties": { "body": { "type": "string", "title": "Note" } }
                })),
            StampNote,
        )
        .operation(
            OperationDef::new("touch_room", "OpBooking")
                .label("Touch Room")
                .roles(&["Manager", "Staff"]),
            TouchRoom,
        )
        .operation(
            OperationDef::new("cycle_a", "OpBooking")
                .label("Cycle A")
                .roles(&["Manager"]),
            CycleA,
        )
        .operation(
            OperationDef::new("cycle_b", "OpBooking")
                .label("Cycle B")
                .roles(&["Manager"]),
            CycleB,
        )
        .operation(
            OperationDef::new("cross_tenant", "OpBooking")
                .label("Cross Tenant")
                .roles(&["Manager"]),
            CrossTenant,
        )
}

async fn runtime() -> axum::Router {
    let url = test_db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
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
    post_with(path, token, body, None)
}

fn post_with(
    path: &str,
    token: Option<&str>,
    body: Value,
    idempotency: Option<&str>,
) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    if let Some(key) = idempotency {
        b = b.header("Idempotency-Key", key);
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

fn delete(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("DELETE").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

async fn register_admin(router: &axum::Router, suffix: &str, tag: &str) -> String {
    let (status, body) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": tag,
                "email": format!("{tag}-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("{tag}-{suffix}"),
                "tenant_slug": format!("{tag}-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn operations_pipeline_transactions_events_and_agent() {
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

    let (status, ops) = json(
        clone_router(&router),
        get("/api/v1/operations", Some(token_a)),
    )
    .await;
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
    let (status, notes_after_fail) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/op-notes?booking_id={booking_id}"),
            Some(token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes_after_fail}");
    assert_eq!(
        notes_after_fail["items"].as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "created note must roll back: {notes_after_fail}"
    );

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
    assert_eq!(confirmed["_operation"]["status"], "completed");
    assert_eq!(confirmed["_operation"]["operation"], "confirm");
    let op_id = confirmed["_operation"]["id"].as_str().cloned();
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
    if let Some(op_id) = op_id {
        assert!(
            events["items"].as_array().unwrap().iter().any(|e| {
                e["name"] == "booking.confirmed"
                    && e["payload"]["operation_id"].as_str() == Some(op_id.as_str())
            }),
            "events should correlate operation_id: {events}"
        );
    }

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

#[tokio::test]
async fn lifecycle_workflow_rbac_isolation_audit_and_concurrency() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register_admin(&router, suffix, "life-a").await;
    let other = register_admin(&router, suffix, "life-b").await;

    let (status, manager_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Mgr",
                "email": format!("life-mgr-{suffix}@example.com"),
                "password": "password123",
                "roles": ["Manager"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{manager_user}");
    let (status, staff_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("life-staff-{suffix}@example.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{staff_user}");
    let (status, mgr_login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("life-mgr-{suffix}@example.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mgr_login}");
    let manager = mgr_login["access_token"].as_str().unwrap();
    let (status, staff_login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("life-staff-{suffix}@example.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_login}");
    let staff = staff_login["access_token"].as_str().unwrap();

    let (_s, room) = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(&admin),
            json!({ "code": format!("L-{suffix}") }),
        ),
    )
    .await;
    let room_id = room["id"].as_str().unwrap();
    let (_s, booking) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    let booking_id = booking["id"].as_str().unwrap();

    let (status, invalid) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/transition"),
            Some(&admin),
            json!({ "transition": "complete" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{invalid}");
    assert_eq!(invalid["error"], "invalid_transition");

    let (status, staff_ops) = json(
        clone_router(&router),
        get("/api/v1/operations", Some(staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_ops}");
    let staff_op_names: Vec<&str> = staff_ops["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["entity"] == "OpBooking")
        .filter_map(|o| o["name"].as_str())
        .collect();
    assert!(staff_op_names.contains(&"confirm"));
    assert!(!staff_op_names.contains(&"explode"));
    let (_status, mgr_ops) = json(
        clone_router(&router),
        get("/api/v1/operations", Some(manager)),
    )
    .await;
    let mgr_op_names: Vec<&str> = mgr_ops["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["entity"] == "OpBooking")
        .filter_map(|o| o["name"].as_str())
        .collect();
    assert!(mgr_op_names.contains(&"explode"), "{mgr_ops}");

    let (_status, staff_tools) =
        json(clone_router(&router), get("/api/v1/tools", Some(staff))).await;
    let staff_tool_names: Vec<&str> = staff_tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(staff_tool_names.contains(&"confirm_op_booking"));
    assert!(!staff_tool_names.contains(&"explode_op_booking"));

    let (status, denied) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/explode_op_booking/invoke",
            Some(staff),
            json!({ "id": booking_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, tenant_id_action) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/confirm"),
            Some(&admin),
            json!({ "tenant_id": Uuid::new_v4() }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{tenant_id_action}");

    let (_s, booking_ok) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    let ok_id = booking_ok["id"].as_str().unwrap();
    let (status, tenant_id_agent) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/confirm_op_booking/invoke",
            Some(&admin),
            json!({ "id": ok_id, "tenant_id": Uuid::new_v4() }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{tenant_id_agent}");

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

    let (status, seated) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/seat_customer"),
            Some(staff),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{seated}");
    assert_eq!(seated["status"], "Seated");

    let (status, completed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/complete"),
            Some(&admin),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["status"], "Completed");

    let (status, cancel_done) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/cancel"),
            Some(&admin),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{cancel_done}");
    assert_eq!(cancel_done["details"]["code"], "invalid_state");

    let (_s, pending) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    let pending_id = pending["id"].as_str().unwrap();
    let (status, cancelled) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{pending_id}/actions/cancel"),
            Some(&admin),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "Cancelled");

    let (status, leaked_get) = json(
        clone_router(&router),
        get(&format!("/api/v1/op-bookings/{booking_id}"), Some(&other)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked_get}");

    let (status, leaked_patch) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/op-bookings/{booking_id}"),
            Some(&other),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked_patch}");
    let (status, leaked_delete) = json(
        clone_router(&router),
        delete(&format!("/api/v1/op-bookings/{booking_id}"), Some(&other)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked_delete}");
    let (status, leaked_action) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/confirm"),
            Some(&other),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked_action}");
    for tool in [
        "get_op_booking",
        "update_op_booking",
        "delete_op_booking",
        "confirm_op_booking",
        "cancel_op_booking",
        "complete_op_booking",
    ] {
        let (status, leaked_agent) = json(
            clone_router(&router),
            post(
                &format!("/api/v1/agent/tools/{tool}/invoke"),
                Some(&other),
                json!({ "id": booking_id }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{tool} {leaked_agent}");
    }

    let (status, search) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/op-bookings?id={booking_id}"),
            Some(&other),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    assert_eq!(search["items"].as_array().unwrap().len(), 0);

    let (status, filter) = json(
        clone_router(&router),
        get("/api/v1/op-bookings?status=Completed", Some(&other)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{filter}");
    assert_eq!(filter["items"].as_array().unwrap().len(), 0);

    let (status, exploded) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_id }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{exploded}");
    let explode_id = exploded["id"].as_str().unwrap();
    let (_s, boom) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{explode_id}/actions/explode"),
            Some(manager),
            json!({}),
        ),
    )
    .await;
    assert_eq!(boom["error"], "business_rule_failed");
    let (_s, audit) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/audit?entity=OpBooking&entity_id={explode_id}"),
            Some(&admin),
        ),
    )
    .await;
    let actions: Vec<&str> = audit["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["action"].as_str())
        .collect();
    assert!(!actions.iter().any(|a| *a == "explode"), "{audit}");
    assert!(!actions.iter().any(|a| *a == "confirm"), "{audit}");

    let (_s, room2) = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(&admin),
            json!({ "code": format!("C-{suffix}") }),
        ),
    )
    .await;
    let room2_id = room2["id"].as_str().unwrap();
    let (_s, b1) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room2_id }),
        ),
    )
    .await;
    let (_s, b2) = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room2_id }),
        ),
    )
    .await;
    let (r1, r2) = tokio::join!(
        json(
            clone_router(&router),
            post(
                &format!(
                    "/api/v1/op-bookings/{}/actions/confirm",
                    b1["id"].as_str().unwrap()
                ),
                Some(&admin),
                json!({}),
            ),
        ),
        json(
            clone_router(&router),
            post(
                &format!(
                    "/api/v1/op-bookings/{}/actions/confirm",
                    b2["id"].as_str().unwrap()
                ),
                Some(&admin),
                json!({}),
            ),
        )
    );
    let ok = [r1.0.is_success(), r2.0.is_success()]
        .into_iter()
        .filter(|v| *v)
        .count();
    assert_eq!(ok, 1, "exactly one confirm should win: {:?} {:?}", r1, r2);

    let started = std::time::Instant::now();
    let mut samples = Vec::new();
    for _ in 0..8 {
        let t0 = std::time::Instant::now();
        let (_s, _) = json(
            clone_router(&router),
            get(&format!("/api/v1/op-bookings/{booking_id}"), Some(&admin)),
        )
        .await;
        samples.push(t0.elapsed().as_millis() as u64);
    }
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let p99 = samples[samples.len() - 1];
    eprintln!(
        "latency_smoke get_booking p50={p50}ms p95={p95}ms p99={p99}ms total_ms={}",
        started.elapsed().as_millis()
    );
    assert!(p99 < 5_000, "p99 {p99}ms is an obvious bottleneck");
}

#[tokio::test]
async fn nested_permissions_tenant_concurrency_idempotency_and_cycles() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register_admin(&router, suffix, "txa").await;
    let other = register_admin(&router, suffix, "txb").await;

    let (status, staff_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("tx-staff-{suffix}@example.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{staff_user}");
    let (status, staff_login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("tx-staff-{suffix}@example.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_login}");
    let staff = staff_login["access_token"].as_str().unwrap();

    let room_a = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(&admin),
            json!({ "code": format!("TA-{suffix}") }),
        ),
    )
    .await
    .1;
    let room_b = json(
        clone_router(&router),
        post(
            "/api/v1/op-rooms",
            Some(&other),
            json!({ "code": format!("TB-{suffix}") }),
        ),
    )
    .await
    .1;
    let booking = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_a["id"] }),
        ),
    )
    .await
    .1;
    let booking_id = booking["id"].as_str().unwrap();

    let (status, denied) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/stamp_note"),
            Some(staff),
            json!({ "body": "nope" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, stamped) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/stamp_note"),
            Some(&admin),
            json!({ "body": "hello" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stamped}");
    let notes = json(
        clone_router(&router),
        get("/api/v1/op-notes", Some(&admin)),
    )
    .await
    .1;
    assert!(
        notes["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["body"] == "hello" && n["booking_id"] == booking_id),
        "{notes}"
    );

    let (status, leaked) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/cross_tenant"),
            Some(&admin),
            json!({ "room_id": room_b["id"] }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN,
        "{leaked}"
    );

    let (status, stale) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/touch_room"),
            Some(&admin),
            json!({ "code": "stale", "_expected_updated_at": "2000-01-01T00:00:00Z" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");

    let current_ts = room_a["updated_at"].as_str().unwrap();
    let (status, touched) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/touch_room"),
            Some(&admin),
            json!({ "code": format!("ok-{suffix}"), "_expected_updated_at": current_ts }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{touched}");

    let booking2 = json(
        clone_router(&router),
        post(
            "/api/v1/op-bookings",
            Some(&admin),
            json!({ "room_id": room_a["id"] }),
        ),
    )
    .await
    .1;
    let booking2_id = booking2["id"].as_str().unwrap();
    let key = format!("confirm-{suffix}");
    let (status, first) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/op-bookings/{booking2_id}/actions/confirm"),
            Some(&admin),
            json!({}),
            Some(&key),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let first_op = first["_operation"]["id"].as_str().unwrap().to_string();
    let (status, second) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/op-bookings/{booking2_id}/actions/confirm"),
            Some(&admin),
            json!({}),
            Some(&key),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["_operation"]["id"].as_str().unwrap(), first_op);

    let (status, cycled) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/op-bookings/{booking_id}/actions/cycle_a"),
            Some(&admin),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{cycled}");
    let msg = cycled["message"].as_str().unwrap_or("").to_lowercase();
    assert!(msg.contains("cycle"), "{cycled}");
}
