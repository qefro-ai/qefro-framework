use crate::error::ApiError;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use qefro_auth::AuthToken;
use qefro_core::{AppManifest, QefroError, TenantConfig};
use qefro_search::parse_query;
use qefro_tenant::Tenant;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
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
    Json(json!({ "status": "ok", "framework": "qefro", "version": "0.3.0" }))
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
    Auth(_ctx): Auth,
) -> Result<Json<Vec<Tenant>>, ApiError> {
    Ok(Json(state.tenants.list().await?))
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

async fn meta_entities(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
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
    Auth(_): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.registry().get(&name)?;
    Ok(Json(serde_json::to_value(&*entity).unwrap_or(json!({}))))
}

async fn meta_ui(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .map(|e| e.to_ui_meta())
        .collect();
    Json(json!({ "entities": entities }))
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
    Json(json!({ "tools": state.tools.available(&ctx, state.entities.permissions()) }))
}

async fn invoke_tool(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let result = state
        .tools
        .invoke(
            &crate::EntityServiceOps(state.entities.as_ref()),
            &ctx,
            &name,
            input,
        )
        .await?;
    Ok(Json(serde_json::to_value(result).unwrap_or(json!({}))))
}

async fn list_events(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    let events = state.entities.events().recent(100).await;
    Json(json!({ "items": events }))
}

fn reject_reserved(slug: &str) -> Result<(), ApiError> {
    const RESERVED: &[&str] = &[
        "auth", "meta", "tenants", "agent", "audit", "health", "events", "docs", "tools",
        "dashboards", "settings", "users", "operations", "jobs",
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

async fn meta_dashboards(State(state): State<AppState>, Auth(_): Auth) -> Json<Value> {
    Json(json!({ "dashboards": state.dashboards }))
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
