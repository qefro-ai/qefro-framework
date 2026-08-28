//! Identity foundation: Person vs User vs business records.

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
        AppModule::new("identity_demo")
            .entity(
                EntityDef::new("ShopCustomer")
                    .table_name("id_shop_customers")
                    .slug_name("shop-customers")
                    .label("Shop customer")
                    .label_plural("Shop customers")
                    .field(FieldDef::string("name").required().searchable())
                    .field(FieldDef::string("email").nullable().email())
                    .field(
                        FieldDef::many_to_one("person_id", "Person")
                            .nullable()
                            .label("Person"),
                    )
                    .build(),
            )
            .build(),
    )
    .permission(PermissionGrant::crud(ROLE_STAFF, "ShopCustomer"))
}

async fn runtime() -> axum::Router {
    let mut rt = QefroRuntime::new(Config {
        database_url: db_url(),
        jwt_secret: "identity-test-secret".into(),
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

fn assert_no_secrets(value: &Value) {
    let blob = value.to_string();
    for key in [
        "password_hash",
        "token_hash",
        "password",
        "access_token",
        "secret",
    ] {
        if key == "access_token" && blob.contains("\"access_token\"") {
            // login responses include access_token by design
            continue;
        }
        assert!(
            !blob.contains(&format!("\"{key}\"")),
            "secret {key} leaked: {value}"
        );
    }
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

#[tokio::test]
async fn user_create_auth_enable_roles_and_tenant_isolation() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("ia-{suffix}@ex.com"),
        &format!("ia-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("ib-{suffix}@ex.com"),
        &format!("ib-{suffix}"),
    )
    .await;

    let (status, created) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin_a),
            json!({
                "name": "Staff Ada",
                "email": format!("staff-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let user_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["roles"], json!(["Staff"]));
    assert_eq!(created["enabled"], true);
    assert!(created.get("password_hash").is_none(), "{created}");
    assert!(created.get("password").is_none(), "{created}");
    assert_no_secrets(&created);

    let (status, listed) = json(clone_router(&router), get("/api/v1/users", Some(&admin_a))).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let items = listed["items"].as_array().unwrap();
    assert!(items.iter().any(|u| u["id"] == user_id));
    for item in items {
        assert!(item.get("password_hash").is_none(), "{item}");
    }

    let (status, other) = json(
        clone_router(&router),
        get(&format!("/api/v1/users/{user_id}"), Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{other}");

    let (status, login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("staff-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("ia-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let staff = login["access_token"].as_str().unwrap().to_string();

    let (status, staff_list) =
        json(clone_router(&router), get("/api/v1/users", Some(&staff))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{staff_list}");

    let (status, escalate) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&staff),
            json!({ "roles": ["Admin"] }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{escalate}");

    let (status, disabled) = json(
        clone_router(&router),
        patch(
            &format!("/api/v1/users/{user_id}"),
            Some(&admin_a),
            json!({ "enabled": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    assert_eq!(disabled["enabled"], false);

    let (status, blocked) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("staff-{suffix}@ex.com"),
                "password": "password123",
                "tenant_slug": format!("ia-{suffix}")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{blocked}");

    let (status, stale) = json(clone_router(&router), get("/api/v1/auth/me", Some(&staff))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{stale}");
}

#[tokio::test]
async fn person_is_tenant_scoped_and_not_a_duplicate_login() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin_a = register(
        &router,
        &format!("pa-{suffix}@ex.com"),
        &format!("pa-{suffix}"),
    )
    .await;
    let admin_b = register(
        &router,
        &format!("pb-{suffix}@ex.com"),
        &format!("pb-{suffix}"),
    )
    .await;

    let (status, person) = json(
        clone_router(&router),
        post(
            "/api/v1/people",
            Some(&admin_a),
            json!({
                "name": "Walk-in Guest",
                "email": format!("guest-{suffix}@ex.com"),
                "phone": "+1 555 0100"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{person}");
    let person_id = person["id"].as_str().unwrap().to_string();
    assert!(person.get("user_id").unwrap().is_null() || person.get("user_id").is_none());
    assert!(person.get("password").is_none());
    assert!(person.get("password_hash").is_none());

    let (status, cross) = json(
        clone_router(&router),
        get(&format!("/api/v1/people/{person_id}"), Some(&admin_b)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{cross}");

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/shop-customers",
            Some(&admin_a),
            json!({
                "name": "Walk-in Guest",
                "email": format!("guest-{suffix}@ex.com"),
                "person_id": person_id
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    assert_eq!(customer["person_id"], person_id);
    assert!(customer.get("password_hash").is_none());
    assert_ne!(customer["id"], person["id"]);
    assert_eq!(customer["name"], "Walk-in Guest");
    assert!(customer.get("email").is_some());

    let (status, person_get) = json(
        clone_router(&router),
        get(&format!("/api/v1/people/{person_id}"), Some(&admin_a)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{person_get}");
    assert_no_secrets(&person_get);
    let related = person_get.get("_related").cloned().unwrap_or(json!({}));
    let links = person_get.get("_links").cloned().unwrap_or(json!([]));
    let related_hit = related
        .as_object()
        .map(|m| {
            m.values().any(|bucket| {
                bucket["entity"] == "ShopCustomer"
                    && bucket["items"]
                        .as_array()
                        .map(|items| items.iter().any(|i| i["id"] == customer["id"]))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let links_hit = links
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|l| l["entity"] == "ShopCustomer" && l["relation"] == "person_id")
        })
        .unwrap_or(false);
    assert!(
        related_hit || links_hit,
        "Person should list related ShopCustomer via _related or _links: {person_get}"
    );

    let (status, customer_get) = json(
        clone_router(&router),
        get(
            &format!(
                "/api/v1/shop-customers/{}",
                customer["id"].as_str().unwrap()
            ),
            Some(&admin_a),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{customer_get}");
    assert_no_secrets(&customer_get);
    assert_eq!(customer_get["name"], "Walk-in Guest");
    assert_eq!(customer_get["_expanded"]["person_id"]["id"], person_id);
}

#[tokio::test]
async fn create_account_on_person_requires_user_permission() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("ca-{suffix}@ex.com"),
        &format!("ca-{suffix}"),
    )
    .await;

    let (status, staff_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("ca-staff-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{staff_user}");

    let (status, login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("ca-staff-{suffix}@ex.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{login}");
    let staff = login["access_token"].as_str().unwrap().to_string();

    let (status, denied) = json(
        clone_router(&router),
        post(
            "/api/v1/people",
            Some(&staff),
            json!({
                "name": "Portal User",
                "email": format!("portal-{suffix}@ex.com"),
                "create_account": true,
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, linked) = json(
        clone_router(&router),
        post(
            "/api/v1/people",
            Some(&admin),
            json!({
                "name": "Portal User",
                "email": format!("portal-{suffix}@ex.com"),
                "create_account": true,
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{linked}");
    assert!(linked.get("password").is_none(), "{linked}");
    let user_id = linked["user_id"].as_str().expect("user_id");
    let (status, user) = json(
        clone_router(&router),
        get(&format!("/api/v1/users/{user_id}"), Some(&admin)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    assert_eq!(user["email"], format!("portal-{suffix}@ex.com"));
    assert!(user.get("password_hash").is_none(), "{user}");
}

#[tokio::test]
async fn meta_and_agent_never_expose_user_secrets() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("meta-{suffix}@ex.com"),
        &format!("meta-{suffix}"),
    )
    .await;

    let (status, ui) = json(clone_router(&router), get("/api/v1/meta/ui", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{ui}");
    assert_eq!(ui["schema_version"], "1");
    let entities = ui["entities"].as_array().unwrap();
    let user = entities
        .iter()
        .find(|e| e["entity"] == "User")
        .expect("User entity");
    let fields = user["fields"].as_array().unwrap();
    assert!(!fields.iter().any(|f| f["name"] == "password_hash"));
    let password = fields.iter().find(|f| f["name"] == "password").unwrap();
    assert_eq!(password["secret"], true);
    assert_eq!(password["list_visible"], false);

    let (status, tools) = json(clone_router(&router), get("/api/v1/tools", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{tools}");
    let blob = tools.to_string();
    assert!(!blob.contains("password_hash"));

    let (status, staff_user) = json(
        clone_router(&router),
        post(
            "/api/v1/users",
            Some(&admin),
            json!({
                "name": "Staff",
                "email": format!("rbac-{suffix}@ex.com"),
                "password": "password123",
                "roles": ["Staff"]
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{staff_user}");
    let (status, login) = json(
        clone_router(&router),
        post(
            "/api/v1/auth/login",
            None,
            json!({
                "email": format!("rbac-{suffix}@ex.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    let staff = login["access_token"].as_str().unwrap();
    assert_eq!(status, StatusCode::OK);

    let (status, invoke) = json(
        clone_router(&router),
        post(
            "/api/v1/agent/tools/create_user/invoke",
            Some(staff),
            json!({
                "name": "Nope",
                "email": format!("nope-{suffix}@ex.com"),
                "password": "password123"
            }),
        ),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
        "{status} {invoke}"
    );
}

#[tokio::test]
async fn unlinked_customer_keeps_own_contact_fields() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("ul-{suffix}@ex.com"),
        &format!("ul-{suffix}"),
    )
    .await;

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/shop-customers",
            Some(&admin),
            json!({
                "name": "Counter Guest",
                "email": format!("counter-{suffix}@ex.com")
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    assert!(customer.get("person_id").unwrap().is_null() || customer.get("person_id").is_none());
    assert_eq!(customer["name"], "Counter Guest");
    assert_eq!(customer["email"], format!("counter-{suffix}@ex.com"));
    assert_no_secrets(&customer);

    let (status, got) = json(
        clone_router(&router),
        get(
            &format!(
                "/api/v1/shop-customers/{}",
                customer["id"].as_str().unwrap()
            ),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_eq!(got["name"], "Counter Guest");
    assert!(got
        .get("_expanded")
        .and_then(|e| e.get("person_id"))
        .is_none());
    assert_no_secrets(&got);
}

#[tokio::test]
async fn linked_person_displays_login_path_without_secrets() {
    let router = runtime().await;
    let suffix = &Uuid::new_v4().to_string()[..8];
    let admin = register(
        &router,
        &format!("lp-{suffix}@ex.com"),
        &format!("lp-{suffix}"),
    )
    .await;

    let (status, person) = json(
        clone_router(&router),
        post(
            "/api/v1/people",
            Some(&admin),
            json!({
                "name": "Ada Lovelace",
                "email": format!("ada-{suffix}@ex.com"),
                "phone": "+1 555 0101",
                "create_account": true,
                "password": "password123"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{person}");
    assert_no_secrets(&person);
    let person_id = person["id"].as_str().unwrap().to_string();
    let user_id = person["user_id"].as_str().expect("user_id");

    let (status, customer) = json(
        clone_router(&router),
        post(
            "/api/v1/shop-customers",
            Some(&admin),
            json!({
                "name": "Legacy Ada",
                "email": format!("legacy-{suffix}@ex.com"),
                "person_id": person_id
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{customer}");
    assert_eq!(customer["name"], "Legacy Ada");
    assert_eq!(customer["email"], format!("legacy-{suffix}@ex.com"));

    let (status, got) = json(
        clone_router(&router),
        get(
            &format!(
                "/api/v1/shop-customers/{}",
                customer["id"].as_str().unwrap()
            ),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    assert_no_secrets(&got);
    assert_eq!(got["name"], "Legacy Ada");
    let person_exp = &got["_expanded"]["person_id"];
    assert_eq!(person_exp["id"], person_id);
    assert_eq!(person_exp["slug"], "people");
    assert_eq!(person_exp["label"], "Ada Lovelace");
    let user_exp = &person_exp["_expanded"]["user_id"];
    assert_eq!(user_exp["id"], user_id);
    assert_eq!(user_exp["slug"], "users");
    assert_eq!(user_exp["enabled"], true);
    assert!(got.get("password_hash").is_none());
    assert!(person_exp.get("password_hash").is_none());
    assert!(user_exp.get("password_hash").is_none());
    assert!(user_exp.get("password").is_none());
}
