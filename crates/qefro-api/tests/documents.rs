use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    AppModule, ChildTableDef, DocumentConfig, EntityDef, FieldDef, NamingConfig, ReportDef,
};
use qefro_permissions::{Action, PermissionGrant, ROLE_MANAGER};
use qefro_workflow::{TransitionDef, WorkflowDef};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn test_db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn test_app() -> InstalledApp {
    let module = AppModule::new("docs_test")
        .entity(
            EntityDef::new("DocCustomer")
                .table_name("doc_customers")
                .slug_name("doc-customers")
                .field(FieldDef::string("name").required())
                .build(),
        )
        .entity(
            EntityDef::new("Invoice")
                .table_name("doc_invoices")
                .slug_name("doc-invoices")
                .workflow("invoice")
                .document(
                    DocumentConfig::new()
                        .submit()
                        .cancel()
                        .duplicate()
                        .lock_states(&["Submitted", "Cancelled"])
                        .number_on("submit"),
                )
                .naming(
                    NamingConfig::new("INV-{YYYY}-{#####}")
                        .field("doc_no")
                        .assign_on("submit"),
                )
                .field(FieldDef::many_to_one("customer_id", "DocCustomer").required())
                .field(
                    FieldDef::enum_values("status", vec!["Draft", "Submitted", "Cancelled"])
                        .required()
                        .default_value(json!("Draft")),
                )
                .child_table(ChildTableDef::new("items", "InvoiceItem").parent_field("invoice_id"))
                .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
                .field(
                    FieldDef::currency("discount")
                        .nullable()
                        .min(0.0)
                        .default_value(json!(0)),
                )
                .field(FieldDef::currency("grand_total").computed("subtotal - discount"))
                .build(),
        )
        .entity(
            EntityDef::new("InvoiceItem")
                .table_name("doc_invoice_items")
                .slug_name("doc-invoice-items")
                .child_of("Invoice", "items")
                .field(FieldDef::many_to_one("invoice_id", "Invoice").required().hidden())
                .field(FieldDef::string("product").required())
                .field(FieldDef::integer("quantity").required().min(1.0))
                .field(FieldDef::currency("rate").required().min(0.0))
                .field(FieldDef::currency("amount").computed("quantity * rate"))
                .build(),
        )
        .report(
            ReportDef::new("invoice-totals", "Invoice")
                .module("docs_test")
                .fields(&["status", "grand_total"])
                .group_by(&["status"])
                .sum("grand_total"),
        )
        .build();
    InstalledApp::new(module)
        .workflow(
            WorkflowDef::new("invoice", "Invoice", "Draft")
                .transition(TransitionDef::new("submit", "Draft", "Submitted").roles(&["Manager"]))
                .transition(
                    TransitionDef::new("cancel", "Draft", "Cancelled").roles(&["Manager"]),
                )
                .transition(
                    TransitionDef::new("cancel_submitted", "Submitted", "Cancelled")
                        .roles(&["Manager"]),
                ),
        )
        .permission(PermissionGrant::crud(ROLE_MANAGER, "DocCustomer"))
        .permission(PermissionGrant::crud(ROLE_MANAGER, "Invoice"))
        .permission(PermissionGrant::crud(ROLE_MANAGER, "InvoiceItem"))
        .permission(PermissionGrant::new(
            ROLE_MANAGER,
            "Invoice",
            vec![Action::Export],
        ))
}

async fn runtime() -> axum::Router {
    let url = test_db_url().expect("DATABASE_URL");
    let mut rt = QefroRuntime::new(Config {
        database_url: url,
        jwt_secret: "test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(test_app());
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

fn clone_router(router: &axum::Router) -> axum::Router {
    router.clone()
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

#[tokio::test]
async fn child_tables_formulas_documents_reports_and_security() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token_a = register(&router, &format!("da-{suffix}@example.com"), &format!("da-{suffix}")).await;
    let token_b = register(&router, &format!("db-{suffix}@example.com"), &format!("db-{suffix}")).await;

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/doc-customers",
            Some(&token_a),
            json!({ "name": "Ahmed" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    let customer_id = customer["id"].as_str().unwrap();

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/doc-invoices",
            Some(&token_a),
            json!({
                "customer_id": customer_id,
                "discount": 10,
                "items": [
                    { "product": "Pizza", "quantity": 2, "rate": 300, "amount": 999999 },
                    { "product": "Coke", "quantity": 1, "rate": 100 }
                ]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["items"].as_array().unwrap().len(), 2);
    assert_eq!(created["items"][0]["amount"].as_f64().unwrap(), 600.0);
    assert_eq!(created["subtotal"].as_f64().unwrap(), 700.0);
    assert_eq!(created["grand_total"].as_f64().unwrap(), 690.0);
    let invoice_id = created["id"].as_str().unwrap();

    let (status, bad) = json(
        clone_router(&router),
        post(
            "/api/v1/doc-invoices",
            Some(&token_a),
            json!({
                "customer_id": customer_id,
                "items": [{ "product": "Bad", "quantity": 0, "rate": 1 }]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    let fields = bad["fields"].as_array().cloned().unwrap_or_default();
    assert!(
        fields.iter().any(|f| f["field"].as_str() == Some("items.0.quantity"))
            || bad["nested"]["items"]["0"]["quantity"].is_string(),
        "{bad}"
    );

    let (status, listed_b) = json(
        clone_router(&router),
        get("/api/v1/doc-invoices", Some(&token_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed_b}");
    assert_eq!(listed_b["items"].as_array().unwrap().len(), 0);

    let (status, cross) = json(
        clone_router(&router),
        get(&format!("/api/v1/doc-invoices/{invoice_id}"), Some(&token_b)),
    )
    .await;
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN, "{cross}");

    let (status, submitted) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/doc-invoices/{invoice_id}/actions/submit"),
            Some(&token_a),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(submitted["status"], "Submitted");
    let doc_no = submitted["doc_no"].as_str().unwrap();
    assert!(doc_no.starts_with("INV-"), "{doc_no}");

    let (status, locked) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/doc-invoices/{invoice_id}"),
            Some(&token_a),
            json!({ "discount": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{locked}");
    assert_eq!(locked["fields"][0]["code"], "locked");

    let (status, cancel_body) = json(
        clone_router(&router),
        post(
            &format!("/api/v1/doc-invoices/{invoice_id}/actions/cancel"),
            Some(&token_b),
            json!({}),
        ),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::UNAUTHORIZED,
        "{cancel_body}"
    );

    let (status, print) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/doc-invoices/{invoice_id}/print"),
            Some(&token_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{print}");
    let html = print["raw"].as_str().unwrap_or("");
    assert!(html.contains("Invoice") || print.to_string().contains("Invoice"));

    let (status, sql) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/invoice-totals/run",
            Some(&token_a),
            json!({ "filters": [{ "sql": "DROP TABLE doc_invoices" }] }),
        ),
    )
    .await;
    assert!(status.is_client_error(), "{sql}");

    let (status, report) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/invoice-totals/run",
            Some(&token_a),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert!(report["rows"].as_array().unwrap().len() >= 1);

    let (status, other_report) = json(
        clone_router(&router),
        post(
            "/api/v1/reports/invoice-totals/run",
            Some(&token_b),
            json!({ "filters": [] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other_report}");
    let total_b: f64 = other_report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r.get("grand_total").and_then(|v| v.as_f64()))
        .sum();
    assert_eq!(total_b, 0.0);
}

#[tokio::test]
async fn concurrent_numbering_is_unique_and_tenant_scoped() {
    if test_db_url().is_none() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token_a = register(&router, &format!("na-{suffix}@example.com"), &format!("na-{suffix}")).await;
    let token_b = register(&router, &format!("nb-{suffix}@example.com"), &format!("nb-{suffix}")).await;

    async fn customer(router: &axum::Router, token: &str) -> String {
        let (status, body) = json(
            clone_router(router),
            post("/api/v1/doc-customers", Some(token), json!({ "name": "C" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body["id"].as_str().unwrap().to_string()
    }
    let cust_a = customer(&router, &token_a).await;
    let cust_b = customer(&router, &token_b).await;

    async fn make_invoice(router: axum::Router, token: String, customer_id: String) -> String {
        let (status, created) = json(
            router.clone(),
            post(
                "/api/v1/doc-invoices",
                Some(&token),
                json!({
                    "customer_id": customer_id,
                    "items": [{ "product": "X", "quantity": 1, "rate": 10 }]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_str().unwrap().to_string();
        let (status, submitted) = json(
            router,
            post(
                &format!("/api/v1/doc-invoices/{id}/actions/submit"),
                Some(&token),
                json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{submitted}");
        submitted["doc_no"].as_str().unwrap().to_string()
    }

    let mut handles = Vec::new();
    for _ in 0..100 {
        let r = clone_router(&router);
        let t = token_a.clone();
        let c = cust_a.clone();
        handles.push(tokio::spawn(async move { make_invoice(r, t, c).await }));
    }
    let mut numbers = Vec::new();
    for h in handles {
        numbers.push(h.await.unwrap());
    }
    let unique: std::collections::HashSet<_> = numbers.iter().cloned().collect();
    assert_eq!(unique.len(), numbers.len(), "{numbers:?}");

    let b_no = make_invoice(clone_router(&router), token_b.clone(), cust_b).await;
    assert!(!numbers.contains(&b_no) || b_no.ends_with("00001"));
}
