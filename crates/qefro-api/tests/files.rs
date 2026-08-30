//! Generic document/file runtime: entity-scoped attachments, tenant isolation,
//! permissions, validation, activity, and audit.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{AppModule, EntityDef, FieldDef};
use qefro_permissions::{PermissionGrant, ROLE_STAFF};
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
        AppModule::new("file_runtime")
            .entity(
                EntityDef::new("FileHost")
                    .table_name("file_hosts")
                    .slug_name("file-hosts")
                    .label("Host")
                    .attachments()
                    .field(FieldDef::string("name").required().searchable())
                    .build(),
            )
            .entity(
                EntityDef::new("FileBare")
                    .table_name("file_bares")
                    .slug_name("file-bares")
                    .label("Bare")
                    .field(FieldDef::string("name").required())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud("Admin", "FileHost"))
    .permission(PermissionGrant::read(ROLE_STAFF, "FileHost"))
    .permission(PermissionGrant::crud("Admin", "FileBare"))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "file-runtime-test-secret".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    rt.install(app());
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

async fn raw(router: axum::Router, req: Request<Body>) -> (StatusCode, Vec<u8>, String) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    let ctype = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec(), ctype)
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

fn delete(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("DELETE").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn multipart_file(
    path: &str,
    token: &str,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Request<Body> {
    let boundary = "----qefroBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

async fn register(router: &axum::Router, email: &str, slug: &str) -> String {
    let (status, body) = json(
        clone_router(router),
        post(
            "/api/v1/auth/register",
            None,
            json!({
                "name": "Ahmed Khan",
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

const PDF: &[u8] = b"%PDF-1.4 invoice";

async fn host_with_file(router: &axum::Router, token: &str) -> (String, String) {
    let (status, created) = json(
        clone_router(router),
        post(
            "/api/v1/file-hosts",
            Some(token),
            json!({ "name": "Order 1042" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_string();
    let (status, uploaded) = json(
        clone_router(router),
        multipart_file(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            token,
            "Invoice.pdf",
            "application/pdf",
            PDF,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uploaded}");
    let file_id = uploaded["id"].as_str().unwrap().to_string();
    (id, file_id)
}

#[tokio::test]
async fn upload_list_download_delete_and_metadata() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("fa-{suffix}@ex.com"),
        &format!("fa-{suffix}"),
    )
    .await;
    let (id, file_id) = host_with_file(&router, &token).await;

    let (status, listed) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["items"][0]["filename"], "Invoice.pdf");
    assert!(listed["items"][0].get("storage_key").is_none(), "{listed}");
    assert_eq!(listed["items"][0]["content_type"], "application/pdf");
    assert_eq!(listed["total"], 1);

    let (status, bytes, ctype) = raw(
        clone_router(&router),
        get(&format!("/api/v1/attachments/{file_id}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("pdf"), "{ctype}");
    assert_eq!(bytes, PDF);

    let (status, patched) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/attachments/{file_id}"),
            Some(&token),
            json!({ "filename": "Invoice-2026.pdf", "description": "August invoice" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["filename"], "Invoice-2026.pdf");
    assert_eq!(patched["description"], "August invoice");
    assert!(patched.get("storage_key").is_none());

    let (status, search) = json(
        clone_router(&router),
        get("/api/v1/search?q=Invoice-2026", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let groups = search["groups"].as_array().cloned().unwrap_or_default();
    assert!(
        groups.iter().any(|g| g["label"] == "Attachments"
            && g["hits"]
                .as_array()
                .is_some_and(|hits| { hits.iter().any(|h| h["label"] == "Invoice-2026.pdf") })),
        "{search}"
    );

    let (status, activity) = json(
        clone_router(&router),
        get(&format!("/api/v1/file-hosts/{id}/activity"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{activity}");
    let items = activity["items"].as_array().cloned().unwrap_or_default();
    assert!(
        items.iter().any(|i| i["message"]
            .as_str()
            .is_some_and(|m| m.contains("attached"))),
        "{activity}"
    );

    let (status, deleted) = json(
        clone_router(&router),
        delete(&format!("/api/v1/attachments/{file_id}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{deleted}");

    let (status, missing) = json(
        clone_router(&router),
        get(&format!("/api/v1/attachments/{file_id}"), Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
}

#[tokio::test]
async fn tenant_and_permission_isolation() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let slug_a = format!("fta-{suffix}");
    let admin_a = register(&router, &format!("fta-{suffix}@ex.com"), &slug_a).await;
    let admin_b = register(
        &router,
        &format!("ftb-{suffix}@ex.com"),
        &format!("ftb-{suffix}"),
    )
    .await;
    let staff = staff_token(&router, &admin_a, suffix, &slug_a).await;
    let (id, file_id) = host_with_file(&router, &admin_a).await;

    let (status, cross_list) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            Some(&admin_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{cross_list}");

    let (status, cross_get) = json(
        clone_router(&router),
        get(&format!("/api/v1/attachments/{file_id}"), Some(&admin_b)),
    )
    .await;
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN,
        "{cross_get}"
    );

    let (status, staff_list) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            Some(&staff),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{staff_list}");

    let (status, staff_bytes, _) = raw(
        clone_router(&router),
        get(&format!("/api/v1/attachments/{file_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(staff_bytes, PDF);

    let (status, staff_upload) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            &staff,
            "Receipt.pdf",
            "application/pdf",
            PDF,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_upload}");

    let (status, staff_delete) = json(
        clone_router(&router),
        delete(&format!("/api/v1/attachments/{file_id}"), Some(&staff)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_delete}");
}

#[tokio::test]
async fn rejects_invalid_entity_path_traversal_spoofed_mime_and_empty() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("fv-{suffix}@ex.com"),
        &format!("fv-{suffix}"),
    )
    .await;
    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/file-hosts",
            Some(&token),
            json!({ "name": "Host" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();

    let (status, missing) = json(
        clone_router(&router),
        get(
            &format!("/api/v1/file-hosts/{}/attachments", Uuid::nil()),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");

    let (status, traversal) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            &token,
            "../secret.pdf",
            "application/pdf",
            PDF,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{traversal}");

    let (status, spoof) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            &token,
            "photo.png",
            "image/png",
            PDF,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{spoof}");

    let (status, empty) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/file-hosts/{id}/attachments"),
            &token,
            "empty.pdf",
            "application/pdf",
            b"",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{empty}");

    let (status, bare) = json(
        clone_router(&router),
        post(
            "/api/v1/file-bares",
            Some(&token),
            json!({ "name": "No files" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bare}");
    let bare_id = bare["id"].as_str().unwrap();
    let (status, disabled) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/file-bares/{bare_id}/attachments"),
            &token,
            "Invoice.pdf",
            "application/pdf",
            PDF,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{disabled}");
}

#[tokio::test]
async fn replace_keeps_id_and_list_count_is_metadata_only() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let token = register(
        &router,
        &format!("fr-{suffix}@ex.com"),
        &format!("fr-{suffix}"),
    )
    .await;
    let (id, file_id) = host_with_file(&router, &token).await;
    let replacement = b"%PDF-1.4 replaced";
    let (status, replaced) = json(
        clone_router(&router),
        multipart_file(
            &format!("/api/v1/attachments/{file_id}/replace"),
            &token,
            "Invoice-v2.pdf",
            "application/pdf",
            replacement,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["id"], file_id);
    assert_eq!(replaced["filename"], "Invoice-v2.pdf");
    assert!(replaced.get("storage_key").is_none());

    let (status, bytes, _) = raw(
        clone_router(&router),
        get(
            &format!("/api/v1/attachments/{file_id}?disposition=inline"),
            Some(&token),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, replacement);

    let (status, listed) = json(
        clone_router(&router),
        get("/api/v1/file-hosts", Some(&token)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let row = listed["items"]
        .as_array()
        .and_then(|items| items.iter().find(|r| r["id"] == id));
    assert_eq!(row.unwrap()["_attachment_count"], 1, "{listed}");
}
