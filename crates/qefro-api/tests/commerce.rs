//! Generic commerce: Quote → Order → Fulfillment → Invoice → Payment → Return.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef, UI_SCHEMA_VERSION};
use qefro_permissions::{PermissionGrant, ROLE_MANAGER, ROLE_STAFF};
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
        AppModule::new("commerce_runtime")
            .entity(
                EntityDef::new("Customer")
                    .table_name("commerce_test_customers")
                    .slug_name("commerce-customers")
                    .label("Customer")
                    .field(
                        FieldDef::string("name")
                            .required()
                            .searchable()
                            .search_weight(10),
                    )
                    .with_commerce()
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "Customer"))
    .permission(PermissionGrant::crud(ROLE_MANAGER, "Customer"))
}

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn runtime() -> (axum::Router, qefro_api::AppState) {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "commerce-runtime-test-secret".into(),
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

async fn user_token(
    router: &axum::Router,
    admin: &str,
    suffix: &str,
    tenant_slug: &str,
    name: &str,
    roles: &[&str],
) -> String {
    let email = format!("{name}-{suffix}@ex.com");
    let (status, created) = json(
        clone_router(router),
        post(
            "/api/v1/users",
            Some(admin),
            json!({
                "name": name,
                "email": email,
                "password": "password123",
                "roles": roles
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

fn money_str(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

fn money_close(v: &Value, expected: &str) -> bool {
    let got = money_str(v);
    got == expected
        || got.starts_with(expected)
        || expected.starts_with(got.trim_end_matches('0').trim_end_matches('.'))
}

async fn create_account(
    router: &axum::Router,
    token: &str,
    code: &str,
    name: &str,
    ty: &str,
) -> Value {
    let (status, created) = json(
        clone_router(router),
        post(
            "/api/v1/accounts",
            Some(token),
            json!({ "code": code, "name": name, "account_type": ty, "enabled": true }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created
}

async fn action(
    router: &axum::Router,
    token: &str,
    path: &str,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let fallback = Uuid::new_v4().to_string();
    let key = key.unwrap_or(&fallback);
    json(
        clone_router(router),
        post_with(path, Some(token), body, Some(key)),
    )
    .await
}

#[tokio::test]
async fn quote_order_fulfill_invoice_payment_and_return() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug = format!("com-{suffix}");
    let admin = register(&router, &format!("com-{suffix}@ex.com"), &slug).await;
    let staff = user_token(&router, &admin, suffix, &slug, "staff", &["Staff"]).await;

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], UI_SCHEMA_VERSION);
    let entities = ui["entities"].as_array().cloned().unwrap_or_default();
    assert!(entities.iter().any(|e| e["entity"] == "Quote"));
    assert!(entities.iter().any(|e| e["entity"] == "SalesOrder"));
    assert!(entities.iter().any(|e| e["entity"] == "Invoice"));
    assert!(entities.iter().any(|e| e["entity"] == "Product"));
    let hidden = ui["hidden_entities"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        hidden.iter().any(|h| h.as_str() == Some("quote-items")),
        "{hidden:?}"
    );
    let nav = ui["navigation"].as_array().cloned().unwrap_or_default();
    assert!(
        nav.iter()
            .any(|n| n.as_str() == Some("sales-orders") || n["slug"] == "sales-orders"),
        "{nav:?}"
    );

    let (status, reports) = json(
        clone_router(&router),
        get("/api/v1/meta/reports", Some(&admin)),
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
    for expected in [
        "sales-by-customer",
        "sales-by-product",
        "orders-by-status",
        "invoices-outstanding",
        "payments-received",
        "returns-by-status",
    ] {
        assert!(names.iter().any(|n| n == expected), "{reports}");
    }

    create_account(&router, &admin, "1100", "Cash", "Asset").await;
    create_account(&router, &admin, "1200", "Receivable", "Asset").await;
    create_account(&router, &admin, "4100", "Sales", "Revenue").await;
    json(
        clone_router(&router),
        patch(
            "/api/v1/tenants/me/config",
            Some(&admin),
            json!({
                "business": {
                    "currency": "USD",
                    "cash_account": "1100",
                    "receivable_account": "1200",
                    "sales_account": "4100"
                }
            }),
        ),
    )
    .await;

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/commerce-customers",
            Some(&staff),
            json!({ "name": "Ahmed Khan" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    let customer_id = customer["id"].as_str().unwrap();

    let (status, product_a) = json(
        clone_router(&router),
        post(
            "/api/v1/products",
            Some(&staff),
            json!({
                "sku": format!("SKU-A-{suffix}"),
                "name": "Widget A",
                "unit_price": "20.00",
                "enabled": true
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{product_a}");
    let (status, product_b) = json(
        clone_router(&router),
        post(
            "/api/v1/products",
            Some(&staff),
            json!({
                "sku": format!("SKU-B-{suffix}"),
                "name": "Widget B",
                "unit_price": "10.00",
                "enabled": true
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{product_b}");

    let (status, quote) = json(
        clone_router(&router),
        post(
            "/api/v1/quotes",
            Some(&staff),
            json!({
                "customer_type": "Customer",
                "customer_id": customer_id,
                "customer_name": "Ahmed Khan",
                "tax_rate": "8.75",
                "discount": "5",
                "items": [
                    {
                        "product_id": product_a["id"],
                        "quantity": 2,
                        "unit_price": "0.01"
                    }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{quote}");
    assert_eq!(quote["status"], "Draft");
    let quote_id = quote["id"].as_str().unwrap().to_string();

    let (status, patched) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/quotes/{quote_id}"),
            Some(&staff),
            json!({ "status": "Accepted" }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{patched}"
    );

    let (status, sent) = action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{quote_id}/actions/send"),
        json!({}),
        Some(&format!("send-{quote_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert_eq!(sent["status"], "Sent");
    assert!(
        sent["doc_no"].as_str().unwrap_or("").starts_with("QT-"),
        "{sent}"
    );
    let items = sent["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items
            .iter()
            .any(|i| money_close(&i["unit_price"], "20") || money_close(&i["unit_price"], "20.00")),
        "{sent}"
    );

    let (status, accepted) = action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{quote_id}/actions/accept"),
        json!({}),
        Some(&format!("accept-{quote_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["status"], "Accepted");

    let (status, converted) = action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{quote_id}/actions/convert"),
        json!({}),
        Some(&format!("convert-{quote_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{converted}");
    assert_eq!(converted["status"], "Converted");
    let (status, replay) = action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{quote_id}/actions/convert"),
        json!({}),
        Some(&format!("convert-{quote_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["status"], "Converted");

    let (status, orders) = json(
        clone_router(&router),
        get("/api/v1/sales-orders", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{orders}");
    let order_rows = orders["items"]
        .as_array()
        .cloned()
        .or_else(|| orders.as_array().cloned())
        .unwrap_or_default();
    let order = order_rows
        .iter()
        .find(|o| o["quote_id"] == quote_id)
        .cloned()
        .or_else(|| {
            converted
                .get("_navigate")
                .and_then(|n| n.get("id"))
                .cloned()
                .map(|id| json!({ "id": id }))
        })
        .expect("order from quote");
    let order_id = order["id"].as_str().unwrap().to_string();
    let (status, order) = json(
        clone_router(&router),
        get(&format!("/api/v1/sales-orders/{order_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order}");
    assert_eq!(order["status"], "Draft");
    assert_eq!(order["customer_name"], "Ahmed Khan");
    assert!(order["doc_no"].as_str().unwrap_or("").starts_with("SO-") || order["doc_no"].is_null());
    let line_prices: Vec<_> = order["items"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|i| money_str(&i["unit_price"]))
        .collect();
    assert!(line_prices.iter().any(|p| p.starts_with("20")), "{order}");

    let actions = order["_actions"].as_array().cloned().unwrap_or_default();
    assert!(actions.iter().any(|a| a["name"] == "confirm"), "{order}");

    let (status, confirmed) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order_id}/actions/confirm"),
        json!({}),
        Some(&format!("confirm-{order_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "Confirmed");

    let (status, staff_cancel) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order_id}/actions/cancel"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_cancel}");

    // Second order with two lines for partial fulfillment.
    let (status, quote2) = json(
        clone_router(&router),
        post(
            "/api/v1/quotes",
            Some(&staff),
            json!({
                "customer_type": "Customer",
                "customer_id": customer_id,
                "customer_name": "Ahmed Khan",
                "items": [
                    { "product_id": product_a["id"], "quantity": 10, "unit_price": "20" },
                    { "product_id": product_b["id"], "quantity": 5, "unit_price": "10" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{quote2}");
    let q2 = quote2["id"].as_str().unwrap();
    action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{q2}/actions/send"),
        json!({}),
        None,
    )
    .await;
    action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{q2}/actions/accept"),
        json!({}),
        None,
    )
    .await;
    let (status, conv2) = action(
        &router,
        &staff,
        &format!("/api/v1/quotes/{q2}/actions/convert"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{conv2}");
    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/sales-orders", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let rows = listed["items"]
        .as_array()
        .cloned()
        .or_else(|| listed.as_array().cloned())
        .unwrap_or_default();
    let order2 = rows
        .iter()
        .find(|o| o["quote_id"] == q2)
        .cloned()
        .expect("second order");
    let order2_id = order2["id"].as_str().unwrap().to_string();
    action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order2_id}/actions/confirm"),
        json!({}),
        None,
    )
    .await;
    let (status, order2) = json(
        clone_router(&router),
        get(&format!("/api/v1/sales-orders/{order2_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order2}");
    let lines = order2["items"].as_array().cloned().unwrap_or_default();
    let line_b = lines
        .iter()
        .find(|l| l["quantity"] == 5 || l["quantity"] == json!(5))
        .cloned()
        .expect("line B");
    let (status, partial) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order2_id}/actions/fulfill"),
        json!({
            "items": [{ "order_item_id": line_b["id"], "quantity": 2 }]
        }),
        Some(&format!("fulfill-partial-{order2_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{partial}");
    let (status, order2) = json(
        clone_router(&router),
        get(&format!("/api/v1/sales-orders/{order2_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order2}");
    assert_eq!(order2["status"], "Confirmed");
    assert_eq!(order2["fulfillment_status"], "Partial");

    let (status, rest) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order2_id}/actions/fulfill"),
        json!({}),
        Some(&format!("fulfill-rest-{order2_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rest}");
    let (status, order2) = json(
        clone_router(&router),
        get(&format!("/api/v1/sales-orders/{order2_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{order2}");
    assert_eq!(order2["status"], "Fulfilled");
    assert_eq!(order2["fulfillment_status"], "Fulfilled");

    action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order2_id}/actions/complete"),
        json!({}),
        None,
    )
    .await;
    let (status, invoiced) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-orders/{order2_id}/actions/issue_invoice"),
        json!({}),
        Some(&format!("inv-{order2_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{invoiced}");

    let (status, invoices) =
        json(clone_router(&router), get("/api/v1/invoices", Some(&staff))).await;
    assert_eq!(status, StatusCode::OK, "{invoices}");
    let inv_rows = invoices["items"]
        .as_array()
        .cloned()
        .or_else(|| invoices.as_array().cloned())
        .unwrap_or_default();
    let invoice = inv_rows
        .iter()
        .find(|i| i["order_id"] == order2_id)
        .cloned()
        .expect("invoice");
    let invoice_id = invoice["id"].as_str().unwrap().to_string();
    let (status, invoice) = json(
        clone_router(&router),
        get(&format!("/api/v1/invoices/{invoice_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{invoice}");
    assert_eq!(invoice["status"], "Issued");
    assert!(
        invoice["doc_no"].as_str().unwrap_or("").starts_with("INV-"),
        "{invoice}"
    );
    assert!(invoice.get("journal_id").is_some(), "{invoice}");

    let (status, paid) = action(
        &router,
        &staff,
        &format!("/api/v1/invoices/{invoice_id}/actions/record_payment"),
        json!({ "method": "Cash" }),
        Some(&format!("pay-{invoice_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{paid}");
    let (status, invoice) = json(
        clone_router(&router),
        get(&format!("/api/v1/invoices/{invoice_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{invoice}");
    assert_eq!(invoice["status"], "Paid");

    let (status, ret) = json(
        clone_router(&router),
        post(
            "/api/v1/sales-returns",
            Some(&staff),
            json!({
                "order_id": order2_id,
                "customer_type": "Customer",
                "customer_id": customer_id,
                "customer_name": "Ahmed Khan",
                "items": [{
                    "order_item_id": line_b["id"],
                    "product_id": product_b["id"],
                    "quantity": 1
                }]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ret}");
    let ret_id = ret["id"].as_str().unwrap();
    let manager = user_token(&router, &admin, suffix, &slug, "mgr", &["Manager"]).await;
    let (status, approved) = action(
        &router,
        &manager,
        &format!("/api/v1/sales-returns/{ret_id}/actions/approve"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    let (status, received) = action(
        &router,
        &staff,
        &format!("/api/v1/sales-returns/{ret_id}/actions/receive"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    let (status, refunded) = action(
        &router,
        &manager,
        &format!("/api/v1/sales-returns/{ret_id}/actions/refund"),
        json!({}),
        Some(&format!("refund-{ret_id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refunded}");
    assert_eq!(refunded["status"], "Refunded");

    let (status, activity) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/sales-orders/{order2_id}/activity"),
            Some(&staff),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let acts = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(acts.iter().any(|a| a["activity_type"] == "created" || a["activity_type"] == "workflow_transition"), "{activity}");

    let (status, audit) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/audit?entity=SalesOrder&entity_id={order2_id}"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{audit}");

    let (status, search) = json(
        clone_router(&router),
        get("/api/v1/search?q=Ahmed%20Khan", Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let blob = search.to_string();
    assert!(
        blob.contains("Ahmed Khan") || blob.contains(customer_id),
        "{search}"
    );

    let (status, related) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/commerce-customers/{customer_id}"),
            Some(&staff),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{related}");
    let links = related["_links"].as_array().cloned().unwrap_or_default();
    assert!(
        links
            .iter()
            .any(|l| l["entity"] == "SalesOrder" || l["label"] == "Sales Orders"),
        "{related}"
    );
}

#[tokio::test]
async fn convert_failure_price_tamper_tenant_and_concurrency() {
    let _lock = TEST_LOCK.lock().await;
    let (router, _state) = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug_a = format!("ca-{suffix}");
    let slug_b = format!("cb-{suffix}");
    let admin_a = register(&router, &format!("ca-{suffix}@ex.com"), &slug_a).await;
    let admin_b = register(&router, &format!("cb-{suffix}@ex.com"), &slug_b).await;
    let staff_a = user_token(&router, &admin_a, suffix, &slug_a, "sa", &["Staff"]).await;

    let (status, product) = json(
        clone_router(&router),
        post(
            "/api/v1/products",
            Some(&staff_a),
            json!({
                "sku": format!("SKU-{suffix}"),
                "name": "Gadget",
                "unit_price": "100.00",
                "enabled": true
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{product}");

    let (status, empty) = json(
        clone_router(&router),
        post(
            "/api/v1/quotes",
            Some(&staff_a),
            json!({ "customer_name": "Empty" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{empty}");
    let empty_id = empty["id"].as_str().unwrap();
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{empty_id}/actions/send"),
        json!({}),
        None,
    )
    .await;
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{empty_id}/actions/accept"),
        json!({}),
        None,
    )
    .await;
    let (status, failed) = action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{empty_id}/actions/convert"),
        json!({}),
        None,
    )
    .await;
    assert!(
        status == StatusCode::UNPROCESSABLE_ENTITY
            || status == StatusCode::CONFLICT
            || status == StatusCode::BAD_REQUEST,
        "{failed}"
    );
    let (status, still) = json(
        clone_router(&router),
        get(&format!("/api/v1/quotes/{empty_id}"), Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{still}");
    assert_eq!(still["status"], "Accepted");

    let (status, p2) = json(
        clone_router(&router),
        post(
            "/api/v1/products",
            Some(&staff_a),
            json!({
                "sku": format!("SKU2-{suffix}"),
                "name": "Gadget 2",
                "unit_price": "50.00",
                "enabled": true
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{p2}");
    let (status, qfail) = json(
        clone_router(&router),
        post(
            "/api/v1/quotes",
            Some(&staff_a),
            json!({
                "customer_name": "Rollback",
                "items": [
                    { "product_id": product["id"], "quantity": 1, "unit_price": "0.01" },
                    { "product_id": p2["id"], "quantity": 1, "unit_price": "0.01" }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{qfail}");
    let qf = qfail["id"].as_str().unwrap();
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{qf}/actions/send"),
        json!({}),
        None,
    )
    .await;
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{qf}/actions/accept"),
        json!({}),
        None,
    )
    .await;
    json(
        clone_router(&router),
        patch(
            &format!("/api/v1/products/{}", p2["id"].as_str().unwrap()),
            Some(&admin_a),
            json!({ "enabled": false }),
        ),
    )
    .await;
    let (status, boom) = action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{qf}/actions/convert"),
        json!({}),
        None,
    )
    .await;
    assert!(
        status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST,
        "{boom}"
    );
    let (status, qafter) = json(
        clone_router(&router),
        get(&format!("/api/v1/quotes/{qf}"), Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{qafter}");
    assert_eq!(qafter["status"], "Accepted");
    let (status, orders) = json(
        clone_router(&router),
        get("/api/v1/sales-orders", Some(&staff_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{orders}");
    let rows = orders["items"]
        .as_array()
        .cloned()
        .or_else(|| orders.as_array().cloned())
        .unwrap_or_default();
    assert!(
        !rows.iter().any(|o| o["quote_id"] == qf),
        "convert must not leave a sales order after failure: {orders}"
    );

    let (status, qt) = json(
        clone_router(&router),
        post(
            "/api/v1/quotes",
            Some(&staff_a),
            json!({
                "customer_name": "Race",
                "items": [{ "product_id": product["id"], "quantity": 1, "unit_price": "100" }]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{qt}");
    let race_id = qt["id"].as_str().unwrap().to_string();
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{race_id}/actions/send"),
        json!({}),
        None,
    )
    .await;
    action(
        &router,
        &staff_a,
        &format!("/api/v1/quotes/{race_id}/actions/accept"),
        json!({}),
        None,
    )
    .await;
    let path = format!("/api/v1/quotes/{race_id}/actions/convert");
    let (a, b) = tokio::join!(
        action(&router, &staff_a, &path, json!({}), Some("conc-a")),
        action(&router, &staff_a, &path, json!({}), Some("conc-b")),
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    let denied = [a.0, b.0]
        .iter()
        .filter(|s| {
            matches!(
                **s,
                StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST
            )
        })
        .count();
    assert_eq!(ok, 1, "one convert should succeed: {a:?} {b:?}");
    assert_eq!(
        denied, 1,
        "the other convert must not duplicate the order: {a:?} {b:?}"
    );

    let (status, leaked) = json(
        clone_router(&router),
        get(&format!("/api/v1/quotes/{race_id}"), Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{leaked}");
    let (status, steal) = action(
        &router,
        &admin_b,
        &format!("/api/v1/quotes/{race_id}/actions/convert"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{steal}");

    let (status, search_b) = json(
        clone_router(&router),
        get("/api/v1/search?q=Race", Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search_b}");
    assert!(!search_b.to_string().contains(&race_id), "{search_b}");
}
