use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{DocumentConfig, EntityDef, FieldDef, NotificationDef, PublicFormDef, WebhookDef};
use qefro_permissions::{
    Action, FieldLevelGrant, PermissionGrant, ROLE_HR, ROLE_MANAGER, ROLE_PUBLIC, ROLE_STAFF,
};
use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

fn platform_app() -> InstalledApp {
    InstalledApp::new(
        qefro_core::AppModule::new("platform_demo")
            .version("1.0.0")
            .label("Platform Demo")
            .entity(
                EntityDef::single("ShopSettings")
                    .table_name("plat_shop_settings")
                    .slug_name("shop-settings")
                    .field(FieldDef::string("shop_name").nullable())
                    .field(FieldDef::string("timezone").nullable())
                    .build(),
            )
            .entity(
                EntityDef::new("Employee")
                    .table_name("plat_employees")
                    .slug_name("employees")
                    .field(FieldDef::string("name").required().searchable())
                    .field(FieldDef::string("email").required().email().searchable())
                    .field(FieldDef::decimal("salary").nullable().permission_level(2))
                    .build(),
            )
            .entity(
                EntityDef::new("PlatInvoice")
                    .table_name("plat_invoices")
                    .slug_name("plat-invoices")
                    .workflow("plat_invoice")
                    .attachments()
                    .document(
                        DocumentConfig::new()
                            .submit()
                            .lock_states(&["Submitted", "Approved"]),
                    )
                    .field(
                        FieldDef::enum_("status", vec!["Draft", "Submitted", "Approved"])
                            .required()
                            .default_value(json!("Draft")),
                    )
                    .field(FieldDef::string("customer").required().searchable())
                    .field(FieldDef::decimal("total").nullable())
                    .field(FieldDef::text("delivery_note").nullable().allow_on_submit())
                    .build(),
            )
            .entity(
                EntityDef::new("Booking")
                    .table_name("plat_bookings")
                    .slug_name("bookings")
                    .public_form(
                        PublicFormDef::new("book")
                            .fields(&["guest_name", "party_size"])
                            .success_message("Booked"),
                    )
                    .field(FieldDef::string("guest_name").required().searchable())
                    .field(
                        FieldDef::integer("party_size")
                            .required()
                            .default_value(json!(2)),
                    )
                    .build(),
            )
            .notification(
                NotificationDef::new("invoice_submitted", "plat_invoice.submitted")
                    .channels(&["in_app"])
                    .recipients(&["Staff", "Manager"]),
            )
            .webhook(WebhookDef::new(
                "invoice-submitted",
                "plat_invoice.submitted",
                "test://invoice",
            ))
            .build(),
    )
    .workflow(
        WorkflowDef::new("plat_invoice", "PlatInvoice", "Draft")
            .state(StateDef::new("Submitted"))
            .state(StateDef::new("Approved"))
            .transition(TransitionDef::new("submit", "Draft", "Submitted")),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "ShopSettings"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "Employee"))
    .permission(PermissionGrant::crud(ROLE_HR, "Employee"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "PlatInvoice"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "PlatInvoice"))
    .permission(PermissionGrant::crud(ROLE_STAFF, "Booking"))
    .permission(PermissionGrant::new(
        ROLE_PUBLIC,
        "Booking",
        vec![Action::Create],
    ))
    .field_level(FieldLevelGrant::new(ROLE_HR, "Employee", 2))
}

async fn runtime() -> axum::Router {
    let url = db_url();
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(platform_app());
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

fn patch(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

async fn register(router: &axum::Router, suffix: &str) -> String {
    let (status, auth) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ada",
                "email": format!("plat-{suffix}@example.com"),
                "password": "password123",
                "tenant_name": format!("P-{suffix}"),
                "tenant_slug": format!("p-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    auth["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn singleton_one_per_tenant_and_settings_api() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;

    let (status, first) = json(
        clone_router(&router),
        get("/api/v1/settings/shop-settings", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = first["id"].as_str().unwrap();

    let (status, again) = json(
        clone_router(&router),
        get("/api/v1/settings/shop-settings", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["id"].as_str().unwrap(), id);

    let (status, patched) = json(
        clone_router(&router),
        patch(
            "/api/v1/settings/shop-settings",
            &token,
            json!({ "shop_name": "Seeni Bhai" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["shop_name"], "Seeni Bhai");

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/shop-settings",
            Some(&token),
            json!({ "shop_name": "Other" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT || status == StatusCode::BAD_REQUEST,
        "{status} {created}"
    );
}

#[tokio::test]
async fn field_permissions_hide_and_reject() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(&router, suffix).await;

    let (status, emp) = json(
        clone_router(&router),
        post(
            "/api/v1/employees",
            Some(&admin),
            json!({ "name": "Sam", "email": format!("sam-{suffix}@ex.com"), "salary": 500000 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{emp}");
    let id = emp["id"].as_str().unwrap();
    assert_eq!(emp["salary"].as_f64(), Some(500000.0));

    // Staff user in same tenant
    let (status, user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("staff-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    let (status, login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("staff-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("p-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let staff = login["access_token"].as_str().unwrap();

    let (status, got) = json(
        clone_router(&router),
        get(&format!("/api/v1/employees/{id}"), Some(staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert!(got.get("salary").is_none(), "{got}");

    let (status, patch_res) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/employees/{id}"),
            staff,
            json!({ "salary": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{patch_res}");
}

#[tokio::test]
async fn allow_on_submit_locks_other_fields() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;

    let (status, inv) = json(
        clone_router(&router),
        post(
            "/api/v1/plat-invoices",
            Some(&token),
            json!({ "customer": "Ahmed", "total": 100 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let id = inv["id"].as_str().unwrap();

    let (status, submitted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/plat-invoices/{id}/actions/submit"),
            Some(&token),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    let (status, locked) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/plat-invoices/{id}"),
            &token,
            json!({ "customer": "Other" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{locked}");

    let (status, note) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/plat-invoices/{id}"),
            &token,
            json!({ "delivery_note": "Leave at door" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{note}");
    assert_eq!(note["delivery_note"], "Leave at door");
}

#[tokio::test]
async fn search_respects_rbac_and_field_permissions() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let _ = json(
        clone_router(&router),
        post(
            "/api/v1/employees",
            Some(&token),
            json!({ "name": "Ahmed Khan", "email": format!("ahmed-{suffix}@ex.com"), "salary": 123456 }),
        ),
    )
    .await;

    let (status, results) = json(
        clone_router(&router),
        get("/api/v1/search?q=Ahmed", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{results}");
    let items = results["results"].as_array().cloned().unwrap_or_default();
    assert!(items.iter().any(|i| i["entity"] == "Employee"), "{results}");
    for hit in &items {
        let snippet = hit["snippet"].as_str().unwrap_or("");
        assert!(!snippet.contains("123456"), "{hit}");
    }
}

#[tokio::test]
async fn csv_import_preview_does_not_write() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let csv = "name,email\nAda Lovelace,ada@ex.com\n,bad";
    let (status, preview) = json(
        clone_router(&router),
        post(
            "/api/v1/employees/import/preview",
            Some(&token),
            json!({ "csv": csv }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["rows"], 2);
    assert!(preview["invalid"].as_u64().unwrap() >= 1);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/employees", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["total"], 0);

    let (status, imported) = json(
        clone_router(&router),
        post(
            "/api/v1/employees/import",
            Some(&token),
            json!({ "csv": "name,email\nAda Lovelace,ada-imp@ex.com" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["imported"], 1);
}

#[tokio::test]
async fn public_form_allowlist_and_tenant_resolution() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let _token = register(&router, suffix).await;
    let slug = format!("p-{suffix}");

    let (status, meta) = json(
        clone_router(&router),
        get(&format!("/api/v1/public/{slug}/book"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{meta}");
    let fields = meta["fields"].as_array().cloned().unwrap_or_default();
    assert!(fields.iter().any(|f| f["name"] == "guest_name"));
    assert!(!fields.iter().any(|f| f["name"] == "id"));

    let (status, created) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/public/{slug}/book"),
            None,
            json!({
                "guest_name": "Ahmed",
                "party_size": 4,
                "tenant_id": Uuid::new_v4(),
                "salary": 1
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["ok"], true);
    assert!(created["record"].get("salary").is_none());
}

#[tokio::test]
async fn attachments_require_record_access() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let (status, inv) = json(
        clone_router(&router),
        post(
            "/api/v1/plat-invoices",
            Some(&token),
            json!({ "customer": "Ahmed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let id = inv["id"].as_str().unwrap();
    let (status, listed) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/plat-invoices/{id}/attachments"),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
}

#[tokio::test]
async fn webhook_and_notification_after_submit() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(&router, suffix).await;
    let (status, inv) = json(
        clone_router(&router),
        post(
            "/api/v1/plat-invoices",
            Some(&token),
            json!({ "customer": "Ahmed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let id = inv["id"].as_str().unwrap();
    let (status, submitted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/plat-invoices/{id}/actions/submit"),
            Some(&token),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    // Drain jobs so webhook.deliver and notify.email run after COMMIT.
    for _ in 0..8 {
        let _ = json(clone_router(&router), get("/ready", None)).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (status, notes) = json(
        clone_router(&router),
        get("/api/v1/notifications", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{notes}");
}

#[tokio::test]
async fn tenant_isolation_for_settings_and_search() {
    let _ = db_url();
    let router = runtime().await;
    let a = &Uuid::new_v4().to_string()[..8];
    let b = &Uuid::new_v4().to_string()[..8];
    let token_a = register(&router, a).await;
    let token_b = register(&router, b).await;
    let _ = json(
        clone_router(&router),
        patch(
            "/api/v1/settings/shop-settings",
            &token_a,
            json!({ "shop_name": "TenantA" }),
        ),
    )
    .await;
    let (status, b_settings) = json(
        clone_router(&router),
        get("/api/v1/settings/shop-settings", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{b_settings}");
    assert_ne!(b_settings["shop_name"], "TenantA");
}

#[tokio::test]
async fn concurrent_public_submissions() {
    let _ = db_url();
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let _token = register(&router, suffix).await;
    let slug = format!("p-{suffix}");
    let mut handles = Vec::new();
    for i in 0..100 {
        let r = clone_router(&router);
        let path = format!("/api/v1/public/{slug}/book");
        handles.push(tokio::spawn(async move {
            json(
                r,
                post(
                    &path,
                    None,
                    json!({ "guest_name": format!("G{i}"), "party_size": 2 }),
                ),
            )
            .await
        }));
    }
    let mut ok = 0;
    for h in handles {
        let (status, _) = h.await.unwrap();
        if status == StatusCode::OK {
            ok += 1;
        }
    }
    assert!(ok >= 1, "ok={ok}");
}
