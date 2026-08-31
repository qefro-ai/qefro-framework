//! Generic double-entry accounting: REST, workflow, reports, tenant isolation.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, OperationCtx, OperationHandler, QefroRuntime};
use qefro_core::{
    AppModule, EntityDef, FieldDef, LedgerPosting, OperationDef, ACCOUNT_KEY_CASH,
    ACCOUNT_KEY_SALES, UI_SCHEMA_VERSION,
};
use qefro_permissions::{PermissionGrant, ROLE_STAFF};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect(
        "DATABASE_URL is required for integration tests. Run scripts/setup-postgres.sh, then export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro",
    )
}

struct PostSale;

#[async_trait]
impl OperationHandler for PostSale {
    async fn handle(&self, ctx: &mut OperationCtx<'_, '_>) -> qefro_core::QefroResult<Value> {
        let amount = ctx.record.get("amount").cloned().unwrap_or(json!(0));
        let number = ctx
            .record
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("sale")
            .to_string();
        let posted = qefro_api::post_ledger(
            ctx,
            LedgerPosting::new(format!("Sale {number}"), number)
                .debit(ACCOUNT_KEY_CASH, amount.clone())
                .credit(ACCOUNT_KEY_SALES, amount),
        )
        .await?;
        if let Some(journal) = posted {
            if let Some(id) = journal.get("id").cloned() {
                ctx.set_field("journal_id", id);
            }
        }
        Ok(ctx.record.clone())
    }
}

fn app() -> InstalledApp {
    InstalledApp::new(
        AppModule::new("acct_runtime")
            .entity(
                EntityDef::new("AcctSale")
                    .table_name("acct_sales")
                    .slug_name("acct-sales")
                    .label("Sale")
                    .field(FieldDef::string("name").required())
                    .field(FieldDef::currency("amount").required())
                    .field(FieldDef::string("journal_id").nullable())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "AcctSale"))
    .operation(
        OperationDef::new("post_ledger", "AcctSale")
            .label("Post ledger")
            .roles(&["Staff", "Manager"])
            .idempotent(),
        PostSale,
    )
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "accounting-runtime-test-secret".into(),
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

async fn register(router: &axum::Router, email: &str, slug: &str) -> String {
    let (status, body) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Admin",
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

async fn staff_token(
    router: &axum::Router,
    admin: &str,
    suffix: &str,
    tenant_slug: &str,
) -> String {
    let email = format!("staff-{suffix}@ex.com");
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

async fn create_account(
    router: &axum::Router,
    token: &str,
    code: &str,
    name: &str,
    account_type: &str,
    enabled: bool,
    parent_id: Option<&str>,
) -> Value {
    let mut body = json!({
        "code": code,
        "name": name,
        "account_type": account_type,
        "enabled": enabled,
    });
    if let Some(parent) = parent_id {
        body["parent_id"] = json!(parent);
    }
    let (status, created) = json(
        clone_router(router),
        post("/api/v1/accounts", Some(token), body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created
}

#[tokio::test]
async fn accounting_entities_on_generic_ui() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("acct-ui-{suffix}@ex.com"),
        &format!("acct-ui-{suffix}"),
    )
    .await;
    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&token))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], UI_SCHEMA_VERSION);
    let entities = ui["entities"].as_array().cloned().unwrap_or_default();
    let account = entities
        .iter()
        .find(|e| e["entity"] == "Account")
        .expect("Account in meta/ui");
    assert_eq!(account["slug"], "accounts");
    let journal = entities
        .iter()
        .find(|e| e["entity"] == "JournalEntry")
        .expect("JournalEntry in meta/ui");
    assert_eq!(journal["slug"], "journal-entries");
    assert!(entities.iter().any(|e| e["entity"] == "FiscalPeriod"));
    let nav = ui["navigation"].as_array().cloned().unwrap_or_default();
    assert!(
        nav.iter()
            .any(|n| n.as_str() == Some("accounts") || n["slug"] == "accounts"),
        "{nav:?}"
    );
    let (status, workspace) = json(
        clone_router(&router),
        get("/api/v1/meta/workspace", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let ws_nav = workspace["navigation"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        ws_nav
            .iter()
            .any(|n| n["entity"] == "Account" && n["section"] == "Finance"),
        "{ws_nav:?}"
    );
    let (status, reports) = json(
        clone_router(&router),
        get("/api/v1/meta/reports", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reports}");
    let names: Vec<_> = reports["reports"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|r| r["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(names.iter().any(|n| n == "trial-balance"), "{reports}");
    assert!(names.iter().any(|n| n == "general-ledger"), "{reports}");
}

#[tokio::test]
async fn posting_reversal_periods_permissions_and_reports() {
    let _lock = TEST_LOCK.lock().await;
    let (router, state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug = format!("acct-{suffix}");
    let admin = register(&router, &format!("acct-{suffix}@ex.com"), &slug).await;
    let staff = staff_token(&router, &admin, suffix, &slug).await;

    let assets = create_account(&router, &admin, "1000", "Assets", "Asset", true, None).await;
    let cash = create_account(
        &router,
        &admin,
        "1100",
        "Cash",
        "Asset",
        true,
        assets["id"].as_str(),
    )
    .await;
    let sales = create_account(
        &router,
        &admin,
        "4100",
        "Sales Revenue",
        "Revenue",
        true,
        None,
    )
    .await;
    let disabled = create_account(
        &router,
        &admin,
        "1999",
        "Disabled Asset",
        "Asset",
        false,
        None,
    )
    .await;

    let (status, posted_create) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin),
            json!({
                "description": "Cannot create posted",
                "posting_date": "2026-08-30",
                "status": "Posted",
                "lines": [
                    { "account_id": cash["id"], "debit": "10.00", "credit": "0" },
                    { "account_id": sales["id"], "debit": "0", "credit": "10.00" }
                ]
            }),
        ),
    )
    .await;
    assert!(status.is_client_error(), "{posted_create}");

    let (status, unbalanced) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin),
            json!({
                "description": "Unbalanced draft",
                "posting_date": "2026-08-30",
                "lines": [
                    { "account_id": cash["id"], "debit": "100.00", "credit": "0" },
                    { "account_id": sales["id"], "debit": "0", "credit": "90.00" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{unbalanced}");
    let unbalanced_id = unbalanced["id"].as_str().unwrap();
    let (status, rejected) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{unbalanced_id}/actions/post"),
            Some(&admin),
            json!({}),
            Some("unbalanced-post"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected}");
    assert_eq!(rejected["fields"][0]["code"], "unbalanced");

    let (status, journal) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin),
            json!({
                "description": "Restaurant sale",
                "posting_date": "2026-08-30",
                "reference": "ORD-1004",
                "currency": "INR",
                "lines": [
                    { "account_id": cash["id"], "description": "Cash", "debit": "100.00", "credit": "0" },
                    { "account_id": sales["id"], "description": "Sales", "debit": "0", "credit": "100.00" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{journal}");
    assert_eq!(journal["status"], "Draft");
    assert!(journal["doc_no"].as_str().unwrap_or("").starts_with("JE-"));
    let journal_id = journal["id"].as_str().unwrap().to_string();

    let (status, posted) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{journal_id}/actions/post"),
            Some(&staff),
            json!({}),
            Some(&format!("post-{journal_id}")),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{posted}");
    assert_eq!(posted["status"], "Posted");

    let (status, locked) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/journal-entries/{journal_id}"),
            Some(&admin),
            json!({ "description": "tamper" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::CONFLICT,
        "{locked}"
    );

    let (status, status_patch) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/journal-entries/{journal_id}"),
            Some(&admin),
            json!({ "status": "Draft" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{status_patch}"
    );

    let (status, disabled_je) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin),
            json!({
                "description": "Disabled account",
                "posting_date": "2026-08-30",
                "lines": [
                    { "account_id": disabled["id"], "debit": "5.00", "credit": "0" },
                    { "account_id": sales["id"], "debit": "0", "credit": "5.00" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{disabled_je}");
    let disabled_id = disabled_je["id"].as_str().unwrap();
    let (status, disabled_post) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{disabled_id}/actions/post"),
            Some(&admin),
            json!({}),
            Some("disabled-acct"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{disabled_post}");
    assert_eq!(disabled_post["fields"][0]["code"], "disabled");

    let (status, staff_reverse) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{journal_id}/actions/reverse"),
            Some(&staff),
            json!({}),
            Some("staff-reverse"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_reverse}");

    let (status, reversed) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{journal_id}/actions/reverse"),
            Some(&admin),
            json!({}),
            Some(&format!("reverse-{journal_id}")),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reversed}");
    assert_eq!(reversed["status"], "Reversed");

    let list = json(
        clone_router(&router),
        get("/api/v1/journal-entries?page_size=50", Some(&admin)),
    )
    .await
    .1;
    let items = list["items"].as_array().cloned().unwrap_or_default();
    let reversal = items
        .iter()
        .find(|j| j["reversed_from_id"] == journal_id && j["status"] == "Posted")
        .expect("reversal posted");
    assert_eq!(reversal["reference"], "ORD-1004");

    let (status, period) = json(
        clone_router(&router),
        post(
            "/api/v1/fiscal-periods",
            Some(&admin),
            json!({
                "code": "2026-08",
                "name": "August 2026",
                "start_date": "2026-08-01",
                "end_date": "2026-08-31"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{period}");
    let period_id = period["id"].as_str().unwrap();
    let (status, closed) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/fiscal-periods/{period_id}/actions/close"),
            Some(&admin),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    assert_eq!(closed["status"], "Closed");

    let (status, staff_reopen) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/fiscal-periods/{period_id}/actions/reopen"),
            Some(&staff),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_reopen}");

    let (status, late) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin),
            json!({
                "description": "Into closed period",
                "posting_date": "2026-08-15",
                "lines": [
                    { "account_id": cash["id"], "debit": "1.00", "credit": "0" },
                    { "account_id": sales["id"], "debit": "0", "credit": "1.00" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{late}");
    let late_id = late["id"].as_str().unwrap();
    let (status, period_err) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{late_id}/actions/post"),
            Some(&admin),
            json!({}),
            Some("closed-period"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{period_err}");
    assert_eq!(period_err["details"]["code"], "period_closed");
    assert!(
        period_err["message"]
            .as_str()
            .unwrap_or("")
            .contains("August 2026"),
        "{period_err}"
    );

    let (status, tb) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/trial-balance/run",
            Some(&admin),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tb}");
    let rows = tb["rows"].as_array().cloned().unwrap_or_default();
    let mut debit = 0.0;
    let mut credit = 0.0;
    for row in &rows {
        debit += row["debit"].as_f64().unwrap_or(0.0);
        credit += row["credit"].as_f64().unwrap_or(0.0);
    }
    assert!(
        (debit - credit).abs() < 0.000001,
        "tb debit {debit} credit {credit} {tb}"
    );

    let (status, gl) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/general-ledger/run",
            Some(&admin),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{gl}");
    assert!(!gl["rows"].as_array().unwrap().is_empty(), "{gl}");

    let (status, bal) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/account-balance/run",
            Some(&admin),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bal}");

    let (status, activity) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/journal-entries/{journal_id}/activity"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        acts.iter()
            .any(|a| a["activity_type"] == "workflow_transition"
                || a["message"].as_str().unwrap_or("").contains("Posted")),
        "{activity}"
    );

    let (status, search) = json(
        clone_router(&router),
        get("/api/v1/search?q=Cash", Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let hits = search["groups"]
        .as_array()
        .cloned()
        .or_else(|| search["items"].as_array().cloned())
        .unwrap_or_default();
    let blob = search.to_string();
    assert!(blob.contains("Cash") || !hits.is_empty(), "{search}");

    let _ = state.entities.dispatch_outbox().await;
}

#[tokio::test]
async fn tenant_isolation_idempotency_and_post_ledger() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug_a = format!("acct-a-{suffix}");
    let slug_b = format!("acct-b-{suffix}");
    let admin_a = register(&router, &format!("acct-a-{suffix}@ex.com"), &slug_a).await;
    let admin_b = register(&router, &format!("acct-b-{suffix}@ex.com"), &slug_b).await;

    let cash_a = create_account(&router, &admin_a, "1100", "Cash", "Asset", true, None).await;
    let sales_a = create_account(
        &router,
        &admin_a,
        "4100",
        "Sales Revenue",
        "Revenue",
        true,
        None,
    )
    .await;
    let cash_b = create_account(&router, &admin_b, "1100", "Cash B", "Asset", true, None).await;
    assert_ne!(cash_a["id"], cash_b["id"]);

    let (status, leaked) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/accounts/{}", cash_a["id"].as_str().unwrap()),
            Some(&admin_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked}");

    let (status, journal) = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin_a),
            json!({
                "description": "Idempotent post",
                "posting_date": "2026-01-15",
                "lines": [
                    { "account_id": cash_a["id"], "debit": "40.00", "credit": "0" },
                    { "account_id": sales_a["id"], "debit": "0", "credit": "40.00" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{journal}");
    let journal_id = journal["id"].as_str().unwrap().to_string();
    let key = format!("invoice-{suffix}-post");
    let (status, first) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{journal_id}/actions/post"),
            Some(&admin_a),
            json!({}),
            Some(&key),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, second) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/journal-entries/{journal_id}/actions/post"),
            Some(&admin_a),
            json!({}),
            Some(&key),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(first["id"], second["id"]);
    assert_eq!(second["status"], "Posted");

    let other = json(
        clone_router(&router),
        post(
            "/api/v1/journal-entries",
            Some(&admin_a),
            json!({
                "description": "Concurrent",
                "posting_date": "2026-01-16",
                "lines": [
                    { "account_id": cash_a["id"], "debit": "7.00", "credit": "0" },
                    { "account_id": sales_a["id"], "debit": "0", "credit": "7.00" }
                ]
            }),
        ),
    )
    .await
    .1;
    let other_id = other["id"].as_str().unwrap().to_string();
    let path = format!("/api/v1/journal-entries/{other_id}/actions/post");
    let (a, b) = tokio::join!(
        json(
            clone_router(&router),
            post_with(&path, Some(&admin_a), json!({}), Some("conc-a")),
        ),
        json(
            clone_router(&router),
            post_with(&path, Some(&admin_a), json!({}), Some("conc-b")),
        )
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    let failed = [a.0, b.0]
        .iter()
        .filter(|s| s.is_client_error() || **s == StatusCode::CONFLICT)
        .count();
    assert_eq!(ok, 1, "concurrent post {:?} {:?}", a, b);
    assert_eq!(failed, 1, "concurrent post {:?} {:?}", a, b);

    let (status, sale) = json(
        clone_router(&router),
        post(
            "/api/v1/acct-sales",
            Some(&admin_a),
            json!({ "name": "walk-in", "amount": "25.00" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{sale}");
    let sale_id = sale["id"].as_str().unwrap();
    let (status, skipped) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/acct-sales/{sale_id}/actions/post_ledger"),
            Some(&admin_a),
            json!({}),
            Some("sale-skip"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{skipped}");
    assert!(skipped["journal_id"].is_null() || skipped.get("journal_id").is_none());

    json(
        clone_router(&router),
        patch(
            "/api/v1/tenants/me/config",
            Some(&admin_a),
            json!({
                "business": { "currency": "INR", "cash_account": "1100", "sales_account": "4100" }
            }),
        ),
    )
    .await;

    let (status, sale2) = json(
        clone_router(&router),
        post(
            "/api/v1/acct-sales",
            Some(&admin_a),
            json!({ "name": "mapped", "amount": "25.00" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{sale2}");
    let sale2_id = sale2["id"].as_str().unwrap();
    let (status, posted_sale) = json(
        clone_router(&router),
        post_with(
            &format!("/api/v1/acct-sales/{sale2_id}/actions/post_ledger"),
            Some(&admin_a),
            json!({}),
            Some("sale-post"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{posted_sale}");
    let jid = posted_sale["journal_id"].as_str();
    assert!(jid.is_some(), "{posted_sale}");
    let (status, je) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/journal-entries/{}", jid.unwrap()),
            Some(&admin_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{je}");
    assert_eq!(je["status"], "Posted");

    let (status, search_b) = json(
        clone_router(&router),
        get("/api/v1/search?q=Cash", Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_b}");
    let blob = search_b.to_string();
    assert!(!blob.contains(cash_a["id"].as_str().unwrap()), "{search_b}");
}
