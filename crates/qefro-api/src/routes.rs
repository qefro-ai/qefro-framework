use crate::error::ApiError;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use qefro_auth::AuthToken;
use qefro_core::{AppManifest, QefroError, RateLimiter, TenantConfig};
use qefro_db::BlobMeta;
use qefro_permissions::Action;
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
        .route("/metrics", get(metrics))
        .route("/api/v1/meta/version", get(meta_version))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/switch-tenant", post(switch_tenant))
        .route("/api/v1/tenants", get(list_tenants).post(create_tenant))
        .route("/api/v1/meta/entities", get(meta_entities))
        .route("/api/v1/meta/entities/{name}", get(meta_entity))
        .route("/api/v1/meta/ui", get(meta_ui))
        .route("/api/v1/meta/permissions", get(meta_permissions))
        .route("/api/v1/meta/workflows", get(meta_workflows))
        .route("/api/v1/meta/modules", get(meta_modules))
        .route("/api/v1/meta/dashboards", get(meta_dashboards))
        .route("/api/v1/meta/pages", get(meta_pages))
        .route("/api/v1/meta/pages/{name}", get(meta_page))
        .route("/api/v1/meta/reports", get(meta_reports))
        .route("/api/v1/meta/workspace", get(meta_workspace))
        .nest("/api/v1/studio", crate::studio::router())
        .route("/api/openapi.json", get(openapi))
        .route("/docs", get(docs))
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/tools", get(list_tools))
        .route("/api/v1/operations", get(list_operations))
        .route("/api/v1/operation-runs/{id}", get(get_operation_run))
        .route("/api/v1/agent/tools", get(list_tools))
        .route("/api/v1/agent/tools/{name}/invoke", post(invoke_tool))
        .route("/api/v1/events", get(list_events))
        .merge(crate::platform::public_router())
        .merge(crate::platform::router())
        .route(
            "/api/v1/tenants/me/config",
            get(get_tenant_config).patch(patch_tenant_config),
        )
        .route("/api/v1/tenant", get(get_tenant).patch(patch_tenant))
        .route(
            "/api/v1/tenant/branding",
            get(get_branding).patch(patch_branding),
        )
        .route("/api/v1/tenant/apps", get(get_apps).patch(patch_apps))
        .route(
            "/api/v1/tenant/features",
            get(get_features).patch(patch_features),
        )
        .route("/api/v1/files", post(upload_file))
        .route(
            "/api/v1/files/{key}",
            get(download_file).delete(delete_file),
        )
        .route(
            "/api/v1/saved-filters",
            get(list_saved_filters).post(create_saved_filter),
        )
        .route(
            "/api/v1/saved-filters/{id}",
            axum::routing::delete(delete_saved_filter),
        )
        .route(
            "/api/v1/saved-views",
            get(list_saved_filters).post(create_saved_filter),
        )
        .route(
            "/api/v1/saved-views/{id}",
            axum::routing::delete(delete_saved_filter),
        )
        .route("/api/v1/dashboards/{name}", get(get_dashboard))
        .route("/api/v1/reports", get(meta_reports))
        .route("/api/v1/reports/{name}", get(get_report))
        .route("/api/v1/reports/{name}/run", post(run_report))
        .route("/api/v1/{slug}/aggregates", get(entity_aggregates))
        .route("/api/v1/{slug}/bulk", post(bulk_entities))
        .route("/api/v1/{slug}/export", get(export_entities))
        .route("/api/v1/{slug}/{id}/print", get(print_document))
        .route("/api/v1/{slug}/{id}/print.pdf", get(print_document_pdf))
        .route("/api/v1/{slug}/{id}/preview", get(print_document))
        .route("/api/v1/{slug}/{id}/workflow", get(get_workflow_state))
        .route("/api/v1/{slug}/{id}/transition", post(transition_entity))
        .route("/api/v1/{slug}/{id}/activity", get(list_activity))
        .route("/api/v1/{slug}/{id}/comments", post(add_comment))
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
    Json(json!({
        "status": "ok",
        "framework": qefro_core::FRAMEWORK_VERSION,
    }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    qefro_db::pool::ping(state.entities.pool())
        .await
        .map_err(|_| QefroError::internal("not ready"))?;
    Ok(Json(json!({
        "status": "ready",
        "database": true,
    })))
}

async fn meta_version() -> Json<Value> {
    Json(json!({
        "framework": qefro_core::FRAMEWORK_VERSION,
        "metadata_schema": qefro_core::METADATA_SCHEMA_VERSION,
        "ui_schema": qefro_core::UI_SCHEMA_VERSION,
        "api": qefro_core::API_VERSION,
        "app_package": qefro_core::APP_API_VERSION,
        "migration_format": qefro_core::MIGRATION_FORMAT_VERSION,
    }))
}

async fn metrics(State(state): State<AppState>) -> Json<Value> {
    let (http_requests, http_errors, http_latency_ms_total) = crate::metrics::http_snapshot();
    let jobs_pending = state
        .entities
        .job_queue()
        .pending_count()
        .await
        .unwrap_or(0);
    let outbox_pending = state.entities.outbox().pending_count().await.unwrap_or(0);
    Json(json!({
        "http_requests": http_requests,
        "http_errors": http_errors,
        "http_latency_ms_total": http_latency_ms_total,
        "jobs_pending": jobs_pending,
        "outbox_pending": outbox_pending,
        "sse_subscribers": state.realtime.subscriber_count(),
    }))
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
    if !state
        .login_limiter
        .allow(&format!("login:{}", body.email.to_ascii_lowercase()))
    {
        return Err(QefroError::rate_limited("too many login attempts").into());
    }
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
        "studio": qefro_core::studio_capabilities(&ctx.roles, &state.env),
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
    let tenant = state.tenants.get(ctx.tenant_id).await.ok();
    let branding = state.resolve_branding(&ctx, &config, tenant.as_ref().map(|t| t.name.as_str()));
    let permissions = state.entities.permissions();
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .filter(|e| ctx.allows_app(e.module.as_deref()))
        .map(|e| {
            let mut meta = e.to_ui_meta();
            meta.apply_terminology(&config.ui_config.terminology);
            meta.permissions = Some(qefro_core::EntityPermissions {
                list: permissions.allows(&ctx.roles, &e.name, Action::List),
                create: permissions.allows(&ctx.roles, &e.name, Action::Create),
                read: permissions.allows(&ctx.roles, &e.name, Action::Read),
                update: permissions.allows(&ctx.roles, &e.name, Action::Update),
                delete: permissions.allows(&ctx.roles, &e.name, Action::Delete),
                export: permissions.allows(&ctx.roles, &e.name, Action::Export),
            });
            let ops = state.entities.operations().for_entity(&e.name);
            if let Some(cap) = meta.capabilities.as_mut() {
                cap.actions = cap.actions || !ops.is_empty();
            }
            for binding in ops {
                if meta
                    .actions
                    .iter()
                    .any(|a| a.name == binding.def.name || a.operation == binding.def.name)
                {
                    continue;
                }
                let mut action = qefro_core::EntityActionDef::new(&binding.def.name)
                    .label(&binding.def.label)
                    .operation(&binding.def.name);
                if binding.def.requires_confirmation {
                    action = action.confirm(
                        binding
                            .def
                            .confirmation_message
                            .clone()
                            .unwrap_or_else(|| format!("Run {}?", binding.def.label)),
                    );
                }
                meta.actions.push(action);
            }
            meta
        })
        .collect();
    Ok(Json(json!({
        "schema_version": qefro_core::UI_SCHEMA_VERSION,
        "entities": entities,
        "branding": branding,
        "enabled_apps": ctx.enabled_apps,
        "features": config.features.flags,
        "locale": config.business.locale,
        "timezone": config.business.timezone,
        "currency": config.business.currency,
        "date_format": config.business.date_format,
        "number_format": config.business.number_format,
        "navigation": if config.ui_config.navigation.is_empty() {
            state.default_navigation.clone()
        } else {
            config.ui_config.navigation.clone()
        },
        "hidden_entities": if config.ui_config.hidden_entities.is_empty() {
            state.default_hidden_entities.clone()
        } else {
            config.ui_config.hidden_entities.clone()
        },
        "terminology": config.ui_config.terminology,
        "default_dashboard": if config
            .ui_config
            .default_dashboard
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            config.ui_config.default_dashboard.clone()
        } else {
            state.default_dashboard_name(&ctx)
        },
        "reports": state
            .reports_live()
            .into_iter()
            .filter(|r| ctx.allows_app(r.module.as_deref()))
            .collect::<Vec<_>>(),
        "workspace": workspace_payload(&state, &ctx, &config),
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
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("audit log requires Admin").into());
    }
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
    let items: Vec<Value> = rows.iter().map(|r| r.to_client_json()).collect();
    Ok(Json(json!({ "items": items })))
}

async fn list_tools(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let tools: Vec<_> = state
        .tools
        .available(&ctx, state.entities.permissions())
        .into_iter()
        .filter(|t| {
            if t.entity.is_empty() {
                return true;
            }
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
    let mut ctx = ctx;
    ctx.source = "agent".into();
    let result = state
        .tools
        .invoke(&crate::EntityServiceOps(&state), &ctx, &name, input)
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
    let events: Vec<Value> = state
        .entities
        .events()
        .recent_for_tenant(ctx.tenant_id, 100)
        .await
        .into_iter()
        .map(|e| e.to_public_json())
        .collect();
    Json(json!({ "items": events }))
}

fn reject_reserved(slug: &str) -> Result<(), ApiError> {
    const RESERVED: &[&str] = &[
        "auth",
        "meta",
        "tenants",
        "tenant",
        "agent",
        "audit",
        "health",
        "ready",
        "events",
        "docs",
        "tools",
        "dashboards",
        "settings",
        "operations",
        "operation-runs",
        "jobs",
        "files",
        "saved-filters",
        "saved-views",
        "reports",
        "print",
        "studio",
        "search",
        "notifications",
        "webhooks",
        "attachments",
        "realtime",
        "public",
        "workspace",
        "pages",
        "bulk",
        "export",
    ];
    if RESERVED.contains(&slug) {
        Err(QefroError::not_found(format!("entity '{slug}' not found")).into())
    } else {
        Ok(())
    }
}

async fn bulk_entities(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Json(body): Json<qefro_db::BulkRequest>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    Ok(Json(state.entities.bulk(&ctx, &slug, body).await?))
}

async fn export_entities(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, ApiError> {
    reject_reserved(&slug)?;
    let entity = state.entities.registry().get(&slug)?;
    let ids = params.get("ids").and_then(|raw| {
        let parsed: Vec<Uuid> = raw
            .split(',')
            .filter_map(|part| Uuid::parse_str(part.trim()).ok())
            .collect();
        if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        }
    });
    let raw: Vec<(String, String)> = params.into_iter().filter(|(key, _)| key != "ids").collect();
    let query = parse_query(&entity, &raw)?;
    let (filename, csv) = state
        .entities
        .export_csv(&ctx, &entity.name, query, ids)
        .await?;
    let disposition = format!(
        "attachment; filename=\"{}\"",
        filename.replace(['"', '/', '\\'], "")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        csv,
    ))
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

#[derive(Deserialize)]
struct CommentBody {
    message: String,
    #[serde(default)]
    attachment_id: Option<Uuid>,
}

async fn list_activity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let items = state.entities.list_activity(&ctx, &slug, id, limit).await?;
    Ok(Json(json!({ "items": items })))
}

async fn add_comment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Json(body): Json<CommentBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    reject_reserved(&slug)?;
    let row = state
        .entities
        .add_comment_with_attachment(&ctx, &slug, id, &body.message, body.attachment_id)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(row).unwrap_or(json!({}))),
    ))
}

async fn list_operations(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
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
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(Json(
        state
            .entities
            .execute_with(
                &ctx,
                &slug,
                id,
                &name,
                body.map(|j| j.0).unwrap_or_else(|| json!({})),
                qefro_db::ExecuteOpts {
                    idempotency_key,
                    force_sync: false,
                    operation_id: None,
                },
            )
            .await?,
    ))
}

async fn get_operation_run(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let run = state.entities.get_operation_run(&ctx, id).await?;
    Ok(Json(run.to_client_json()))
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
        .dashboards_live()
        .into_iter()
        .filter(|d| ctx.allows_app(d.module.as_deref()))
        .collect();
    Json(json!({ "dashboards": dashboards }))
}

async fn meta_pages(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let pages: Vec<_> = state
        .pages_live()
        .into_iter()
        .filter(|p| page_allowed(&state, &ctx, p))
        .map(|p| visible_page(&state, &ctx, p))
        .collect();
    Json(json!({ "pages": pages }))
}

async fn meta_page(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let page = state
        .pages_live()
        .into_iter()
        .find(|p| p.name == name || p.slug() == name)
        .ok_or_else(|| QefroError::not_found(format!("page '{name}' not found")))?;
    if !page_allowed(&state, &ctx, &page) {
        return Err(QefroError::not_found(format!("page '{name}' not found")).into());
    }
    Ok(Json(visible_page(&state, &ctx, page)))
}

fn nav_item_visible(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    item: &qefro_core::WorkspaceNavItem,
) -> bool {
    if !ctx.allows_app(item.module.as_deref()) {
        return false;
    }
    if let Some(page_slug) = &item.page {
        return state
            .pages_live()
            .into_iter()
            .find(|p| p.slug() == page_slug || p.name == *page_slug)
            .is_some_and(|p| page_allowed(state, ctx, &p));
    }
    if item.entity.is_empty() {
        return false;
    }
    state
        .entities
        .permissions()
        .allows(&ctx.roles, &item.entity, Action::List)
}

fn page_allowed(state: &AppState, ctx: &qefro_core::OpContext, page: &qefro_core::PageDef) -> bool {
    if !ctx.allows_app(page.module.as_deref()) {
        return false;
    }
    if !page.roles.is_empty() && !ctx.is_admin() && !page.roles.iter().any(|r| ctx.has_role(r)) {
        return false;
    }
    let _ = state;
    true
}

fn section_allowed(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    section: &qefro_core::PageSection,
) -> bool {
    if !section.roles.is_empty()
        && !ctx.is_admin()
        && !section.roles.iter().any(|r| ctx.has_role(r))
    {
        return false;
    }
    let permissions = state.entities.permissions();
    if let Some(entity) = section.entity_name() {
        if !permissions.allows(&ctx.roles, entity, Action::List) {
            return false;
        }
    }
    if let Some(report_name) = &section.report {
        if let Some(report) = state
            .reports_live()
            .into_iter()
            .find(|r| r.name == *report_name)
        {
            if !ctx.allows_app(report.module.as_deref())
                || !permissions.allows(&ctx.roles, &report.entity, Action::List)
            {
                return false;
            }
        }
    }
    if let Some(card) = &section.card {
        if !card.entity.is_empty()
            && !card.entity.starts_with('_')
            && !permissions.allows(&ctx.roles, &card.entity, Action::List)
        {
            return false;
        }
        if !card.roles.is_empty() && !ctx.is_admin() && !card.roles.iter().any(|r| ctx.has_role(r))
        {
            return false;
        }
    } else if let Some(dash_name) = &section.dashboard {
        let want = section
            .widget
            .as_deref()
            .or(Some(section.title.as_str()))
            .unwrap_or("");
        if let Some(dashboard) = state
            .dashboards_live()
            .into_iter()
            .find(|d| d.name == *dash_name)
        {
            if let Some(card) = dashboard
                .cards
                .iter()
                .find(|c| c.title == want || c.title == section.title)
            {
                if !card.entity.is_empty()
                    && !card.entity.starts_with('_')
                    && !permissions.allows(&ctx.roles, &card.entity, Action::List)
                {
                    return false;
                }
                if !card.roles.is_empty()
                    && !ctx.is_admin()
                    && !card.roles.iter().any(|r| ctx.has_role(r))
                {
                    return false;
                }
            }
        }
    }
    true
}

fn visible_page(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    mut page: qefro_core::PageDef,
) -> Value {
    page.sections
        .retain(|section| section_allowed(state, ctx, section));
    page.actions.retain(|action| {
        state
            .entities
            .permissions()
            .allows(&ctx.roles, &action.entity, Action::List)
    });
    json!(page)
}

async fn meta_workspace(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(workspace_payload(&state, &ctx, &config)))
}

fn workspace_payload(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    config: &TenantConfig,
) -> Value {
    let permissions = state.entities.permissions();
    let navigation: Vec<_> = state
        .default_nav_items
        .iter()
        .filter(|item| nav_item_visible(state, ctx, item))
        .cloned()
        .collect();
    let dashboards: Vec<_> = state
        .dashboards_live()
        .into_iter()
        .filter(|d| ctx.allows_app(d.module.as_deref()))
        .map(|d| json!({ "name": d.name, "label": d.label, "module": d.module }))
        .collect();
    let reports: Vec<_> = state
        .reports_live()
        .into_iter()
        .filter(|r| ctx.allows_app(r.module.as_deref()))
        .map(
            |r| json!({ "name": r.name, "label": r.label, "entity": r.entity, "module": r.module }),
        )
        .collect();
    let pages: Vec<_> = state
        .pages_live()
        .into_iter()
        .filter(|p| page_allowed(state, ctx, p))
        .map(|p| {
            json!({
                "name": p.name,
                "label": p.label,
                "slug": p.slug(),
                "module": p.module,
                "layout": p.layout,
                "route": p.route(),
            })
        })
        .collect();
    let default_dashboard = if config
        .ui_config
        .default_dashboard
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        config.ui_config.default_dashboard.clone()
    } else {
        state.default_dashboard_name(ctx)
    };
    let mut shortcuts = Vec::new();
    let mut seen_create = std::collections::HashSet::new();
    for item in &navigation {
        if seen_create.insert(item.entity.clone())
            && !item.entity.is_empty()
            && item.page.is_none()
            && permissions.allows(&ctx.roles, &item.entity, Action::Create)
        {
            let noun = item.label.trim_end_matches('s');
            shortcuts.push(json!({
                "label": format!("New {noun}"),
                "to": format!("/{}/new", item.slug),
                "entity": item.entity,
                "kind": "create",
            }));
        }
    }
    for item in &navigation {
        let mut search = Vec::new();
        if let Some(q) = &item.query {
            if !q.is_empty() {
                search.push(q.clone());
            }
        }
        if let Some(view) = &item.view {
            if !view.is_empty() {
                search.push(format!("view={view}"));
            }
        }
        let to = if let Some(page) = &item.page {
            format!("/pages/{page}")
        } else if search.is_empty() {
            format!("/{}", item.slug)
        } else {
            format!("/{}?{}", item.slug, search.join("&"))
        };
        shortcuts.push(json!({
            "label": item.label,
            "to": to,
            "entity": item.entity,
            "kind": if item.page.is_some() { "page" } else { "list" },
        }));
    }
    if let Some(name) = &default_dashboard {
        if let Some(dash) = dashboards.iter().find(|d| d["name"] == *name) {
            shortcuts.push(json!({
                "label": dash["label"],
                "to": "/",
                "kind": "dashboard",
            }));
        }
    }
    for report in &reports {
        shortcuts.push(json!({
            "label": report["label"],
            "to": "/reports",
            "kind": "report",
        }));
    }
    json!({
        "navigation": navigation,
        "shortcuts": shortcuts,
        "default_dashboard": default_dashboard,
        "dashboards": dashboards,
        "pages": pages,
        "reports": reports,
    })
}

async fn get_dashboard(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let dash = state
        .dashboards_live()
        .into_iter()
        .find(|d| d.name == name)
        .ok_or_else(|| QefroError::not_found(format!("dashboard '{name}' not found")))?;
    if !ctx.allows_app(dash.module.as_deref()) {
        return Err(QefroError::not_found(format!("dashboard '{name}' not found")).into());
    }
    let extra: Vec<qefro_core::ui::DashboardFilter> = params
        .into_iter()
        .filter(|(k, v)| !k.is_empty() && !v.is_empty() && !["name"].contains(&k.as_str()))
        .map(|(field, value)| qefro_core::ui::DashboardFilter { field, value })
        .collect();
    let mut cards = Vec::new();
    for card in &dash.cards {
        let mut card = card.clone();
        if !card.roles.is_empty() && !ctx.is_admin() && !card.roles.iter().any(|r| ctx.has_role(r))
        {
            continue;
        }
        if let Some(view_name) = card.saved_view.clone() {
            match state
                .saved_filters
                .list(ctx.tenant_id, ctx.user_id, &card.entity)
                .await
            {
                Ok(items) => {
                    if let Some(view) = items.into_iter().find(|v| v.name == view_name) {
                        apply_saved_query(&mut card, &view.query);
                    } else {
                        continue;
                    }
                }
                Err(_) => continue,
            }
        }
        for extra_f in &extra {
            let base = extra_f
                .field
                .split_once('.')
                .map(|(f, _)| f)
                .unwrap_or(&extra_f.field);
            card.filters.retain(|f| {
                let fb = f.field.split_once('.').map(|(n, _)| n).unwrap_or(&f.field);
                fb != base
            });
            card.filters.push(extra_f.clone());
        }
        if let Some(report_name) = card.report.clone() {
            let report = state
                .reports_live()
                .into_iter()
                .find(|r| r.name == report_name);
            let Some(report) = report else {
                continue;
            };
            match state.entities.run_report(&ctx, &report, json!([])).await {
                Ok(mut value) => {
                    if let Some(obj) = value.as_object_mut() {
                        obj.insert("title".into(), json!(card.title));
                        obj.insert("kind".into(), json!("report"));
                        obj.insert("size".into(), json!(card.size));
                        obj.insert("entity".into(), json!(card.entity));
                    }
                    cards.push(value);
                }
                Err(err) if skippable_card_error(&err) => continue,
                Err(err) => return Err(err.into()),
            }
            continue;
        }
        match state.entities.dashboard_card_value(&ctx, &card).await {
            Ok(value) => cards.push(value),
            Err(err) if skippable_card_error(&err) => continue,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(Json(json!({
        "name": dash.name,
        "label": dash.label,
        "module": dash.module,
        "cards": cards,
    })))
}

fn skippable_card_error(err: &QefroError) -> bool {
    matches!(
        err,
        QefroError::Forbidden { .. }
            | QefroError::NotFound { .. }
            | QefroError::AppNotEnabled { .. }
    )
}

fn apply_saved_query(card: &mut qefro_core::DashboardCard, query: &Value) {
    let Some(obj) = query.as_object() else {
        return;
    };
    for (key, value) in obj {
        if matches!(
            key.as_str(),
            "sort" | "view" | "page" | "page_size" | "columns"
        ) {
            continue;
        }
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        card.filters.retain(|f| f.field != *key);
        card.filters.push(qefro_core::ui::DashboardFilter {
            field: key.clone(),
            value: text,
        });
    }
}

async fn meta_reports(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    let reports: Vec<_> = state
        .reports_live()
        .into_iter()
        .filter(|r| ctx.allows_app(r.module.as_deref()))
        .collect();
    Json(json!({ "reports": reports }))
}

async fn get_report(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let report = state
        .reports_live()
        .into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| QefroError::not_found(format!("report '{name}' not found")))?;
    if !ctx.allows_app(report.module.as_deref()) {
        return Err(QefroError::not_found(format!("report '{name}' not found")).into());
    }
    Ok(Json(serde_json::to_value(report).unwrap_or(json!({}))))
}

async fn run_report(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let report = state
        .reports_live()
        .into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| QefroError::not_found(format!("report '{name}' not found")))?;
    if !ctx.allows_app(report.module.as_deref()) {
        return Err(QefroError::not_found(format!("report '{name}' not found")).into());
    }
    let filters = body.get("filters").cloned().unwrap_or(json!([]));
    Ok(Json(
        state.entities.run_report(&ctx, &report, filters).await?,
    ))
}

async fn entity_aggregates(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    reject_reserved(&slug)?;
    let entity = state.entities.registry().get(&slug)?;
    let group_by = params
        .get("group_by")
        .cloned()
        .ok_or_else(|| QefroError::bad_request("group_by is required"))?;
    let metric = params
        .get("metric")
        .or_else(|| params.get("aggregation"))
        .cloned()
        .unwrap_or_else(|| "count".into());
    let field = params.get("field").cloned();
    let raw: Vec<(String, String)> = params
        .into_iter()
        .filter(|(k, _)| !matches!(k.as_str(), "group_by" | "metric" | "aggregation" | "field"))
        .collect();
    let query = parse_query(&entity, &raw)?;
    Ok(Json(
        state
            .entities
            .entity_aggregates(
                &ctx,
                &entity.name,
                &group_by,
                &metric,
                field.as_deref(),
                query,
            )
            .await?,
    ))
}

async fn print_document(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<axum::response::Html<String>, ApiError> {
    reject_reserved(&slug)?;
    let format_name = params.get("format").map(|s| s.as_str());
    let (format, record, items) = state
        .entities
        .print_document(&ctx, &slug, id, format_name)
        .await?;
    let entity = state.entities.registry().get(&slug)?;
    let mut config = state.tenants.get_config(ctx.tenant_id).await?;
    let tenant = state.tenants.get(ctx.tenant_id).await.ok();
    config.branding =
        state.resolve_branding(&ctx, &config, tenant.as_ref().map(|t| t.name.as_str()));
    let html = qefro_db::print::render_html(&entity, &format, &record, &items, &config);
    Ok(axum::response::Html(html))
}

async fn print_document_pdf(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, ApiError> {
    reject_reserved(&slug)?;
    let format_name = params.get("format").map(|s| s.as_str());
    let (_format, record, items) = state
        .entities
        .print_document(&ctx, &slug, id, format_name)
        .await?;
    let entity = state.entities.registry().get(&slug)?;
    let lines = qefro_db::print::pdf_lines(&entity, &record, &items);
    let bytes = qefro_db::print::render_pdf(&entity.label, &lines);
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "inline; filename=\"document.pdf\"",
            ),
        ],
        bytes,
    ))
}

async fn get_tenant_config(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let mut config = state.tenants.get_config(ctx.tenant_id).await?;
    let tenant = state.tenants.get(ctx.tenant_id).await.ok();
    config.branding =
        state.resolve_branding(&ctx, &config, tenant.as_ref().map(|t| t.name.as_str()));
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
    let branding = state.resolve_branding(&ctx, &config, Some(tenant.name.as_str()));
    Ok(Json(json!({
        "id": tenant.id,
        "name": tenant.name,
        "slug": tenant.slug,
        "created_at": tenant.created_at,
        "branding": branding,
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
    let tenant = state.tenants.get(ctx.tenant_id).await.ok();
    let branding = state.resolve_branding(&ctx, &config, tenant.as_ref().map(|t| t.name.as_str()));
    Ok(Json(serde_json::to_value(branding).unwrap_or(json!({}))))
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
    Ok(Json(
        serde_json::to_value(config.branding).unwrap_or(json!({})),
    ))
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
    state
        .blobs
        .insert(ctx.tenant_id, ctx.user_id, &meta)
        .await?;
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
            "application/pdf" | "application/json" | "application/octet-stream" | "application/zip"
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
    ensure_entity_list(&state, &ctx, entity)?;
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
    ensure_entity_list(&state, &ctx, &body.entity)?;
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

fn ensure_entity_app(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    name: &str,
) -> Result<(), ApiError> {
    let entity = state.entities.registry().get(name)?;
    if !ctx.allows_app(entity.module.as_deref()) {
        return Err(QefroError::not_found(format!("entity '{name}' not found")).into());
    }
    Ok(())
}

fn ensure_entity_list(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    name: &str,
) -> Result<(), ApiError> {
    ensure_entity_app(state, ctx, name)?;
    state
        .entities
        .permissions()
        .check(ctx, name, Action::List)?;
    Ok(())
}
