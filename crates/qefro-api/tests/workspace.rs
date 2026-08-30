use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{DashboardCard, DashboardDef, EntityDef, FieldDef, ReportDef};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

fn workspace_app() -> InstalledApp {
    InstalledApp::new(
        qefro_core::AppModule::new("workspace_demo")
            .version("1.0.0")
            .label("Workspace Demo")
            .nav(qefro_core::NavItem::new("Deals", "WsDeal").section("Pipeline"))
            .nav(qefro_core::NavItem::new("Ledger", "WsLedger").section("Finance"))
            .entity(
                EntityDef::new("WsDeal")
                    .table_name("ws_deals")
                    .slug_name("ws-deals")
                    .field(FieldDef::string("name").required().search_weight(10))
                    .field(FieldDef::string("code").search_exact())
                    .field(
                        FieldDef::enum_("status", vec!["Lead", "Won", "Lost"])
                            .required()
                            .default_value(json!("Lead"))
                            .filterable(),
                    )
                    .field(
                        FieldDef::decimal("amount")
                            .required()
                            .default_value(json!(0)),
                    )
                    .field(FieldDef::string("secret_note").searchable().secret())
                    .build(),
            )
            .entity(
                EntityDef::new("WsLedger")
                    .table_name("ws_ledgers")
                    .slug_name("ws-ledgers")
                    .field(FieldDef::string("name").required())
                    .build(),
            )
            .dashboard(
                DashboardDef::new("ws-ops", "Workspace ops")
                    .module("workspace_demo")
                    .card(DashboardCard::kpi("Deals", "WsDeal"))
                    .card(DashboardCard::sum("Pipeline", "WsDeal", "amount").roles(&["Manager"]))
                    .card(DashboardCard::workflow("By status", "WsDeal")),
            )
            .report(
                ReportDef::new("deals-by-status", "WsDeal")
                    .module("workspace_demo")
                    .group_by(&["status"])
                    .sum("amount")
                    .count("id"),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "WsDeal"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "WsDeal"))
    .permission(PermissionGrant::new(
        ROLE_MANAGER,
        "WsDeal",
        vec![Action::Export],
    ))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(workspace_app());
    rt.build().await.expect("build").0
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

fn delete(path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn register(router: &axum::Router, suffix: &str) -> String {
    let (status, auth) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("ws-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("W-{suffix}"),
                "tenant_slug": format!("w-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    auth["access_token"].as_str().unwrap().to_string()
}

async fn staff_token(
    router: &axum::Router,
    admin: &str,
    suffix: &str,
    tenant_slug: &str,
) -> String {
    let email = format!("ws-staff-{suffix}@ex.com");
    let (status, created) = json(
        clone_router(router),
        post(
            "/api/v1/users",
            Some(admin),
            json!({
                "name": "Staff",
                "email": email,
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, login) = json(
        clone_router(router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": email,
                "password": "password123",
                "tenant_slug": tenant_slug
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    login["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn search_is_permission_aware_ranked_and_grouped() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    json(
        clone_router(&router),
        post(
            "/api/v1/ws-deals",
            Some(&token),
            json!({
                "name": "Ahmed Khan",
                "code": "AHMED",
                "status": "Lead",
                "amount": 100,
                "secret_note": "classified"
            }),
        ),
    )
    .await;

    let (status, body) = json(
        clone_router(&router),
        get("/api/v1/search?q=Ahmed", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let results = body["results"].as_array().cloned().unwrap_or_default();
    assert!(results.iter().any(|i| i["entity"] == "WsDeal"), "{body}");
    for hit in &results {
        let blob = hit.to_string();
        assert!(!blob.contains("classified"), "{hit}");
        if hit["entity"] == "WsDeal" {
            assert_eq!(hit["label"], "Ahmed Khan", "{hit}");
        }
    }
    let groups = body["groups"].as_array().cloned().unwrap_or_default();
    assert!(
        groups.iter().any(|g| g["entity"] == "WsDeal"
            && g["hits"].as_array().map(|h| !h.is_empty()).unwrap_or(false)),
        "{body}"
    );
}

#[tokio::test]
async fn search_and_reports_are_tenant_isolated() {
    let _ = db_url();
    let router = runtime().await;
    let a = &Uuid::new_v4().to_string()[..8];
    let b = &Uuid::new_v4().to_string()[..8];
    let token_a = register(&router, a).await;
    let token_b = register(&router, b).await;
    json(
        clone_router(&router),
        post(
            "/api/v1/ws-deals",
            Some(&token_a),
            json!({ "name": format!("secret-{a}"), "status": "Lead", "amount": 9 }),
        ),
    )
    .await;
    let (status, search) = json(
        clone_router(&router),
        get(&format!("/api/v1/search?q=secret-{a}"), Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let results = search["results"].as_array().cloned().unwrap_or_default();
    assert!(results.is_empty(), "{search}");
}

#[tokio::test]
async fn aggregation_grouping_and_report_authorization() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    for (status, amount) in [("Lead", 100), ("Lead", 50), ("Won", 400)] {
        json(
            clone_router(&router),
            post(
                "/api/v1/ws-deals",
                Some(&token),
                json!({ "name": status, "status": status, "amount": amount }),
            ),
        )
        .await;
    }
    let (status, agg) = json(
        clone_router(&router),
        get(
            "/api/v1/ws-deals/aggregates?group_by=status&metric=sum&field=amount",
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{agg}");
    let series = agg["series"].as_array().cloned().unwrap_or_default();
    let lead = series.iter().find(|r| r["label"] == "Lead").unwrap();
    assert_eq!(lead["value"].as_f64(), Some(150.0), "{agg}");
    let won = series.iter().find(|r| r["label"] == "Won").unwrap();
    assert_eq!(won["value"].as_f64(), Some(400.0), "{agg}");

    let (status, report) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/deals-by-status/run",
            Some(&token),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert!(
        report["rows"]
            .as_array()
            .map(|r| !r.is_empty())
            .unwrap_or(false),
        "{report}"
    );

    let (status, sql) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/deals-by-status/run",
            Some(&token),
            json!({ "filters": [{ "sql": "1=1" }] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{sql}");
}

#[tokio::test]
async fn dashboard_hides_unauthorized_widgets() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(&router, suffix).await;
    let staff = staff_token(&router, &admin, suffix, &format!("w-{suffix}")).await;
    json(
        clone_router(&router),
        post(
            "/api/v1/ws-deals",
            Some(&admin),
            json!({ "name": "A", "status": "Lead", "amount": 25 }),
        ),
    )
    .await;

    let (status, admin_dash) = json(
        clone_router(&router),
        get("/api/v1/dashboards/ws-ops", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{admin_dash}");
    let admin_titles: Vec<_> = admin_dash["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap_or(""))
        .collect();
    assert!(admin_titles.contains(&"Pipeline"), "{admin_dash}");

    let (status, staff_dash) = json(
        clone_router(&router),
        get("/api/v1/dashboards/ws-ops", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_dash}");
    let staff_titles: Vec<_> = staff_dash["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap_or(""))
        .collect();
    assert!(staff_titles.contains(&"Deals"), "{staff_dash}");
    assert!(!staff_titles.contains(&"Pipeline"), "{staff_dash}");
}

#[tokio::test]
async fn saved_views_are_user_and_permission_scoped() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(&router, suffix).await;
    let staff = staff_token(&router, &admin, suffix, &format!("w-{suffix}")).await;
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/saved-views",
            Some(&admin),
            json!({
                "entity": "WsDeal",
                "name": "My Leads",
                "query": { "status": "Lead", "view": "list" }
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, staff_list) = json(
        clone_router(&router),
        get("/api/v1/saved-views?entity=WsDeal", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_list}");
    let items = staff_list["items"].as_array().cloned().unwrap_or_default();
    assert!(items.is_empty(), "{staff_list}");

    let (status, deleted) = json(
        clone_router(&router),
        delete(&format!("/api/v1/saved-views/{id}"), &staff),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{deleted}");

    let (status, workspace) = json(
        clone_router(&router),
        get("/api/v1/meta/workspace", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    assert_eq!(workspace["default_dashboard"], "ws-ops");
    assert!(
        workspace["navigation"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["label"] == "Deals"),
        "{workspace}"
    );
}

#[tokio::test]
async fn create_applies_server_side_field_defaults() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/ws-deals",
            Some(&token),
            json!({ "name": format!("default-{suffix}") }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["status"], "Lead", "{created}");
    assert_eq!(created["amount"].as_f64(), Some(0.0), "{created}");
}

#[tokio::test]
async fn workspace_navigation_and_shortcuts_respect_list_permission() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(&router, suffix).await;
    let staff = staff_token(&router, &admin, suffix, &format!("w-{suffix}")).await;

    let (status, admin_ws) = json(
        clone_router(&router),
        get("/api/v1/meta/workspace", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{admin_ws}");
    let admin_nav = admin_ws["navigation"].as_array().cloned().unwrap_or_default();
    assert!(
        admin_nav.iter().any(|i| i["label"] == "Deals" && i["section"] == "Pipeline"),
        "{admin_ws}"
    );
    assert!(
        admin_nav.iter().any(|i| i["label"] == "Ledger" && i["section"] == "Finance"),
        "{admin_ws}"
    );
    let admin_shortcuts = admin_ws["shortcuts"].as_array().cloned().unwrap_or_default();
    assert!(
        admin_shortcuts
            .iter()
            .any(|s| s["kind"] == "create" && s["entity"] == "WsDeal"),
        "{admin_ws}"
    );
    assert!(
        admin_shortcuts
            .iter()
            .any(|s| s["kind"] == "create" && s["entity"] == "WsLedger"),
        "{admin_ws}"
    );

    let (status, staff_ws) = json(
        clone_router(&router),
        get("/api/v1/meta/workspace", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_ws}");
    let staff_nav = staff_ws["navigation"].as_array().cloned().unwrap_or_default();
    assert!(staff_nav.iter().any(|i| i["label"] == "Deals"), "{staff_ws}");
    assert!(
        !staff_nav.iter().any(|i| i["label"] == "Ledger"),
        "{staff_ws}"
    );
    let staff_shortcuts = staff_ws["shortcuts"].as_array().cloned().unwrap_or_default();
    assert!(
        staff_shortcuts
            .iter()
            .any(|s| s["kind"] == "create" && s["entity"] == "WsDeal"),
        "{staff_ws}"
    );
    assert!(
        !staff_shortcuts
            .iter()
            .any(|s| s["entity"] == "WsLedger"),
        "{staff_ws}"
    );

    let (status, denied) = json(
        clone_router(&router),
        get("/api/v1/ws-ledgers", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
}
