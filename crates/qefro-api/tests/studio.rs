use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, DashboardDef, EntityDef, FieldDef, ReportDef};
use qefro_permissions::{PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn studio_app() -> InstalledApp {
    InstalledApp::new(
        AppModule::new("studio_demo")
            .version("1.0.0")
            .label("Studio Demo")
            .entity(
                EntityDef::new("StudioGuest")
                    .table_name("studio_guests")
                    .slug_name("studio-guests")
                    .field(FieldDef::string("name").required().searchable())
                    .build(),
            )
            .entity(
                EntityDef::new("StudioBooking")
                    .table_name("studio_bookings")
                    .slug_name("studio-bookings")
                    .workflow("studio_booking")
                    .field(FieldDef::many_to_one("guest_id", "StudioGuest").required())
                    .field(
                        FieldDef::enum_("status", vec!["Pending", "Confirmed", "Cancelled"])
                            .required()
                            .default_value(json!("Pending")),
                    )
                    .field(FieldDef::string("notes").nullable())
                    .build(),
            )
            .entity(
                EntityDef::new("StudioTicket")
                    .table_name("studio_tickets")
                    .slug_name("studio-tickets")
                    .workflow("studio_ticket")
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Confirmed", "Completed", "Cancelled"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .field(FieldDef::integer("quantity").required().default_value(json!(1)))
                    .field(FieldDef::decimal("rate").required().default_value(json!(0)))
                    .field(FieldDef::decimal("amount").computed("quantity * rate"))
                    .build(),
            )
            .report(
                ReportDef::new("studio-sales", "StudioTicket")
                    .module("studio_demo")
                    .fields(&["status", "amount"])
                    .group_by(&["status"])
                    .sum("amount"),
            )
            .dashboard(
                DashboardDef::new("studio-ops", "Studio Ops")
                    .module("studio_demo")
                    .card(qefro_core::DashboardCard::count("Tickets", "StudioTicket")),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "StudioGuest"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "StudioBooking"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "StudioTicket"))
    .workflow(
        WorkflowDef::new("studio_booking", "StudioBooking", "Pending")
            .state(StateDef::new("Confirmed"))
            .state(StateDef::new("Cancelled").terminal())
            .transition(
                TransitionDef::new("confirm", "Pending", "Confirmed").roles(&["Staff", "Manager"]),
            )
            .transition(TransitionDef::new("cancel", "Pending", "Cancelled")),
    )
    .workflow(
        WorkflowDef::new("studio_ticket", "StudioTicket", "Draft")
            .state(StateDef::new("Confirmed"))
            .state(StateDef::new("Completed").terminal())
            .state(StateDef::new("Cancelled").terminal())
            .transition(
                TransitionDef::new("confirm", "Draft", "Confirmed").roles(&["Staff", "Manager"]),
            )
            .transition(
                TransitionDef::new("complete", "Confirmed", "Completed").roles(&["Manager"]),
            )
            .transition(TransitionDef::new("cancel", "Draft", "Cancelled")),
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

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
}

fn get(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
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

async fn boot() -> Option<(axum::Router, String, String)> {
    let url = db_url()?;
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "studio-test".into(),
        bind: "127.0.0.1:0".into(),
        env: "development".into(),
        ..Config::default()
    });
    rt.install(studio_app());
    let (router, _) = rt.build().await.expect("studio runtime");
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let (status, body) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ada",
                    "email": format!("ada-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": "Studio Co",
                    "tenant_slug": format!("studio-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["access_token"].as_str().unwrap().to_string();
    Some((router, token, suffix))
}

#[tokio::test]
async fn studio_rbac_and_staff_denied() {
    let Some((router, admin, suffix)) = boot().await else {
        return;
    };
    let (status, me) = json(clone_router(&router), get("/api/v1/auth/me", &admin)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert!(me["studio"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c.as_str() == Some("studio.view")));

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            &admin,
            json!({
                "name": "Sam",
                "email": format!("sam-{suffix}@example.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, login) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": format!("sam-{suffix}@example.com"),
                    "password": "password123"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let staff = login["access_token"].as_str().unwrap();
    let (status, denied) = json(clone_router(&router), get("/api/v1/studio/overview", staff)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
}

#[tokio::test]
async fn studio_publish_field_appears_in_generic_ui() {
    let Some((router, token, _)) = boot().await else {
        return;
    };
    let (status, overview) = json(clone_router(&router), get("/api/v1/studio/overview", &token)).await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert!(overview["entities"].as_u64().unwrap() >= 3);

    let (status, preview) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/validate",
            &token,
            json!({
                "kind": "entity.field.upsert",
                "target": "StudioBooking",
                "payload": {
                    "name": "source",
                    "type": "enum",
                    "values": ["Website", "WhatsApp", "Walk-in"],
                    "label": "Source",
                    "nullable": true,
                    "ui": { "widget": "select" }
                }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["impact"], "additive");
    assert_eq!(preview["ok"], true);

    let (status, published) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/publish",
            &token,
            json!({
                "kind": "entity.field.upsert",
                "target": "StudioBooking",
                "confirm_migration": true,
                "summary": "add source field",
                "payload": {
                    "name": "source",
                    "type": "enum",
                    "values": ["Website", "WhatsApp", "Walk-in"],
                    "label": "Source",
                    "nullable": true,
                    "ui": { "widget": "select" }
                }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    assert_eq!(published["published"], true);

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", &token)).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    let booking = ui["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["entity"] == "StudioBooking")
        .unwrap();
    assert!(
        booking["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "source" && f["widget"] == "select"),
        "{booking}"
    );

    let (status, guest) = json(
        clone_router(&router),
        post(
            "/api/v1/studio-guests",
            &token,
            json!({ "name": "Ahmed" }),
        ),
    )
    .await;
    assert!(status.is_success(), "{guest}");
    let (status, booking) = json(
        clone_router(&router),
        post(
            "/api/v1/studio-bookings",
            &token,
            json!({
                "guest_id": guest["id"],
                "source": "WhatsApp"
            }),
        ),
    )
    .await;
    assert!(status.is_success(), "{booking}");
    assert_eq!(booking["source"], "WhatsApp");
}

#[tokio::test]
async fn studio_workflow_publish_exposes_transition() {
    let Some((router, token, _)) = boot().await else {
        return;
    };
    let (status, current) = json(
        clone_router(&router),
        get("/api/v1/studio/workflows/StudioTicket", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{current}");
    let mut wf = current["workflow"].clone();
    let states = wf["states"].as_array_mut().unwrap();
    states.push(json!({ "name": "Approved", "label": "Approved", "terminal": false }));
    let transitions = wf["transitions"].as_array_mut().unwrap();
    transitions.push(json!({
        "name": "approve",
        "from": "Confirmed",
        "to": "Approved",
        "label": "Approve",
        "allowed_roles": ["Manager"]
    }));
    transitions.push(json!({
        "name": "complete_approved",
        "from": "Approved",
        "to": "Completed",
        "label": "Complete",
        "allowed_roles": ["Manager"]
    }));
    let (status, published) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/publish",
            &token,
            json!({
                "kind": "workflow",
                "target": "StudioTicket",
                "payload": wf,
                "summary": "add Approved"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");

    let (status, ticket) = json(
        clone_router(&router),
        post("/api/v1/studio-tickets", &token, json!({ "quantity": 1, "rate": 10 })),
    )
    .await;
    assert!(status.is_success(), "{ticket}");
    let id = ticket["id"].as_str().unwrap();
    let (status, confirmed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/studio-tickets/{id}/transition"),
            &token,
            json!({ "transition": "confirm" }),
        ),
    )
    .await;
    assert!(status.is_success(), "{confirmed}");
    let names: Vec<_> = confirmed["_workflow"]["transitions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"approve"), "{confirmed}");
}

#[tokio::test]
async fn studio_rejects_type_change_and_invalid_formula() {
    let Some((router, token, _)) = boot().await else {
        return;
    };
    let (status, err) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/publish",
            &token,
            json!({
                "kind": "entity.field.upsert",
                "target": "StudioTicket",
                "payload": { "name": "rate", "type": "datetime" }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
    assert!(err["message"].as_str().unwrap().contains("migration"), "{err}");

    let (status, bad) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/validate",
            &token,
            json!({
                "kind": "entity.field.ui",
                "target": "StudioTicket",
                "payload": { "name": "amount", "formula": "DROP TABLE tickets" }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");

    let (status, preview) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/formula/preview",
            &token,
            json!({ "formula": "quantity * rate", "record": { "quantity": 2, "rate": 300 } }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["result"], 600.0);
}

#[tokio::test]
async fn studio_drafts_are_tenant_scoped() {
    let Some((router, token_a, suffix)) = boot().await else {
        return;
    };
    let (status, draft) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/drafts",
            &token_a,
            json!({
                "kind": "entity.field.ui",
                "target": "StudioBooking",
                "payload": { "name": "notes", "label": "Internal notes" },
                "summary": "relabel notes"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{draft}");
    let draft_id = draft["draft"]["id"].as_str().unwrap();

    let (status, b) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Other",
                    "email": format!("other-{suffix}@example.com"),
                    "password": "password123",
                    "tenant_name": "Other Co",
                    "tenant_slug": format!("other-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    let token_b = b["access_token"].as_str().unwrap();
    let (status, missing) = json(
        clone_router(&router),
        get(&format!("/api/v1/studio/drafts/{draft_id}"), token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");

    let (status, list) = json(clone_router(&router), get("/api/v1/studio/drafts", token_b)).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(list["drafts"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn studio_search_and_permissions_publish() {
    let Some((router, token, _)) = boot().await else {
        return;
    };
    let (status, found) = json(
        clone_router(&router),
        get("/api/v1/studio/search?q=StudioTicket", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    let kinds: Vec<_> = found["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"entity"), "{found}");
    assert!(kinds.contains(&"workflow"), "{found}");

    let (status, published) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/publish",
            &token,
            json!({
                "kind": "permissions",
                "target": "StudioTicket",
                "payload": [
                    { "role": "Staff", "entity": "StudioTicket", "actions": ["read", "list"] },
                    { "role": "Manager", "entity": "StudioTicket", "actions": ["create", "read", "update", "delete", "list"] }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    let (status, grants) = json(
        clone_router(&router),
        get("/api/v1/studio/permissions/StudioTicket", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{grants}");
    assert!(grants["grants"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["role"] == "Staff" && g["actions"].as_array().unwrap().iter().all(|a| a != "create")));
}

#[tokio::test]
async fn studio_audit_records_publish() {
    let Some((router, token, _)) = boot().await else {
        return;
    };
    let (status, _) = json(
        clone_router(&router),
        post(
            "/api/v1/studio/publish",
            &token,
            json!({
                "kind": "entity.field.ui",
                "target": "StudioBooking",
                "payload": { "name": "notes", "label": "Booking notes" },
                "summary": "relabel"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, audit) = json(
        clone_router(&router),
        get("/api/v1/audit?entity=studio", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    assert!(
        audit["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["action"].as_str().unwrap().contains("entity.field.ui")),
        "{audit}"
    );
}
