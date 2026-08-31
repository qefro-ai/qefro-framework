//! Settings, attachments, notifications, webhooks, import, search, public forms, realtime.

use crate::error::ApiError;
use crate::extract::Auth;
use crate::realtime::RealtimeHub;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use qefro_core::{OpContext, QefroError, RateLimiter, ROLE_PUBLIC};
use qefro_db::{
    signed_headers, DuplicatePolicy, ImportFormat, ImportMapping, ImportMode, ImportOptions,
};
use qefro_permissions::Action;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/settings/{slug}",
            get(get_settings).patch(patch_settings),
        )
        .route("/api/v1/search", get(global_search))
        .route("/api/v1/notifications", get(list_notifications))
        .route("/api/v1/notifications/{id}/read", post(read_notification))
        .route("/api/v1/webhooks", get(list_webhooks))
        .route("/api/v1/webhooks/{name}/deliveries", get(list_deliveries))
        .route("/api/v1/webhooks/{name}/test", post(test_webhook))
        .route(
            "/api/v1/attachments/{id}",
            get(get_attachment)
                .patch(patch_attachment)
                .delete(delete_attachment),
        )
        .route("/api/v1/attachments/{id}/replace", post(replace_attachment))
        .route("/api/v1/realtime", get(realtime))
        .route(
            "/api/v1/{slug}/{id}/attachments",
            get(list_attachments).post(upload_attachment),
        )
        .route("/api/v1/{slug}/import/preview", post(import_preview))
        .route("/api/v1/{slug}/import", post(run_import))
        .route("/api/v1/{slug}/import/upload", post(upload_import))
        .route("/api/v1/{slug}/imports", get(list_entity_imports))
        .route("/api/v1/imports", get(list_imports))
        .route("/api/v1/imports/{id}", get(get_import))
        .route("/api/v1/imports/{id}/cancel", post(cancel_import))
        .route("/api/v1/imports/{id}/retry", post(retry_import))
        .route("/api/v1/imports/{id}/errors", get(download_import_errors))
}

pub fn public_router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/public/{tenant}/{form}",
        get(public_form_meta).post(public_form_submit),
    )
}

async fn get_settings(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.entity_by_slug(&slug)?;
    Ok(Json(
        state.entities.get_singleton(&ctx, &entity.name).await?,
    ))
}

async fn patch_settings(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.entity_by_slug(&slug)?;
    Ok(Json(
        state
            .entities
            .patch_singleton(&ctx, &entity.name, body)
            .await?,
    ))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn global_search(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    let key = format!("search:{}:{}", ctx.tenant_id, ctx.user_id);
    if !state.search_limiter.allow(&key) {
        return Err(QefroError::rate_limited("search rate limit exceeded").into());
    }
    let results = state
        .entities
        .global_search_grouped(&ctx, &query.q, 10)
        .await?;
    Ok(Json(json!({
        "results": results.results,
        "groups": results.groups,
    })))
}

async fn list_notifications(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    let items = state
        .notifications
        .list(ctx.tenant_id, ctx.user_id, false)
        .await?;
    let unread = state
        .notifications
        .unread_count(ctx.tenant_id, ctx.user_id)
        .await?;
    Ok(Json(json!({ "items": items, "unread": unread })))
}

async fn read_notification(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .notifications
        .mark_read(ctx.tenant_id, ctx.user_id, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_webhooks(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("webhooks require Admin").into());
    }
    let hooks: Vec<_> = state
        .webhooks
        .iter()
        .map(|w| {
            json!({
                "name": w.name,
                "event": w.event,
                "target": w.target,
                "enabled": w.enabled,
                "module": w.module,
            })
        })
        .collect();
    Ok(Json(json!({ "webhooks": hooks })))
}

async fn list_deliveries(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("webhook deliveries require Admin").into());
    }
    let items = state.webhook_log.list(ctx.tenant_id, Some(&name)).await?;
    Ok(Json(json!({ "deliveries": items })))
}

async fn test_webhook(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    if !ctx.is_admin() {
        return Err(QefroError::forbidden("test delivery requires Admin").into());
    }
    let hook = state
        .webhooks
        .iter()
        .find(|w| w.name == name)
        .ok_or_else(|| QefroError::not_found("webhook not found"))?;
    let event_id = Uuid::new_v4();
    let body = json!({ "event": "webhook.test", "ok": true });
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let ts = chrono::Utc::now().timestamp();
    let headers = signed_headers(
        hook.secret_env.as_deref(),
        "webhook.test",
        event_id,
        ts,
        &bytes,
    );
    Ok(Json(json!({
        "webhook": hook.name,
        "target": hook.target,
        "headers": headers.into_iter().map(|(k,v)| json!({k: v})).collect::<Vec<_>>(),
        "body": body,
    })))
}

async fn list_attachments(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.entity_by_slug(&slug)?;
    let page = params.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    let page_size = params
        .get("page_size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let listed = state
        .entities
        .list_attachments_page(&ctx, &entity.name, id, &state.attachments, page, page_size)
        .await?;
    Ok(Json(listed.to_client_json()))
}

async fn read_multipart_file(
    multipart: &mut Multipart,
) -> Result<(String, String, Vec<u8>), ApiError> {
    let mut filename = String::from("file");
    let mut mime = String::from("application/octet-stream");
    let mut bytes = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| QefroError::bad_request(e.to_string()))?
    {
        filename = field.file_name().unwrap_or("file").to_string();
        mime = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        bytes = field
            .bytes()
            .await
            .map_err(|e| QefroError::bad_request(e.to_string()))?
            .to_vec();
    }
    Ok((filename, mime, bytes))
}

async fn upload_attachment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path((slug, id)): Path<(String, Uuid)>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let key = format!("upload:{}:{}", ctx.tenant_id, ctx.user_id);
    if !state.rate_limiter.allow(&key) {
        return Err(QefroError::rate_limited("upload rate limit exceeded").into());
    }
    let entity = state.entities.entity_by_slug(&slug)?;
    let (filename, mime, bytes) = read_multipart_file(&mut multipart).await?;
    let row = state
        .entities
        .create_attachment(
            &ctx,
            &entity.name,
            id,
            &filename,
            &mime,
            &bytes,
            state.blob_store.as_ref(),
            &state.attachments,
        )
        .await?;
    Ok(Json(row.to_client_json()))
}

#[derive(Deserialize)]
struct AttachmentQuery {
    #[serde(default)]
    disposition: Option<String>,
    #[serde(default)]
    inline: Option<bool>,
}

async fn get_attachment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Query(query): Query<AttachmentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (meta, bytes) = state
        .entities
        .get_attachment(&ctx, id, &state.attachments, state.blob_store.as_ref())
        .await?;
    let inline = query.inline.unwrap_or(false)
        || query
            .disposition
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case("inline"));
    let filename = meta.filename.replace(['"', '\\', '\r', '\n'], "");
    let disposition = if inline {
        format!("inline; filename=\"{filename}\"")
    } else {
        format!("attachment; filename=\"{filename}\"")
    };
    Ok((
        [
            (header::CONTENT_TYPE, meta.mime_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

#[derive(Deserialize)]
struct PatchAttachmentBody {
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

async fn patch_attachment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAttachmentBody>,
) -> Result<Json<Value>, ApiError> {
    let row = state
        .entities
        .update_attachment_meta(
            &ctx,
            id,
            body.filename.as_deref(),
            body.description.as_deref(),
            &state.attachments,
        )
        .await?;
    Ok(Json(row.to_client_json()))
}

async fn replace_attachment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let key = format!("upload:{}:{}", ctx.tenant_id, ctx.user_id);
    if !state.rate_limiter.allow(&key) {
        return Err(QefroError::rate_limited("upload rate limit exceeded").into());
    }
    let (filename, mime, bytes) = read_multipart_file(&mut multipart).await?;
    let row = state
        .entities
        .replace_attachment(
            &ctx,
            id,
            &filename,
            &mime,
            &bytes,
            &state.attachments,
            state.blob_store.as_ref(),
        )
        .await?;
    Ok(Json(row.to_client_json()))
}

async fn delete_attachment(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .entities
        .delete_attachment(&ctx, id, &state.attachments, state.blob_store.as_ref())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ImportBody {
    #[serde(default)]
    csv: String,
    #[serde(default)]
    json: Option<String>,
    #[serde(default)]
    mapping: Vec<ImportMapping>,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    duplicate_policy: Option<String>,
    #[serde(default)]
    match_field: Option<String>,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    strict: Option<bool>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    blob_key: Option<String>,
}

fn import_opts(body: &ImportBody) -> Result<ImportOptions, ApiError> {
    let format = if body.json.is_some() {
        ImportFormat::Json
    } else {
        ImportFormat::parse(body.format.as_deref())
    };
    Ok(ImportOptions {
        mapping: body.mapping.clone(),
        mode: ImportMode::parse(body.mode.as_deref())?,
        duplicate_policy: DuplicatePolicy::parse(body.duplicate_policy.as_deref())?,
        match_field: body.match_field.clone(),
        dry_run: body.dry_run.unwrap_or(false),
        batch_size: body.batch_size.unwrap_or(100),
        strict: body.strict.unwrap_or(false),
        format,
        idempotency_key: body.idempotency_key.clone(),
        filename: None,
    })
}

fn import_text(body: &ImportBody) -> Result<&str, ApiError> {
    if let Some(json) = body.json.as_deref().filter(|s| !s.is_empty()) {
        return Ok(json);
    }
    if !body.csv.is_empty() {
        return Ok(&body.csv);
    }
    Err(QefroError::bad_request("csv or json is required").into())
}

async fn import_preview(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Json(body): Json<ImportBody>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.entity_by_slug(&slug)?;
    let opts = import_opts(&body)?;
    let preview = state
        .entities
        .preview_import_source(&ctx, &entity.name, import_text(&body)?, &opts)
        .await?;
    Ok(Json(serde_json::to_value(preview).unwrap_or(json!({}))))
}

async fn run_import(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    Json(body): Json<ImportBody>,
) -> Result<Json<Value>, ApiError> {
    let key = format!("import:{}:{}", ctx.tenant_id, ctx.user_id);
    if !state.rate_limiter.allow(&key) {
        return Err(QefroError::rate_limited("import rate limit exceeded").into());
    }
    let entity = state.entities.entity_by_slug(&slug)?;
    let opts = import_opts(&body)?;
    let result = if let Some(blob_key) = body.blob_key.as_deref().filter(|s| !s.is_empty()) {
        state
            .entities
            .submit_import_blob(
                &ctx,
                &entity.name,
                blob_key,
                &opts,
                state.blob_store.as_ref(),
                Some(state.blobs.as_ref()),
                Some(state.notifications.as_ref()),
            )
            .await?
    } else {
        state
            .entities
            .run_import_source(
                &ctx,
                &entity.name,
                import_text(&body)?,
                &opts,
                Some(state.blob_store.as_ref()),
                Some(state.blobs.as_ref()),
                Some(state.notifications.as_ref()),
            )
            .await?
    };
    Ok(Json(serde_json::to_value(result).unwrap_or(json!({}))))
}

async fn upload_import(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let key = format!("import:{}:{}", ctx.tenant_id, ctx.user_id);
    if !state.rate_limiter.allow(&key) {
        return Err(QefroError::rate_limited("import rate limit exceeded").into());
    }
    let entity = state.entities.entity_by_slug(&slug)?;
    let (filename, mime, bytes) = read_multipart_file(&mut multipart).await?;
    let (blob_key, filename, format) = state
        .entities
        .store_import_file(
            &ctx,
            &entity.name,
            &filename,
            &mime,
            &bytes,
            state.blob_store.as_ref(),
            state.blobs.as_ref(),
        )
        .await?;
    let text = qefro_db::import::decode_text(&bytes)?;
    let opts = ImportOptions {
        format,
        filename: Some(filename.clone()),
        ..ImportOptions::default()
    };
    let preview = state
        .entities
        .preview_import_source(&ctx, &entity.name, &text, &opts)
        .await?;
    Ok(Json(json!({
        "blob_key": blob_key,
        "filename": filename,
        "format": format,
        "preview": preview,
    })))
}

async fn list_entity_imports(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entity = state.entities.entity_by_slug(&slug)?;
    let items = state
        .entities
        .list_import_jobs(&ctx, Some(&entity.name))
        .await?;
    Ok(Json(json!({
        "items": items.iter().map(|j| j.to_client_json()).collect::<Vec<_>>()
    })))
}

async fn list_imports(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let entity = params.get("entity").map(String::as_str);
    let items = state.entities.list_import_jobs(&ctx, entity).await?;
    Ok(Json(json!({
        "items": items.iter().map(|j| j.to_client_json()).collect::<Vec<_>>()
    })))
}

async fn get_import(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let job = state.entities.get_import_job(&ctx, id).await?;
    Ok(Json(job.to_client_json()))
}

async fn cancel_import(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let job = state.entities.cancel_import_job(&ctx, id).await?;
    Ok(Json(job.to_client_json()))
}

async fn retry_import(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let job = state.entities.retry_import_job(&ctx, id).await?;
    Ok(Json(job.to_client_json()))
}

async fn download_import_errors(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let (filename, bytes) = state
        .entities
        .import_error_report(&ctx, id, state.blob_store.as_ref())
        .await?;
    let filename = filename.replace(['"', '\\', '\r', '\n'], "");
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    ))
}

#[derive(Deserialize)]
struct RealtimeQuery {
    entity: Option<String>,
    record_id: Option<Uuid>,
}

async fn realtime(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<RealtimeQuery>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    if let Some(entity) = &q.entity {
        state
            .entities
            .permissions()
            .check(&ctx, entity, Action::Read)?;
        if let Some(id) = q.record_id {
            let _ = state.entities.get(&ctx, entity, id).await?;
        }
    } else if q.record_id.is_some() {
        return Err(QefroError::bad_request("record subscription requires entity").into());
    }
    let rx = state.realtime.subscribe();
    let tenant = ctx.tenant_id;
    let entity_filter = q.entity;
    let record_filter = q.record_id;
    let stream = BroadcastStream::new(rx).filter_map(move |msg| {
        let entity_filter = entity_filter.clone();
        async move {
            let Ok(msg) = msg else { return None };
            if msg.tenant_id != tenant {
                return None;
            }
            if let Some(entity) = &entity_filter {
                if &msg.entity != entity {
                    return None;
                }
            }
            if let Some(id) = record_filter {
                if msg.record_id != id {
                    return None;
                }
            }
            Some(Ok(Event::default()
                .json_data(msg.payload)
                .unwrap_or_else(|_| Event::default())))
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn public_form_meta(
    State(state): State<AppState>,
    Path((tenant, form)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let (ctx, entity, public) = resolve_public(&state, &tenant, &form).await?;
    let _ = ctx;
    let ui = entity.to_ui_meta();
    let fields: Vec<_> = ui
        .fields
        .into_iter()
        .filter(|f| public.allows(&f.name))
        .collect();
    Ok(Json(json!({
        "slug": public.slug,
        "title": public.title.unwrap_or(entity.label.clone()),
        "description": public.description,
        "success_message": public.success_message,
        "entity": entity.name,
        "fields": fields,
    })))
}

async fn public_form_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((tenant, form)): Path<(String, String)>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    let key = format!("public:{ip}:{tenant}:{form}");
    if !state.public_limiter.allow(&key) {
        return Err(QefroError::rate_limited("public form rate limit exceeded").into());
    }
    let (ctx, entity, public) = resolve_public(&state, &tenant, &form).await?;
    if let Some(obj) = body.as_object_mut() {
        obj.remove("tenant_id");
        let allowed: Vec<_> = obj.keys().filter(|k| !public.allows(k)).cloned().collect();
        for k in allowed {
            obj.remove(&k);
        }
    }
    let created = state.entities.create(&ctx, &entity.name, body).await?;
    let id = created.get("id").cloned().unwrap_or(json!(null));
    let mut public_record = serde_json::Map::new();
    if let Some(obj) = created.as_object() {
        for field in &public.fields {
            if let Some(v) = obj.get(field) {
                public_record.insert(field.clone(), v.clone());
            }
        }
        if let Some(v) = obj.get("id") {
            public_record.insert("id".into(), v.clone());
        }
        if let Some(naming) = &entity.naming {
            if let Some(v) = obj.get(&naming.field) {
                public_record.insert(naming.field.clone(), v.clone());
            }
        }
    }
    Ok(Json(json!({
        "ok": true,
        "message": public.success_message.unwrap_or_else(|| "Received".into()),
        "reference": id,
        "record": public_record,
    })))
}

async fn resolve_public(
    state: &AppState,
    tenant_slug: &str,
    form_slug: &str,
) -> Result<(OpContext, qefro_core::EntityDef, qefro_core::PublicFormDef), ApiError> {
    let tenant = state.tenants.get_by_slug(tenant_slug).await?;
    let entity = state
        .entities
        .registry()
        .list()
        .into_iter()
        .find(|e| {
            e.public_form
                .as_ref()
                .map(|f| f.enabled && f.slug == form_slug)
                .unwrap_or(false)
        })
        .ok_or_else(|| QefroError::not_found("public form not found"))?;
    let entity = (*entity).clone();
    let public = entity
        .public_form
        .clone()
        .ok_or_else(|| QefroError::not_found("public form not found"))?;
    let mut ctx = OpContext::public(tenant.id);
    if let Ok(config) = state.tenants.get_config(tenant.id).await {
        ctx.apply_tenant_config(&config);
    }
    ctx.roles = vec![ROLE_PUBLIC.into()];
    Ok((ctx, entity, public))
}

pub struct WebhookDeliverJob {
    pub client: reqwest::Client,
    pub log: qefro_db::WebhookLog,
}

#[async_trait::async_trait]
impl qefro_db::JobHandler for WebhookDeliverJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> qefro_core::QefroResult<()> {
        let target = payload.get("target").and_then(|v| v.as_str()).unwrap_or("");
        let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let event_id = payload
            .get("event_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::nil);
        let webhook = payload
            .get("webhook")
            .and_then(|v| v.as_str())
            .unwrap_or("webhook");
        let ts = payload
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        let body = payload.get("payload").cloned().unwrap_or(json!({}));
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        let secret_env = payload.get("secret_env").and_then(|v| v.as_str());
        let headers = signed_headers(secret_env, event, event_id, ts, &bytes);
        if target.starts_with("test://") || target.is_empty() {
            self.log
                .record(
                    ctx.tenant_id,
                    webhook,
                    event,
                    event_id,
                    target,
                    Some(200),
                    true,
                    1,
                    None,
                )
                .await?;
            return Ok(());
        }
        let mut req = self.client.post(target).body(bytes);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        match req.send().await {
            Ok(resp) => {
                let code = resp.status().as_u16() as i32;
                let ok = resp.status().is_success();
                self.log
                    .record(
                        ctx.tenant_id,
                        webhook,
                        event,
                        event_id,
                        target,
                        Some(code),
                        ok,
                        1,
                        if ok { None } else { Some("http error") },
                    )
                    .await?;
                if !ok {
                    return Err(QefroError::internal(format!("webhook {code}")));
                }
                Ok(())
            }
            Err(err) => {
                self.log
                    .record(
                        ctx.tenant_id,
                        webhook,
                        event,
                        event_id,
                        target,
                        None,
                        false,
                        1,
                        Some(&err.to_string()),
                    )
                    .await?;
                Err(QefroError::internal(err.to_string()))
            }
        }
    }
}

#[allow(dead_code)]
fn _hub(_: &RealtimeHub) {}
#[allow(dead_code)]
fn _body(_: Body) {}
