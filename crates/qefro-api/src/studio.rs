//! Qefro Studio HTTP API. Reads and publishes through the runtime registries.

use crate::error::ApiError;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use qefro_core::{
    entity_referrers, preview_formula, require_studio_cap, studio_capabilities, AppManifest,
    QefroError, CAP_EDIT, CAP_MANAGE_APPS, CAP_MANAGE_PERMISSIONS, CAP_MANAGE_WORKFLOWS,
    CAP_PUBLISH, CAP_VIEW, FORMULA_FUNCTIONS, FRAMEWORK_VERSION,
};
use qefro_db::app_registry;
use qefro_db::{to_yaml, DraftRequest, PublishRequest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/overview", get(overview))
        .route("/capabilities", get(caps))
        .route("/search", get(search))
        .route("/apps", get(list_apps))
        .route("/apps/{app}", get(get_app))
        .route("/apps/{app}/disable", post(disable_app))
        .route("/apps/{app}/enable", post(enable_app))
        .route("/apps/{app}/uninstall", post(uninstall_app))
        .route("/entities", get(list_entities))
        .route("/entities/{entity}", get(get_entity))
        .route("/workflows/{entity}", get(get_workflow))
        .route("/permissions/{entity}", get(get_permissions))
        .route("/operations/{entity}", get(get_operations))
        .route("/reports", get(list_reports))
        .route("/reports/{report}", get(get_report))
        .route("/dashboards", get(list_dashboards))
        .route("/dashboards/{dashboard}", get(get_dashboard))
        .route("/pages", get(list_pages))
        .route("/pages/{page}", get(get_page))
        .route("/print-formats", get(list_print_formats))
        .route("/print-formats/{format}", get(get_print_format))
        .route("/print-formats/{format}/preview", get(preview_print_format))
        .route("/communications", get(list_communications_studio))
        .route("/communications/{name}", get(get_communication))
        .route("/communications/{name}/preview", get(preview_communication))
        .route("/tenant", get(tenant_studio))
        .route("/notifications", get(list_notifications))
        .route("/webhooks", get(list_webhooks))
        .route("/automations", get(list_automations))
        .route("/automations/runs", get(list_automation_runs))
        .route("/automations/{name}", get(get_automation))
        .route("/automations/{name}/preview", post(preview_automation))
        .route("/automations/{name}/runs", get(list_named_automation_runs))
        .route("/automations/{name}/disable", post(disable_automation))
        .route("/automations/{name}/enable", post(enable_automation))
        .route("/public-forms", get(list_public_forms))
        .route("/drafts", get(list_drafts).post(create_draft))
        .route("/drafts/{id}", get(get_draft))
        .route("/validate", post(validate_change))
        .route("/publish", post(publish_change))
        .route("/versions", get(list_versions))
        .route("/rollback", post(rollback_change))
        .route("/formula/preview", post(formula_preview))
}

fn require(ctx: &qefro_core::OpContext, env: &str, cap: &str) -> Result<(), ApiError> {
    require_studio_cap(&ctx.roles, env, cap).map_err(Into::into)
}

async fn caps(State(state): State<AppState>, Auth(ctx): Auth) -> Json<Value> {
    Json(json!({
        "env": state.env,
        "production": qefro_core::studio::is_production(&state.env),
        "capabilities": studio_capabilities(&ctx.roles, &state.env),
        "roles": ctx.roles,
        "platform": ctx.has_role(qefro_core::studio::ROLE_PLATFORM_ADMIN),
    }))
}

async fn overview(State(state): State<AppState>, Auth(ctx): Auth) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let installed = qefro_core::load_installed();
    let entities = state.entities.registry().list();
    let workflows = state.entities.workflows().list();
    let mut warnings = Vec::new();
    let mut apps = Vec::new();
    for module in &state.modules {
        let disabled = installed.is_disabled(&module.name);
        if disabled {
            warnings.push(json!({ "kind": "disabled_app", "app": module.name }));
        }
        let missing: Vec<_> = module
            .dependencies
            .keys()
            .filter(|dep| {
                !qefro_core::is_framework_dep(dep) && !state.modules.iter().any(|m| m.name == **dep)
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            warnings.push(json!({
                "kind": "missing_dependency",
                "app": module.name,
                "missing": missing,
            }));
        }
        apps.push(json!({
            "name": module.name,
            "label": module.label,
            "version": module.version,
            "source": module.source,
            "disabled": disabled,
        }));
    }
    let recent = state
        .entities
        .audit()
        .list(&ctx, Some("studio"), None, 10)
        .await?;
    Ok(Json(json!({
        "installed_apps": apps.len(),
        "entities": entities.len(),
        "workflows": workflows.len(),
        "reports": state.reports_live().len(),
        "dashboards": state.dashboards_live().len(),
        "pages": state.pages_live().len(),
        "print_formats": state.print_formats_live().len(),
        "communications": state.communications_live().len(),
        "automations": state.automation.defs().len(),
        "apps": apps,
        "warnings": warnings,
        "recent_changes": recent,
        "env": state.env,
    })))
}

async fn list_apps(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let installed = qefro_core::load_installed();
    let registry_rows = app_registry::list_apps(state.entities.pool())
        .await
        .unwrap_or_default();
    let apps: Vec<_> = state
        .modules
        .iter()
        .map(|m| app_json(&state, m, &installed, &registry_rows))
        .collect();
    Ok(Json(json!({ "apps": apps })))
}

async fn get_app(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(app): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let module = state
        .modules
        .iter()
        .find(|m| m.name == app)
        .ok_or_else(|| QefroError::not_found(format!("app '{app}' not found")))?;
    let installed = qefro_core::load_installed();
    let rows = app_registry::list_apps(state.entities.pool())
        .await
        .unwrap_or_default();
    let mut body = app_json(&state, module, &installed, &rows);
    let entities: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .filter(|e| e.module.as_deref() == Some(app.as_str()))
        .map(|e| json!({ "name": e.name, "slug": e.slug, "label": e.label }))
        .collect();
    let reverse: Vec<_> = state
        .modules
        .iter()
        .filter(|m| m.dependencies.contains_key(&app))
        .map(|m| m.name.clone())
        .collect();
    body["entities"] = json!(entities);
    body["workflows"] = json!(state
        .entities
        .workflows()
        .list()
        .into_iter()
        .filter(|w| state
            .entities
            .registry()
            .try_get(&w.entity)
            .and_then(|e| e.module.clone())
            == Some(app.clone()))
        .map(|w| w.name)
        .collect::<Vec<_>>());
    body["reports"] = json!(state
        .reports_live()
        .into_iter()
        .filter(|r| r.module.as_deref() == Some(app.as_str()))
        .map(|r| r.name)
        .collect::<Vec<_>>());
    body["dashboards"] = json!(state
        .dashboards_live()
        .into_iter()
        .filter(|d| d.module.as_deref() == Some(app.as_str()))
        .map(|d| d.name)
        .collect::<Vec<_>>());
    body["pages"] = json!(state
        .pages_live()
        .into_iter()
        .filter(|p| p.module.as_deref() == Some(app.as_str()))
        .map(|p| p.name)
        .collect::<Vec<_>>());
    body["print_formats"] = json!(state
        .print_formats_live()
        .into_iter()
        .filter(|p| p.module.as_deref() == Some(app.as_str()))
        .map(|p| p.name)
        .collect::<Vec<_>>());
    body["communications"] = json!(state
        .communications_live()
        .into_iter()
        .filter(|c| c.module.as_deref() == Some(app.as_str()))
        .map(|c| c.name)
        .collect::<Vec<_>>());
    body["navigation"] = json!(module.navigation);
    body["reverse_dependencies"] = json!(reverse);
    body["source_managed"] =
        json!(module.source.is_empty() || module.source == "catalog" || module.source == "rust");
    Ok(Json(body))
}

fn app_json(
    state: &AppState,
    module: &AppManifest,
    installed: &qefro_core::InstalledSet,
    rows: &[qefro_db::AppRegistryRow],
) -> Value {
    let row = rows.iter().find(|r| r.name == module.name);
    json!({
        "name": module.name,
        "label": module.label,
        "version": module.version,
        "installed_version": row.map(|r| r.version.clone()).unwrap_or_else(|| module.version.clone()),
        "source": if module.source.is_empty() { "catalog" } else { module.source.as_str() },
        "framework_version": module.framework_version,
        "framework_runtime": FRAMEWORK_VERSION,
        "dependencies": module.dependencies,
        "disabled": installed.is_disabled(&module.name),
        "status": row.map(|r| r.status.clone()).unwrap_or_else(|| {
            if installed.is_disabled(&module.name) { "disabled".into() } else { "installed".into() }
        }),
        "enabled_for_tenant": state.installed_apps.contains(&module.name),
        "description": module.description,
    })
}

async fn disable_app(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(app): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_MANAGE_APPS)?;
    let reverse: Vec<_> = state
        .modules
        .iter()
        .filter(|m| {
            m.dependencies.contains_key(&app) && !qefro_core::load_installed().is_disabled(&m.name)
        })
        .map(|m| m.name.clone())
        .collect();
    if !reverse.is_empty() {
        return Err(QefroError::bad_request(format!(
            "cannot disable '{app}': required by {}",
            reverse.join(", ")
        ))
        .into());
    }
    qefro_core::disable_app(&app)?;
    Ok(Json(json!({ "app": app, "status": "disabled" })))
}

async fn enable_app(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(app): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_MANAGE_APPS)?;
    qefro_core::enable_app(&app)?;
    Ok(Json(json!({ "app": app, "status": "installed" })))
}

async fn uninstall_app(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(app): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_MANAGE_APPS)?;
    let reverse: Vec<_> = state
        .modules
        .iter()
        .filter(|m| m.dependencies.contains_key(&app))
        .map(|m| m.name.clone())
        .collect();
    if !reverse.is_empty() {
        return Err(QefroError::bad_request(format!(
            "cannot uninstall '{app}': required by {}",
            reverse.join(", ")
        ))
        .into());
    }
    qefro_core::remove_app(&app)?;
    Ok(Json(json!({
        "app": app,
        "status": "uninstalled",
        "warning": "Application tables were not dropped."
    })))
}

async fn list_entities(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
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
                "module": e.module,
                "workflow": e.workflow,
                "child_of": e.child_of,
                "singleton": e.singleton,
                "overlay": state.entities.registry().is_overlay(&e.name),
            })
        })
        .collect();
    Ok(Json(json!({ "entities": entities })))
}

async fn get_entity(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(entity): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state.entities.registry().get(&entity)?;
    if !ctx.allows_app(def.module.as_deref()) {
        return Err(QefroError::not_found(format!("entity '{entity}' not found")).into());
    }
    let referrers = entity_referrers(state.entities.registry(), &def.name);
    let yaml = to_yaml(&*def)?;
    let json_body = serde_json::to_value(&*def).unwrap_or(json!({}));
    let source = if def
        .module
        .as_ref()
        .and_then(|m| state.modules.iter().find(|mod_| &mod_.name == m))
        .map(|m| m.source.as_str() == "yaml")
        .unwrap_or(false)
    {
        "yaml"
    } else if state.entities.registry().is_overlay(&def.name) {
        "overlay"
    } else {
        "rust"
    };
    Ok(Json(json!({
        "entity": json_body,
        "json": serde_json::to_string_pretty(&*def).unwrap_or_default(),
        "yaml": yaml,
        "source": source,
        "source_managed": source == "rust",
        "referrers": referrers,
        "overlay": state.entities.registry().is_overlay(&def.name),
        "formula_functions": FORMULA_FUNCTIONS,
        "ui": def.to_ui_meta(),
    })))
}

async fn get_workflow(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(entity): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let wf = state
        .entities
        .workflows()
        .for_entity(&entity)
        .ok_or_else(|| QefroError::not_found(format!("no workflow for {entity}")))?;
    let warnings = wf.validate().unwrap_or_default();
    Ok(Json(json!({
        "workflow": wf,
        "json": serde_json::to_string_pretty(&wf).unwrap_or_default(),
        "yaml": to_yaml(&wf)?,
        "warnings": warnings,
    })))
}

async fn get_permissions(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(entity): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    state.entities.registry().get(&entity)?;
    let grants: Vec<_> = state
        .entities
        .permissions()
        .grants()
        .into_iter()
        .filter(|g| g.entity == entity || g.entity == "*")
        .collect();
    let field_levels: Vec<_> = state
        .entities
        .permissions()
        .field_levels()
        .into_iter()
        .filter(|g| g.entity == entity || g.entity == "*")
        .collect();
    let def = state.entities.registry().get(&entity)?;
    let fields: Vec<_> = def
        .fields
        .iter()
        .filter(|f| !f.system)
        .map(|f| {
            json!({
                "name": f.name,
                "label": f.ui.label,
                "permission_level": f.permission_level,
                "allow_on_submit": f.allow_on_submit,
            })
        })
        .collect();
    Ok(Json(json!({
        "entity": entity,
        "grants": grants,
        "field_levels": field_levels,
        "fields": fields,
    })))
}

async fn get_operations(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(entity): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let ops: Vec<_> = state
        .entities
        .operations()
        .for_entity(&entity)
        .into_iter()
        .map(|b| {
            json!({
                "name": b.def.name,
                "label": b.def.label,
                "description": b.def.description,
                "roles": b.def.roles,
                "permission": b.def.permission,
                "kind": b.def.kind,
                "source_managed": true,
                "workflow_transition": b.def.workflow_transition,
                "event": b.def.event,
                "execution": b.def.execution,
                "idempotent": b.def.idempotent,
                "input_schema": b.def.input_schema,
                "requires_confirmation": b.def.requires_confirmation,
                "confirmation_message": b.def.confirmation_message,
            })
        })
        .collect();
    Ok(Json(json!({ "entity": entity, "operations": ops })))
}

async fn list_reports(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({ "reports": state.reports_live() })))
}

async fn get_report(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(report): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .reports_live()
        .into_iter()
        .find(|r| r.name == report)
        .ok_or_else(|| QefroError::not_found(format!("report '{report}' not found")))?;
    Ok(Json(json!({
        "report": def,
        "json": serde_json::to_string_pretty(&def).unwrap_or_default(),
        "yaml": to_yaml(&def)?,
    })))
}

async fn list_dashboards(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({ "dashboards": state.dashboards_live() })))
}

async fn get_dashboard(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(dashboard): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .dashboards_live()
        .into_iter()
        .find(|d| d.name == dashboard)
        .ok_or_else(|| QefroError::not_found(format!("dashboard '{dashboard}' not found")))?;
    Ok(Json(json!({
        "dashboard": def,
        "json": serde_json::to_string_pretty(&def).unwrap_or_default(),
        "yaml": to_yaml(&def)?,
    })))
}

async fn list_pages(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({ "pages": state.pages_live() })))
}

async fn get_page(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(page): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .pages_live()
        .into_iter()
        .find(|p| p.name == page || p.slug() == page)
        .ok_or_else(|| QefroError::not_found(format!("page '{page}' not found")))?;
    Ok(Json(json!({
        "page": def,
        "json": serde_json::to_string_pretty(&def).unwrap_or_default(),
        "yaml": to_yaml(&def)?,
    })))
}

async fn list_print_formats(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({ "print_formats": state.print_formats_live() })))
}

async fn get_print_format(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(format): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .print_formats_live()
        .into_iter()
        .find(|p| p.name == format)
        .ok_or_else(|| QefroError::not_found(format!("print format '{format}' not found")))?;
    Ok(Json(json!({
        "print_format": def,
        "json": serde_json::to_string_pretty(&def).unwrap_or_default(),
        "yaml": to_yaml(&def)?,
    })))
}

async fn preview_print_format(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(format): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .print_formats_live()
        .into_iter()
        .find(|p| p.name == format)
        .ok_or_else(|| QefroError::not_found(format!("print format '{format}' not found")))?;
    let entity = state.entities.registry().get(&def.entity)?;
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    let sample = sample_record(&entity);
    let items = sample_items(&entity);
    let html = qefro_db::print::render_html(&entity, &def, &sample, &items, &config);
    Ok(Json(
        json!({ "html": html, "sample": sample, "items": items }),
    ))
}

async fn list_communications_studio(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(
        json!({ "communications": state.communications_live() }),
    ))
}

async fn get_communication(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .communications_live()
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| QefroError::not_found(format!("communication '{name}' not found")))?;
    Ok(Json(json!({
        "communication": def,
        "json": serde_json::to_string_pretty(&def).unwrap_or_default(),
        "yaml": to_yaml(&def)?,
    })))
}

async fn preview_communication(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .communications_live()
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| QefroError::not_found(format!("communication '{name}' not found")))?;
    let entity = state.entities.registry().get(&def.entity)?;
    let sample = sample_record(&entity);
    let mut extras = std::collections::HashMap::new();
    if let Some(path) = &def.recipient_path {
        extras.insert(
            path.clone(),
            json!({
                "name": "Ahmed",
                "email": "ahmed@example.com",
                "phone": "+10000000000"
            }),
        );
    }
    let ctx_value = qefro_core::wrap_record(&def.entity, sample.clone(), extras);
    let opts = qefro_core::FormatOpts {
        currency: ctx.currency.clone(),
        locale: ctx.locale.clone(),
        date_format: "YYYY-MM-DD".into(),
    };
    let subject = if let Some(s) = &def.subject {
        qefro_core::render_template(s, &ctx_value, &opts).unwrap_or_default()
    } else {
        def.name.replace('_', " ")
    };
    let body = qefro_core::render_template(&def.body, &ctx_value, &opts).unwrap_or_default();
    Ok(Json(json!({
        "preview": true,
        "sent": false,
        "subject": subject,
        "body": body,
        "channel": def.channels.first().cloned().unwrap_or_else(|| "in_app".into()),
        "sample": sample,
    })))
}

fn sample_record(entity: &qefro_core::EntityDef) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("doc_no".into(), json!("INV-2026-00001"));
    map.insert("status".into(), json!("Draft"));
    if let Some(display) = entity.get_field(&entity.display_field) {
        map.insert(display.name.clone(), json!("Sample"));
    }
    for field in &entity.fields {
        if map.contains_key(&field.name) || field.system || field.is_child_table() {
            continue;
        }
        map.insert(
            field.name.clone(),
            match field.field_type.as_str() {
                "integer" | "decimal" => json!(2),
                "boolean" => json!(true),
                _ => json!("Sample"),
            },
        );
    }
    Value::Object(map)
}

fn sample_items(entity: &qefro_core::EntityDef) -> Vec<Value> {
    if entity.fields.iter().any(|f| f.is_child_table()) {
        vec![
            json!({ "name": "Pizza", "quantity": 2, "rate": 300, "amount": 600 }),
            json!({ "name": "Coke", "quantity": 2, "rate": 80, "amount": 160 }),
        ]
    } else {
        Vec::new()
    }
}

async fn tenant_studio(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let config = state.tenants.get_config(ctx.tenant_id).await?;
    Ok(Json(json!({
        "scope": "tenant",
        "tenant_id": ctx.tenant_id,
        "config": config,
        "note": "Tenant Studio cannot edit platform application metadata.",
    })))
}

async fn list_notifications(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({ "notifications": state.notification_defs })))
}

async fn list_webhooks(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
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

async fn list_automations(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let items: Vec<_> = state
        .automation
        .defs()
        .iter()
        .map(|d| d.to_studio_json())
        .collect();
    Ok(Json(json!({ "automations": items })))
}

async fn get_automation(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let defs = state.automation.defs();
    let def = defs
        .iter()
        .find(|d| d.name == name || d.id_key() == name)
        .ok_or_else(|| QefroError::not_found(format!("automation '{name}' not found")))?;
    Ok(Json(json!({
        "automation": def.to_studio_json(),
        "json": serde_json::to_string_pretty(def).unwrap_or_default(),
        "yaml": to_yaml(def)?,
    })))
}

#[derive(Debug, Deserialize)]
struct AutomationPreviewBody {
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    entity: Option<String>,
    #[serde(default)]
    record_id: Option<Uuid>,
    #[serde(default)]
    payload: Value,
}

async fn preview_automation(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
    Json(body): Json<AutomationPreviewBody>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let def = state
        .automation
        .defs()
        .into_iter()
        .find(|d| d.name == name || d.id_key() == name)
        .ok_or_else(|| QefroError::not_found(format!("automation '{name}' not found")))?;
    let event_name = body
        .event
        .or(def.trigger.event.clone())
        .unwrap_or_else(|| "entity.updated".into());
    let entity = body.entity.unwrap_or_default();
    let mut event = qefro_events::DomainEvent::new(
        event_name,
        entity,
        body.record_id.unwrap_or(Uuid::nil()),
        ctx.tenant_id,
        body.payload,
    );
    event.user_id = Some(ctx.user_id);
    let plan = state.automation.preview(&ctx, &name, &event).await?;
    Ok(Json(plan))
}

async fn list_automation_runs(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let automation_id = params.get("automation").map(|s| s.as_str());
    let entity = params.get("entity").map(|s| s.as_str());
    let record_id = params
        .get("record_id")
        .and_then(|s| Uuid::parse_str(s).ok());
    let runs = state
        .automation
        .list_runs(&ctx, automation_id, entity, record_id, 50)
        .await?;
    Ok(Json(json!({ "runs": runs })))
}

async fn list_named_automation_runs(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let runs = state
        .automation
        .list_runs(&ctx, Some(&name), None, None, 50)
        .await?;
    Ok(Json(json!({ "runs": runs })))
}

async fn disable_automation(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_PUBLISH)?;
    set_automation_enabled(&state, &ctx, &name, false).await
}

async fn enable_automation(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_PUBLISH)?;
    set_automation_enabled(&state, &ctx, &name, true).await
}

async fn set_automation_enabled(
    state: &AppState,
    ctx: &qefro_core::OpContext,
    name: &str,
    enabled: bool,
) -> Result<Json<Value>, ApiError> {
    let mut def = state
        .automation
        .defs()
        .into_iter()
        .find(|d| d.name == name || d.id_key() == name)
        .ok_or_else(|| QefroError::not_found(format!("automation '{name}' not found")))?;
    def.enabled = enabled;
    let payload = serde_json::to_value(&def).unwrap_or(json!({}));
    let result = state
        .studio
        .publish(
            ctx,
            PublishRequest {
                draft_id: None,
                kind: "automation".into(),
                target: def.name.clone(),
                payload,
                confirm_migration: false,
                summary: if enabled {
                    format!("Enable automation {}", def.name)
                } else {
                    format!("Disable automation {}", def.name)
                },
            },
        )
        .await?;
    Ok(Json(result))
}

async fn list_public_forms(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let forms: Vec<_> = state
        .entities
        .registry()
        .list()
        .into_iter()
        .filter_map(|e| {
            e.public_form.as_ref().map(|f| {
                json!({
                    "entity": e.name,
                    "slug": f.slug,
                    "enabled": f.enabled,
                    "title": f.title,
                    "fields": f.fields,
                })
            })
        })
        .collect();
    Ok(Json(json!({ "public_forms": forms })))
}

async fn search(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let q = params
        .get("q")
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if q.is_empty() {
        return Ok(Json(json!({ "results": [] })));
    }
    let mut results = Vec::new();
    for module in &state.modules {
        if module.name.to_lowercase().contains(&q) || module.label.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "app", "name": module.name, "label": module.label }));
        }
    }
    for entity in state.entities.registry().list() {
        if entity.name.to_lowercase().contains(&q) || entity.label.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "entity", "name": entity.name, "label": entity.label }));
        }
        for field in &entity.fields {
            if field.is_child_table()
                && (field.name.to_lowercase().contains(&q)
                    || field
                        .relation
                        .as_ref()
                        .map(|r| r.target_entity.to_lowercase().contains(&q))
                        .unwrap_or(false))
            {
                results.push(json!({
                    "kind": "child_table",
                    "name": field.name,
                    "entity": entity.name,
                    "target": field.relation.as_ref().map(|r| r.target_entity.clone()),
                }));
            }
        }
    }
    for wf in state.entities.workflows().list() {
        if wf.name.to_lowercase().contains(&q) || wf.entity.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "workflow", "name": wf.name, "entity": wf.entity }));
        }
    }
    for report in state.reports_live() {
        if report.name.to_lowercase().contains(&q) || report.label.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "report", "name": report.name, "label": report.label }));
        }
    }
    for dash in state.dashboards_live() {
        if dash.name.to_lowercase().contains(&q) || dash.label.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "dashboard", "name": dash.name, "label": dash.label }));
        }
    }
    for page in state.pages_live() {
        if page.name.to_lowercase().contains(&q) || page.label.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "page", "name": page.name, "label": page.label }));
        }
    }
    for pf in state.print_formats_live() {
        if pf.name.to_lowercase().contains(&q) {
            results.push(json!({ "kind": "print_format", "name": pf.name, "entity": pf.entity }));
        }
    }
    for def in state.communications_live() {
        if def.name.to_lowercase().contains(&q)
            || def.entity.to_lowercase().contains(&q)
            || def.event.to_lowercase().contains(&q)
        {
            results.push(json!({
                "kind": "communication",
                "name": def.name,
                "entity": def.entity,
                "label": def.event,
            }));
        }
    }
    for def in state.automation.defs() {
        if def.name.to_lowercase().contains(&q)
            || def.description.to_lowercase().contains(&q)
            || def
                .trigger
                .event
                .as_deref()
                .map(|e| e.to_lowercase().contains(&q))
                .unwrap_or(false)
        {
            results.push(json!({
                "kind": "automation",
                "name": def.name,
                "label": def.description,
                "module": def.module,
            }));
        }
    }
    for grant in state.entities.permissions().grants() {
        if grant.role.to_lowercase().contains(&q) && grant.entity.to_lowercase().contains(&q) {
            results.push(json!({
                "kind": "permission",
                "name": format!("{} {}", grant.entity, grant.role),
                "entity": grant.entity,
            }));
        }
    }
    Ok(Json(json!({ "results": results })))
}

async fn list_drafts(
    State(state): State<AppState>,
    Auth(ctx): Auth,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(
        json!({ "drafts": state.studio.list_drafts(&ctx).await? }),
    ))
}

async fn create_draft(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<DraftRequest>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_EDIT)?;
    cap_for_kind(&ctx, &state.env, &body.kind)?;
    let draft = state.studio.create_draft(&ctx, body).await?;
    Ok(Json(json!({ "draft": draft })))
}

async fn get_draft(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let draft = state.studio.get_draft(&ctx, id).await?;
    let analysis = state
        .studio
        .analyze(&draft.kind, &draft.target, &draft.payload)?;
    Ok(Json(json!({ "draft": draft, "preview": analysis })))
}

#[derive(Deserialize)]
struct ValidateBody {
    kind: String,
    target: String,
    #[serde(default)]
    payload: Value,
}

async fn validate_change(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<ValidateBody>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let analysis = state
        .studio
        .validate_payload(&body.kind, &body.target, &body.payload)?;
    Ok(Json(json!({
        "ok": true,
        "impact": analysis.impact.as_str(),
        "migration_required": analysis.migration_required,
        "warnings": analysis.warnings,
        "diff": analysis.diff,
    })))
}

async fn publish_change(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<PublishRequest>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_PUBLISH)?;
    let kind = if body.kind.is_empty() {
        if let Some(id) = body.draft_id {
            state.studio.get_draft(&ctx, id).await?.kind
        } else {
            body.kind.clone()
        }
    } else {
        body.kind.clone()
    };
    cap_for_kind(&ctx, &state.env, &kind)?;
    Ok(Json(state.studio.publish(&ctx, body).await?))
}

#[derive(Deserialize)]
struct VersionsQuery {
    kind: String,
    target: String,
}

async fn list_versions(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<VersionsQuery>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    Ok(Json(json!({
        "versions": state.studio.list_versions(&ctx, &q.kind, &q.target).await?
    })))
}

#[derive(Deserialize)]
struct RollbackBody {
    kind: String,
    target: String,
    version: i32,
}

async fn rollback_change(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<RollbackBody>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_PUBLISH)?;
    cap_for_kind(&ctx, &state.env, &body.kind)?;
    Ok(Json(
        state
            .studio
            .rollback(&ctx, &body.kind, &body.target, body.version)
            .await?,
    ))
}

#[derive(Deserialize)]
struct FormulaPreviewBody {
    formula: String,
    #[serde(default)]
    record: Value,
}

async fn formula_preview(
    State(state): State<AppState>,
    Auth(ctx): Auth,
    Json(body): Json<FormulaPreviewBody>,
) -> Result<Json<Value>, ApiError> {
    require(&ctx, &state.env, CAP_VIEW)?;
    let value = preview_formula(&body.formula, &body.record)?;
    Ok(Json(json!({
        "formula": body.formula,
        "record": body.record,
        "result": value,
        "preview": true,
        "authoritative": false,
    })))
}

fn cap_for_kind(ctx: &qefro_core::OpContext, env: &str, kind: &str) -> Result<(), ApiError> {
    match kind {
        "permissions" => require_studio_cap(&ctx.roles, env, CAP_MANAGE_PERMISSIONS)?,
        "workflow" => require_studio_cap(&ctx.roles, env, CAP_MANAGE_WORKFLOWS)?,
        _ => {}
    }
    Ok(())
}
