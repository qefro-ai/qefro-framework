use crate::error::ApiError;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use qefro_auth::AuthToken;
use qefro_core::{AppManifest, QefroError, TenantConfig};
use qefro_db::BlobMeta;
use qefro_search::parse_query;
use qefro_tenant::Tenant;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/switch-tenant", post(switch_tenant))
        .route("/api/v1/users", post(create_user))
        .route("/api/v1/tenants", get(list_tenants).post(create_tenant))
        .route("/api/v1/meta/entities", get(meta_entities))
        .route("/api/v1/meta/entities/{name}", get(meta_entity))
        .route("/api/v1/meta/ui", get(meta_ui))
        .route("/api/v1/meta/permissions", get(meta_permissions))
        .route("/api/v1/meta/workflows", get(meta_workflows))
        .route("/api/v1/meta/modules", get(meta_modules))
        .route("/api/v1/meta/dashboards", get(meta_dashboards))
        .route("/api/openapi.json", get(openapi))
        .route("/docs", get(docs))
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/tools", get(list_tools))
        .route("/api/v1/operations", get(list_operations))
        .route("/api/v1/agent/tools", get(list_tools))
        .route("/api/v1/agent/tools/{name}/invoke", post(invoke_tool))
        .route("/api/v1/events", get(list_events))
        .route("/api/v1/tenants/me/config", get(get_tenant_config).patch(patch_tenant_config))
        .route("/api/v1/tenant", get(get_tenant).patch(patch_tenant))
        .route("/api/v1/tenant/branding", get(get_branding).patch(patch_branding))
        .route("/api/v1/tenant/apps", get(get_apps).patch(patch_apps))
        .route("/api/v1/tenant/features", get(get_features).patch(patch_features))
        .route("/api/v1/files", post(upload_file))
        .route("/api/v1/files/{key}", get(download_file).delete(delete_file))
        .route("/api/v1/saved-filters", get(list_saved_filters).post(create_saved_filter))
        .route("/api/v1/saved-filters/{id}", axum::routing::delete(delete_saved_filter))
        .route("/api/v1/dashboards/{name}", get(get_dashboard))
        .route("/api/v1/{slug}/{id}/workflow", get(get_workflow_state))
        .route("/api/v1/{slug}/{id}/transition", post(transition_entity))
        .route("/api/v1/{slug}/{id}/actions/{name}", post(execute_action))
        .route("/api/v1/{slug}/{id}/actions", get(list_record_actions))
        .route(
            "/api/v1/{slug}/{id}",
            get(get_entity).patch(patch_entity).delete(delete_entity),
        )
        .route("/api/v1/{slug}", get(list_entities).post(create_entity))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    qefro_db::pool::ping(state.entities.pool())
        .await
        .map_err(|_| QefroError::internal("not ready"))?;
    Ok(Json(json!({ "status": "ready" })))
}

#[derive(Deserialize)]
struct RegisterBody {
    name: String,
    email: String,
    password: String,
    tenant_name: String,
    tenant_slug: String,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<AuthToken>, ApiError> {
    let token = state
        .auth
        .register(
            &body.name,
            &body.email,
            &body.password,
            &body.tenant_name,
            &body.tenant_slug,
        )
        .await?;
    Ok(Json(token))
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
    tenant_slug: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginBody>,
) -> Result<Json<AuthToken>, ApiError> {
    let token = state
        .auth
        .login(&body.email, &body.password, body.tenant_slug.as_deref())
        .await?;
    Ok(Json(token))
}

async fn logout(State(state): State<AppState>, Auth(ctx): Auth) -> Result<StatusCode, ApiError> {
    if let Some(sid) = ctx.session_id {
        state.auth.logout(sid).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(state): State<AppState>, Auth(ctx): Auth) -> Result<Json<Value>, ApiError> {
    let user = state.auth.get_user(ctx.user_id).await?;
    Ok(Json(json!({
        "user": user,
        "tenant_id": ctx.tenant_id,
        "roles": ctx.roles,
        "enabled_apps": ctx.enabled_apps,
        "timezone": ctx.timezone,
        "locale": ctx.locale,
        "plan": ctx.plan,
        "request_id": ctx.request_id,
    })))
}

#[derive(Deserialize)]
struct SwitchBody {
    tenant_id: Uuid,
}

async fn switch_tenant(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<SwitchBody>,
) -> Result<Json<AuthToken>, ApiError> {
    let token = state
        .auth
        .switch_tenant(ctx.user_id, body.tenant_id)
        .await?;
    Ok(Json(token))
}

async fn list_tenants(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Vec<Tenant>>, ApiError> {
    Ok(Json(vec![state.tenants.get(ctx.tenant_id).await?]))
}

#[derive(Deserialize)]
struct CreateTenantBody {
    name: String,
    slug: String,
}

async fn create_tenant(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<CreateTenantBody>,
) -> Result<Json<Tenant>, ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("only Admin can create tenants").into());
    }
    Ok(Json(state.tenants.create(&body.name, &body.slug).await?))
}

#[derive(Deserialize)]
struct CreateUserBody {
    name: String,
    email: String,
    password: String,
    #[serde(default)]
    roles: Vec<String>,
}

async fn create_user(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<CreateUserBody>,
) -> Result<(StatusCode, Json<qefro_auth::User>), ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("only Admin can create users").into());
    }
    let roles = if body.roles.is_empty() {
        vec!["Staff".into()]
    } else {
        body.roles
    };
    let user = state
        .auth
        .create_user_in_tenant(ctx.tenant_id, &body.name, &body.email, &body.password, roles)
        .await?;
    Ok((StatusCode::CREATED, Json(user)))
}

async fn meta_entities(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .filter(|e| ctx.allows_app(e.module.as_deref()))
        .map(|e| {
            json!({
                "name": e.name,
                "slug": e.slug,
                "label": e.label,
                "label_plural": e.label_plural,
                "module": e.module,
                "workflow": e.workflow,
            })
        })
        .collect();
    Json(json!({ "entities": entities }))
}

async fn meta_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.registry().get(&name)?;
    if !ctx.allows_app(entity.module.as_deref()) {
        return Err(QefroError::not_found(format!("entity '{name}' not found")).into());
    }
    Ok(Json(serde_json::to_value(&*entity).unwrap_or(json!({}))))
}

async fn meta_ui(State(state): State<AppState>, Auth(ctx): Auth) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .filter(|e| ctx.allows_app(e.module.as_deref()))
        .map(|e| {
            let mut meta = e.to_ui_meta();
            meta.apply_terminology(&config.ui_config.terminology);
            meta
        })
        .collect();
    Ok(Json(json!({
        "schema_version": qefro_core::UI_SCHEMA_VERSION,
        "entities": entities,
        "branding": config.branding,
        "enabled_apps": ctx.enabled_apps,
        "features": config.features.flags,
        "locale": config.business.locale,
        "timezone": config.business.timezone,
        "currency": config.business.currency,
        "date_format": config.business.date_format,
        "number_format": config.business.number_format,
        "navigation": config.ui_config.navigation,
        "terminology": config.ui_config.terminology,
        "default_dashboard": config.ui_config.default_dashboard,
    })))
}

async fn meta_permissions(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    Json(json!({ "grants": state.entities.permissions().grants() }))
}

async fn meta_workflows(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    Json(json!({ "workflows": state.entities.workflows().list() }))
}

async fn meta_modules(State(state): State<AppState>, Auth(_): Auth) -> Json<Vec<AppManifest>> {
    Json(state.modules.clone())
}

async fn openapi(State(state): State<AppState>) -> Json<Value> {
    Json(crate::openapi::spec(&state))
}

async fn docs() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("docs.html"))
}

async fn list_audit(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let entity = params.get("entity").map(|s| s.as_str());
    let entity_id = params
        .get("entity_id")
        .and_then(|s| Uuid::parse_str(s).ok());
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let rows = state
        .entities
        .audit()
        .list(&ctx, entity, entity_id, limit)
        .await?;
    Ok(Json(json!({ "items": rows })))
}

async fn list_tools(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let tools: Vec<_> = state
        .tools
        .available(&ctx, state.entities.permissions())
        .into_iter()
        .filter(|t| {
            state
                .entities
                .registry()
                .try_get(&t.entity)
                .map(|e| ctx.allows_app(e.module.as_deref()))
                .unwrap_or(false)
        })
        .collect();
    Json(json!({ "tools": tools }))
}

async fn invoke_tool(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if !ctx.feature_allowed("agent_actions") {
        return Err(QefroError::forbidden("agent actions are disabled for this tenant").into());
    }
    let tool = state.tools.get(&name)?.clone();
    if let Some(entity) = state.entities.registry().try_get(&tool.entity) {
        if !ctx.allows_app(entity.module.as_deref()) {
            return Err(QefroError::not_found(format!("tool '{name}' not found")).into());
        }
    }
    let result = state
        .tools
        .invoke(
            &crate::EntityServiceOps(state.entities.as_ref()),
            &ctx,
            &name,
            input,
        )
        .await?;
    tracing::info!(
        request_id = %ctx.request_id,
        tenant_id = %ctx.tenant_id,
        user_id = %ctx.user_id,
        operation = %name,
        entity = %tool.entity,
        status = "success",
        "agent.tool.executed"
    );
    qefro_core::MeteringEvent::new(
        ctx.tenant_id,
        "agent.tool.executed",
        &tool.entity,
        ctx.request_id,
    )
    .with_user(ctx.user_id)
    .emit();
    Ok(Json(serde_json::to_value(result).unwrap_or(json!({}))))
}

async fn list_events(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let events = state.entities.events().recent_for_tenant(ctx.tenant_id, 100).await;
    Json(json!({ "items": events }))
}

fn reject_reserved(slug: &str) -> Result<(), ApiError> {
    const RESERVED: &[&str] = &[
        "auth", "meta", "tenants", "tenant", "agent", "audit", "health", "ready", "events", "docs",
        "tools", "dashboards", "settings", "users", "operations", "jobs",
        "files", "saved-filters",
    ];
    if RESERVED.contains(&slug) {
        Err(QefroError::not_found(format!("entity '{slug}' not found")).into())
    } else {
        Ok(())
    }
}

async fn list_entities(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let entity = state.entities.registry().get(&slug)?;
    let raw: Vec<(String, String)> = params.into_iter().collect();
    let query = parse_query(&entity, &raw)?;
    let page = state.entities.list(&ctx, &entity.name, query).await?;
    Ok(Json(serde_json::to_value(page).unwrap_or(json!({}))))
}

async fn create_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    reject_reserved(&slug)?;
    let created = state.entities.create(&ctx, &slug, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    Ok(Json(state.entities.get(&ctx, &slug, id).await?))
}

async fn patch_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    Ok(Json(state.entities.update(&ctx, &slug, id, body).await?))
}

async fn delete_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Result<StatusCode, ApiError> {
    reject_reserved(&slug)?;
    state.entities.delete(&ctx, &slug, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TransitionBody {
    transition: String,
}

async fn transition_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Json(body): Json<TransitionBody>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    Ok(Json(
        state
            .entities
            .transition(&ctx, &slug, id, &body.transition)
            .await?,
    ))
}

async fn list_operations(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Json<Value> {
    let operations: Vec<_> = state
        .entities
        .list_operations(&ctx)
        .into_iter()
        .map(|d| d.to_client_json())
        .collect();
    Json(json!({ "operations": operations }))
}

async fn list_record_actions(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let record = state.entities.get(&ctx, &slug, id).await?;
    Ok(Json(json!({
        "actions": record.get("_actions").cloned().unwrap_or(json!([])),
    })))
}

async fn execute_action(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id, name)): Path<(String, Uuid, String)>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    Ok(Json(
        state
            .entities
            .execute(
                &ctx,
                &slug,
                id,
                &name,
                body.map(|j| j.0).unwrap_or_else(|| json!({})),
            )
            .await?,
    ))
}

async fn get_workflow_state(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let record = state.entities.get(&ctx, &slug, id).await?;
    Ok(Json(state.entities.workflow_snapshot(&ctx, &slug, &record)))
}

async fn meta_dashboards(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let dashboards: Vec<_> = state
        .dashboards
        .iter()
        .filter(|d| ctx.allows_app(d.module.as_deref()))
        .cloned()
        .collect();
    Json(json!({ "dashboards": dashboards }))
}

async fn get_dashboard(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let dash = state
        .dashboards
        .iter()
        .find(|d| d.name == name)
        .ok_or_else(|| QefroError::not_found(format!("dashboard '{name}' not found")))?;
    if !ctx.allows_app(dash.module.as_deref()) {
        return Err(QefroError::not_found(format!("dashboard '{name}' not found")).into());
    }
    let mut cards = Vec::new();
    for card in &dash.cards {
        cards.push(state.entities.dashboard_card_value(&ctx, card).await?);
    }
    Ok(Json(json!({
        "name": dash.name,
        "label": dash.label,
        "module": dash.module,
        "cards": cards,
    })))
}

async fn get_tenant_config(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(serde_json::to_value(config).unwrap_or(json!({}))))
}

async fn patch_tenant_config(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<TenantConfig>,
) -> Result<Json<Value>, ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("only Admin can update tenant configuration").into());
    }
    let config = state.tenants.upsert_config(ctx.tenant_id, &body).await?;
    Ok(Json(serde_json::to_value(config).unwrap_or(json!({}))))
}

fn require_admin(ctx: &qefro_core::OpContext) -> Result<(), ApiError> {
    if ctx.is_admin() {
        Ok(())
    } else {
        Err(QefroError::forbidden("only Admin can update tenant configuration").into())
    }
}

async fn get_tenant(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let tenant = state.tenants.get(ctx.tenant_id).await?;
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(json!({
        "id": tenant.id,
        "name": tenant.name,
        "slug": tenant.slug,
        "created_at": tenant.created_at,
        "branding": config.branding,
        "ui_config": config.ui_config,
        "enabled_apps": ctx.enabled_apps,
        "features": config.features.flags,
        "business": config.business,
        "plan": config.plan,
        "installed_apps": state.installed_apps,
    })))
}

async fn patch_tenant(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<TenantConfig>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;
    reject_client_tenant_field(&serde_json::to_value(&body).unwrap_or(json!({})))?;
    let config = state.tenants.upsert_config(ctx.tenant_id, &body).await?;
    Ok(Json(serde_json::to_value(config).unwrap_or(json!({}))))
}

async fn get_branding(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(serde_json::to_value(config.branding).unwrap_or(json!({}))))
}

async fn patch_branding(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;
    reject_client_tenant_field(&body)?;
    let branding: qefro_core::TenantBranding =
        serde_json::from_value(body).map_err(|e| QefroError::bad_request(e.to_string()))?;
    let mut config = state.tenants.get_config(ctx.tenant_id).await?;
    config.branding = branding;
    let config = state.tenants.upsert_config(ctx.tenant_id, &config).await?;
    Ok(Json(serde_json::to_value(config.branding).unwrap_or(json!({}))))
}

#[derive(Deserialize)]
struct AppsBody {
    enabled_apps: Vec<String>,
}

async fn get_apps(State(state): State<AppState>, Auth(ctx): Auth) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(json!({
        "installed": state.installed_apps,
        "enabled": ctx.enabled_apps,
        "configured": config.enabled_apps,
        "plan": config.plan,
    })))
}

async fn patch_apps(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<AppsBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;
    let mut config = state.tenants.get_config(ctx.tenant_id).await?;
    for app in &body.enabled_apps {
        if !state
            .entitlements
            .can_enable(app, &state.installed_apps, config.plan.as_deref())
        {
            return Err(QefroError::forbidden(format!(
                "application '{app}' is not available on this plan"
            ))
            .into());
        }
    }
    config.enabled_apps = body.enabled_apps;
    let config = state.tenants.upsert_config(ctx.tenant_id, &config).await?;
    Ok(Json(json!({
        "enabled": state.entitlements.resolve_apps(
            &state.installed_apps,
            &config.enabled_apps,
            config.plan.as_deref(),
        ),
        "configured": config.enabled_apps,
    })))
}

#[derive(Deserialize)]
struct FeaturesBody {
    flags: std::collections::HashMap<String, bool>,
}

async fn get_features(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(json!({ "flags": config.features.flags })))
}

async fn patch_features(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<FeaturesBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;
    let mut config = state.tenants.get_config(ctx.tenant_id).await?;
    config.features.flags = body.flags;
    let config = state.tenants.upsert_config(ctx.tenant_id, &config).await?;
    Ok(Json(json!({ "flags": config.features.flags })))
}

fn reject_client_tenant_field(data: &Value) -> Result<(), ApiError> {
    if data.get("tenant_id").is_some() {
        Err(QefroError::bad_request("tenant_id cannot be set by the client").into())
    } else {
        Ok(())
    }
}

const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

async fn upload_file(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let kind = params.get("kind").map(|s| s.as_str()).unwrap_or("file");
    let mut filename = String::from("upload.bin");
    let mut content_type = String::from("application/octet-stream");
    let mut bytes = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| QefroError::bad_request(format!("multipart: {e}")))?
    {
        if let Some(name) = field.file_name() {
            filename = sanitize_filename(name);
        }
        if let Some(ct) = field.content_type() {
            content_type = ct.to_string();
        }
        bytes = field
            .bytes()
            .await
            .map_err(|e| QefroError::bad_request(format!("upload: {e}")))?
            .to_vec();
    }
    if bytes.is_empty() {
        return Err(QefroError::bad_request("file is required").into());
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(QefroError::bad_request("file exceeds 8MB limit").into());
    }
    if kind == "image" && !content_type.starts_with("image/") {
        return Err(QefroError::bad_request("image uploads must be an image MIME type").into());
    }
    if !allowed_mime(&content_type) {
        return Err(QefroError::bad_request("file type is not allowed").into());
    }
    let key = format!("{}-{}", Uuid::new_v4(), filename);
    state.blob_store.put(ctx.tenant_id, &key, &bytes)?;
    let meta = BlobMeta {
        key: key.clone(),
        filename,
        content_type: content_type.clone(),
        size: bytes.len() as i64,
    };
    state.blobs.insert(ctx.tenant_id, ctx.user_id, &meta).await?;
    Ok(Json(json!({
        "key": meta.key,
        "filename": meta.filename,
        "content_type": meta.content_type,
        "size": meta.size,
        "url": format!("/api/v1/files/{}", meta.key),
    })))
}

async fn download_file(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let meta = state.blobs.get(ctx.tenant_id, &key).await?;
    let bytes = state.blob_store.get(ctx.tenant_id, &key)?;
    let disposition = format!("inline; filename=\"{}\"", meta.filename.replace('"', ""));
    Ok((
        [
            (header::CONTENT_TYPE, meta.content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

async fn delete_file(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _ = state.blobs.get(ctx.tenant_id, &key).await?;
    state.blob_store.delete(ctx.tenant_id, &key)?;
    state.blobs.delete(ctx.tenant_id, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "upload.bin".into()
    } else {
        cleaned.chars().take(80).collect()
    }
}

fn allowed_mime(ct: &str) -> bool {
    ct.starts_with("image/")
        || ct.starts_with("text/")
        || matches!(
            ct,
            "application/pdf"
                | "application/json"
                | "application/octet-stream"
                | "application/zip"
        )
}

#[derive(Deserialize)]
struct SavedFilterBody {
    entity: String,
    name: String,
    #[serde(default)]
    query: Value,
}

async fn list_saved_filters(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let entity = params
        .get("entity")
        .ok_or_else(|| QefroError::bad_request("entity is required"))?;
    ensure_entity_app(&state, &ctx, entity)?;
    let items = state
        .saved_filters
        .list(ctx.tenant_id, ctx.user_id, entity)
        .await?;
    Ok(Json(json!({ "items": items })))
}

async fn create_saved_filter(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<SavedFilterBody>,
) -> Result<Json<Value>, ApiError> {
    ensure_entity_app(&state, &ctx, &body.entity)?;
    if body.name.trim().is_empty() {
        return Err(QefroError::bad_request("name is required").into());
    }
    let created = state
        .saved_filters
        .create(
            ctx.tenant_id,
            ctx.user_id,
            &body.entity,
            body.name.trim(),
            body.query,
        )
        .await?;
    Ok(Json(serde_json::to_value(created).unwrap_or(json!({}))))
}

async fn delete_saved_filter(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .saved_filters
        .delete(ctx.tenant_id, ctx.user_id, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn ensure_entity_app(state: &AppState, ctx: &qefro_core::OpContext, name: &str) -> Result<(), ApiError> {
    let entity = state.entities.registry().get(name)?;
    if !ctx.allows_app(entity.module.as_deref()) {
        return Err(QefroError::not_found(format!("entity '{name}' not found")).into());
    }
    Ok(())
}
