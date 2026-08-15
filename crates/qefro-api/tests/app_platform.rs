use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use qefro_api::{Config, InstalledApp, QefroRuntime};
use qefro_core::{
    extract_package, validate_bundle, write_package, AppBundle, AppModule, EntityDef, FieldDef,
};
use qefro_permissions::PermissionGrant;
use serde_json::{json, Value};
use tower::ServiceExt;

fn db_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
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

#[test]
fn package_validate_extract_cycle() {
    let root = std::env::temp_dir().join(format!("qefro-cycle-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("entities")).unwrap();
    std::fs::write(
        root.join("app.toml"),
        r#"name = "myshop"
version = "1.0.0"
label = "My Shop"
api_version = "1"
framework_version = ">=0.7"
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("entities/customer.yaml"),
        "name: Customer\nfields:\n  - name: name\n    type: string\n    required: true\n",
    )
    .unwrap();
    let bundle = AppBundle::load(&root).unwrap();
    let report = validate_bundle(&bundle, &[]);
    assert!(report.ok(), "{:?}", report.errors);
    let pkg = root.join("myshop-1.0.0.qefro");
    write_package(&root, &pkg, "myshop", "1.0.0").unwrap();
    let dest = root.join("extracted");
    extract_package(&pkg, &dest).unwrap();
    let loaded = AppBundle::load(&dest).unwrap();
    assert_eq!(loaded.manifest.name, "myshop");
    assert_eq!(loaded.entities.len(), 1);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn upgrade_adds_field_without_dropping_rows() {
    let Some(database_url) = db_url() else {
        return;
    };
    let v1 = InstalledApp::new(
        AppModule::new("upgrade_shop")
            .version("1.0.0")
            .entity(
                EntityDef::new("UpgradeItem")
                    .table_name("upgrade_items")
                    .slug_name("upgrade-items")
                    .field(FieldDef::string("name").required())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud("Staff", "UpgradeItem"));
    let mut runtime = QefroRuntime::new(Config {
        database_url: database_url.clone(),
        jwt_secret: "test".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    runtime.install(v1);
    let (router, _) = runtime.build().await.expect("v1 build");
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
                    "tenant_name": "Shop",
                    "tenant_slug": format!("shop-{suffix}")
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = body["access_token"].as_str().unwrap();
    let (status, created) = json(
        clone_router(&router),
        Request::builder()
            .method("POST")
            .uri("/api/v1/upgrade-items")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(json!({ "name": "kept" }).to_string()))
            .unwrap(),
    )
    .await;
    assert!(status.is_success(), "{created}");
    let id = created["id"].as_str().unwrap().to_string();

    let v2 = InstalledApp::new(
        AppModule::new("upgrade_shop")
            .version("1.1.0")
            .entity(
                EntityDef::new("UpgradeItem")
                    .table_name("upgrade_items")
                    .slug_name("upgrade-items")
                    .field(FieldDef::string("name").required())
                    .field(FieldDef::string("source").nullable())
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud("Staff", "UpgradeItem"));
    let mut runtime = QefroRuntime::new(Config {
        database_url,
        jwt_secret: "test".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    runtime.install(v2);
    let (router, _) = runtime.build().await.expect("v2 build");
    let (status, got) = json(
        clone_router(&router),
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/upgrade-items/{id}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["name"], "kept");
}

#[tokio::test]
async fn yaml_runtime_exposes_entity_like_rust() {
    let Some(database_url) = db_url() else {
        return;
    };
    let rust = InstalledApp::new(
        AppModule::new("compat")
            .entity(
                EntityDef::new("CompatCustomer")
                    .table_name("compat_customers")
                    .slug_name("compat-customers")
                    .field(FieldDef::string("name").required().searchable())
                    .build(),
            )
            .build(),
    );
    let mut runtime = QefroRuntime::new(Config {
        database_url,
        jwt_secret: "test".into(),
        bind: "127.0.0.1:0".into(),
        ..Config::default()
    });
    runtime.install(rust);
    let names = runtime.entity_names();
    assert!(names.iter().any(|n| n == "CompatCustomer"));
    let yaml = EntityDef::from_yaml(
        "name: CompatCustomer\nfields:\n  - name: name\n    type: string\n    required: true\n    searchable: true\n",
    )
    .unwrap();
    assert_eq!(yaml.name, "CompatCustomer");
}
