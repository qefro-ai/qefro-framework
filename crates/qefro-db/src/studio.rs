//! Studio metadata change service: draft, validate, diff, publish, audit.
//!
//! Publishes update the live registries (the same ones `EntityService` uses)
//! and optionally write YAML source. Studio never issues ad-hoc DML against
//! business tables.

use crate::audit::AuditLogger;
use crate::schema::apply_schema;
use chrono::{DateTime, Utc};
use qefro_core::studio::{apply_field_ui_patch, is_production, validate_formula_on_entity};
use qefro_core::ui::DashboardDef;
use qefro_core::{
    classify_entity_change, detect_cycles, entity_referrers, find_app_root, snake_case,
    ChangeAnalysis, EntityDef, EntityRegistry, EntityViews, FieldDef, FieldUiPatch, OpContext,
    PageDef, PrintFormat, QefroError, QefroResult, ReportDef, SchemaImpact, StudioCatalog,
};
use qefro_permissions::{PermissionGrant, PermissionRegistry};
use qefro_workflow::WorkflowDef;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StudioDraft {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub target: String,
    pub payload: Value,
    pub status: String,
    pub summary: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StudioVersion {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub target: String,
    pub version: i32,
    pub payload: Value,
    pub summary: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftRequest {
    pub kind: String,
    pub target: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishRequest {
    pub draft_id: Option<Uuid>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub confirm_migration: bool,
    #[serde(default)]
    pub summary: String,
}

pub struct MetadataChangeService {
    pool: PgPool,
    registry: Arc<EntityRegistry>,
    workflows: Arc<qefro_workflow::WorkflowRegistry>,
    permissions: Arc<PermissionRegistry>,
    catalog: Arc<StudioCatalog>,
    audit: AuditLogger,
    env: String,
}

impl MetadataChangeService {
    pub fn new(
        pool: PgPool,
        registry: Arc<EntityRegistry>,
        workflows: Arc<qefro_workflow::WorkflowRegistry>,
        permissions: Arc<PermissionRegistry>,
        catalog: Arc<StudioCatalog>,
        env: String,
    ) -> Self {
        Self {
            audit: AuditLogger::new(pool.clone()),
            pool,
            registry,
            workflows,
            permissions,
            catalog,
            env,
        }
    }

    pub fn env(&self) -> &str {
        &self.env
    }

    pub fn catalog(&self) -> &StudioCatalog {
        &self.catalog
    }

    pub async fn create_draft(
        &self,
        ctx: &OpContext,
        req: DraftRequest,
    ) -> QefroResult<StudioDraft> {
        let analysis = self.analyze(&req.kind, &req.target, &req.payload)?;
        let id = Uuid::new_v4();
        let summary = if req.summary.is_empty() {
            analysis
                .diff
                .first()
                .cloned()
                .unwrap_or_else(|| req.kind.clone())
        } else {
            req.summary
        };
        sqlx::query(
            r#"
            INSERT INTO qefro_studio_drafts (
                id, tenant_id, kind, target, payload, status, summary, created_by, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,'draft',$6,$7, now(), now())
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .bind(&req.kind)
        .bind(&req.target)
        .bind(&req.payload)
        .bind(&summary)
        .bind(ctx.user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        self.get_draft(ctx, id).await
    }

    pub async fn get_draft(&self, ctx: &OpContext, id: Uuid) -> QefroResult<StudioDraft> {
        sqlx::query_as::<_, StudioDraft>(
            r#"
            SELECT id, tenant_id, kind, target, payload, status, summary, created_by, created_at, updated_at
            FROM qefro_studio_drafts
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(ctx.tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("studio draft not found"))
    }

    pub async fn list_drafts(&self, ctx: &OpContext) -> QefroResult<Vec<StudioDraft>> {
        sqlx::query_as::<_, StudioDraft>(
            r#"
            SELECT id, tenant_id, kind, target, payload, status, summary, created_by, created_at, updated_at
            FROM qefro_studio_drafts
            WHERE tenant_id = $1
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind(ctx.tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub async fn list_versions(
        &self,
        ctx: &OpContext,
        kind: &str,
        target: &str,
    ) -> QefroResult<Vec<StudioVersion>> {
        sqlx::query_as::<_, StudioVersion>(
            r#"
            SELECT id, tenant_id, kind, target, version, payload, summary, created_by, created_at
            FROM qefro_studio_versions
            WHERE tenant_id = $1 AND kind = $2 AND target = $3
            ORDER BY version DESC
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(kind)
        .bind(target)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))
    }

    pub fn analyze(
        &self,
        kind: &str,
        target: &str,
        payload: &Value,
    ) -> QefroResult<ChangeAnalysis> {
        match kind {
            "entity" | "entity.replace" => {
                let after: EntityDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(format!("invalid entity: {e}")))?;
                let before = self
                    .registry
                    .get(target)
                    .map(|e| (*e).clone())
                    .unwrap_or_else(|_| EntityDef::new(target).build());
                Ok(classify_entity_change(&before, &after))
            }
            "entity.field" | "entity.field.upsert" => {
                let mut before = (*self.registry.get(target)?).clone();
                let after = apply_field_payload(&mut before, payload)?;
                Ok(classify_entity_change(
                    &self.registry.get(target).map(|e| (*e).clone())?,
                    &after,
                ))
            }
            "entity.field.ui" => {
                let mut analysis = ChangeAnalysis::safe();
                if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
                    analysis
                        .diff
                        .push(format!("~ {target}.{name} presentation"));
                }
                Ok(analysis)
            }
            "entity.views" => analyze_views_overlay(target, payload),
            "workflow" => {
                let wf: WorkflowDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(format!("invalid workflow: {e}")))?;
                wf.validate()?;
                let mut analysis = ChangeAnalysis::safe();
                analysis.diff.push(format!("~ workflow {}", wf.name));
                Ok(analysis)
            }
            "permissions" => {
                let mut analysis = ChangeAnalysis::safe();
                analysis.diff.push(format!("~ permissions {target}"));
                Ok(analysis)
            }
            "report" | "dashboard" | "page" | "print_format" => {
                let mut analysis = ChangeAnalysis::safe();
                analysis.diff.push(format!("~ {kind} {target}"));
                Ok(analysis)
            }
            other => Err(QefroError::bad_request(format!(
                "unknown Studio change kind '{other}'"
            ))),
        }
    }

    pub fn validate_payload(
        &self,
        kind: &str,
        target: &str,
        payload: &Value,
    ) -> QefroResult<ChangeAnalysis> {
        let analysis = self.analyze(kind, target, payload)?;
        match kind {
            "entity" | "entity.replace" => {
                let after: EntityDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                validate_entity(self.registry.as_ref(), &after)?;
            }
            "entity.field" | "entity.field.upsert" | "entity.field.ui" => {
                let mut entity = (*self.registry.get(target)?).clone();
                let after = apply_field_payload(&mut entity, payload)?;
                validate_entity(self.registry.as_ref(), &after)?;
            }
            "entity.views" => {
                let mut entity = (*self.registry.get(target)?).clone();
                let after = apply_views_payload(&mut entity, payload)?;
                validate_entity(self.registry.as_ref(), &after)?;
            }
            "workflow" => {
                let wf: WorkflowDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                wf.validate()?;
            }
            "permissions" => {
                let grants: Vec<PermissionGrant> = serde_json::from_value(payload.clone())
                    .or_else(|_| {
                        payload
                            .get("grants")
                            .cloned()
                            .ok_or_else(|| QefroError::bad_request("permissions require grants"))
                            .and_then(|v| {
                                serde_json::from_value(v)
                                    .map_err(|e| QefroError::bad_request(e.to_string()))
                            })
                    })?;
                for grant in &grants {
                    if self.registry.try_get(&grant.entity).is_none() && grant.entity != "*" {
                        return Err(QefroError::bad_request(format!(
                            "permission references unknown entity '{}'",
                            grant.entity
                        )));
                    }
                }
            }
            "report" => {
                let report: ReportDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                let entity = self.registry.get(&report.entity)?;
                for field in report.fields.iter().chain(report.group_by.iter()) {
                    if entity.get_field(field).is_none() {
                        return Err(QefroError::bad_request(format!(
                            "report field '{field}' is not on {}",
                            report.entity
                        )));
                    }
                }
            }
            "dashboard" => {
                let dash: DashboardDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                for card in &dash.cards {
                    if self.registry.try_get(&card.entity).is_none() {
                        return Err(QefroError::bad_request(format!(
                            "dashboard card '{}' references unknown entity '{}'",
                            card.title, card.entity
                        )));
                    }
                }
            }
            "page" => {
                qefro_core::reject_unsafe_page_payload(payload)?;
                let mut page: PageDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                page.normalize();
                let reports = self.catalog.merge_reports(&[]);
                let dashboards = self.catalog.merge_dashboards(&[]);
                let slugs: Vec<String> = self
                    .registry
                    .list()
                    .into_iter()
                    .map(|e| e.slug.clone())
                    .collect();
                let errors = qefro_core::validate_page(
                    &page,
                    self.registry.as_ref(),
                    &reports,
                    &dashboards,
                    &slugs,
                );
                if let Some(err) = errors.into_iter().next() {
                    return Err(QefroError::bad_request(err));
                }
            }
            "print_format" => {
                let pf: PrintFormat = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                self.registry.get(&pf.entity)?;
            }
            _ => {}
        }
        Ok(analysis)
    }

    pub async fn publish(&self, ctx: &OpContext, req: PublishRequest) -> QefroResult<Value> {
        let (kind, target, payload, summary, draft_id) = if let Some(id) = req.draft_id {
            let draft = self.get_draft(ctx, id).await?;
            (
                draft.kind,
                draft.target,
                draft.payload,
                draft.summary,
                Some(id),
            )
        } else {
            (req.kind, req.target, req.payload, req.summary, None)
        };
        if kind.is_empty() || target.is_empty() {
            return Err(QefroError::bad_request("kind and target are required"));
        }
        let analysis = self.validate_payload(&kind, &target, &payload)?;
        match analysis.impact {
            SchemaImpact::Destructive => {
                return Err(QefroError::bad_request(format!(
                    "⚠ Database migration required. {}",
                    analysis.warnings.join(" ")
                )));
            }
            SchemaImpact::Additive if is_production(&self.env) && !req.confirm_migration => {
                return Err(QefroError::bad_request(
                    "⚠ Database migration required. Resubmit with confirm_migration=true to add columns. Existing data is not converted.",
                ));
            }
            _ => {}
        }

        let before = self.snapshot(&kind, &target);
        self.apply(&kind, &target, &payload).await?;
        if analysis.migration_required {
            apply_schema(&self.pool, self.registry.as_ref()).await?;
        }
        let after = self.snapshot(&kind, &target);
        self.record_version(ctx, &kind, &target, &after, &summary)
            .await?;
        if let Some(id) = draft_id {
            sqlx::query(
                "UPDATE qefro_studio_drafts SET status = 'published', updated_at = now() WHERE id = $1 AND tenant_id = $2",
            )
            .bind(id)
            .bind(ctx.tenant_id)
            .execute(&self.pool)
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        }
        let action = format!("{kind}.updated");
        let _ = self
            .audit
            .record(
                ctx,
                "studio",
                None,
                &action,
                before.as_ref(),
                after.as_ref(),
            )
            .await;
        Ok(json!({
            "kind": kind,
            "target": target,
            "impact": analysis.impact.as_str(),
            "migration_required": analysis.migration_required,
            "warnings": analysis.warnings,
            "diff": analysis.diff,
            "published": true,
        }))
    }

    pub async fn rollback(
        &self,
        ctx: &OpContext,
        kind: &str,
        target: &str,
        version: i32,
    ) -> QefroResult<Value> {
        let row = sqlx::query_as::<_, StudioVersion>(
            r#"
            SELECT id, tenant_id, kind, target, version, payload, summary, created_by, created_at
            FROM qefro_studio_versions
            WHERE tenant_id = $1 AND kind = $2 AND target = $3 AND version = $4
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(kind)
        .bind(target)
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?
        .ok_or_else(|| QefroError::not_found("studio version not found"))?;
        let current = self.snapshot(kind, target).unwrap_or(json!(null));
        let analysis = self.analyze(kind, target, &row.payload)?;
        if analysis.migration_required {
            return Err(QefroError::bad_request(
                "⚠ Migration required. Studio will not roll back schema-changing metadata automatically.",
            ));
        }
        self.apply(kind, target, &row.payload).await?;
        let _ = self
            .audit
            .record(
                ctx,
                "studio",
                None,
                &format!("{kind}.rollback"),
                Some(&current),
                Some(&row.payload),
            )
            .await;
        Ok(json!({ "restored": version, "kind": kind, "target": target }))
    }

    fn snapshot(&self, kind: &str, target: &str) -> Option<Value> {
        match kind {
            "entity"
            | "entity.replace"
            | "entity.field"
            | "entity.field.upsert"
            | "entity.field.ui"
            | "entity.views" => self
                .registry
                .try_get(target)
                .and_then(|e| serde_json::to_value(&*e).ok()),
            "workflow" => self
                .workflows
                .for_entity(target)
                .and_then(|w| serde_json::to_value(w).ok()),
            "permissions" => {
                let grants: Vec<_> = self
                    .permissions
                    .grants()
                    .into_iter()
                    .filter(|g| g.entity == target)
                    .collect();
                serde_json::to_value(grants).ok()
            }
            "report" => self
                .catalog
                .report(target)
                .and_then(|r| serde_json::to_value(r).ok()),
            "dashboard" => self
                .catalog
                .dashboard(target)
                .and_then(|d| serde_json::to_value(d).ok()),
            "page" => self
                .catalog
                .page(target)
                .and_then(|p| serde_json::to_value(p).ok()),
            "print_format" => self
                .catalog
                .print_format(target)
                .and_then(|p| serde_json::to_value(p).ok()),
            _ => None,
        }
    }

    async fn apply(&self, kind: &str, target: &str, payload: &Value) -> QefroResult<()> {
        match kind {
            "entity" | "entity.replace" => {
                let def: EntityDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                validate_entity(self.registry.as_ref(), &def)?;
                let module = def.module.clone();
                self.registry.overlay_put(def.clone())?;
                maybe_write_yaml(module.as_deref(), &def)?;
            }
            "entity.field" | "entity.field.upsert" | "entity.field.ui" => {
                let mut entity = (*self.registry.get(target)?).clone();
                let after = apply_field_payload(&mut entity, payload)?;
                validate_entity(self.registry.as_ref(), &after)?;
                let module = after.module.clone();
                self.registry.overlay_put(after.clone())?;
                maybe_write_yaml(module.as_deref(), &after)?;
            }
            "entity.views" => {
                let mut entity = (*self.registry.get(target)?).clone();
                let after = apply_views_payload(&mut entity, payload)?;
                validate_entity(self.registry.as_ref(), &after)?;
                let module = after.module.clone();
                self.registry.overlay_put(after.clone())?;
                maybe_write_yaml(module.as_deref(), &after)?;
            }
            "workflow" => {
                let wf: WorkflowDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                wf.validate()?;
                self.workflows.overlay_put(wf);
            }
            "permissions" => {
                let grants: Vec<PermissionGrant> = serde_json::from_value(payload.clone())
                    .or_else(|_| {
                        payload
                            .get("grants")
                            .cloned()
                            .ok_or_else(|| QefroError::bad_request("permissions require grants"))
                            .and_then(|v| {
                                serde_json::from_value(v)
                                    .map_err(|e| QefroError::bad_request(e.to_string()))
                            })
                    })?;
                self.permissions.overlay_entity(target, grants);
            }
            "report" => {
                let report: ReportDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                self.catalog.upsert_report(report);
            }
            "dashboard" => {
                let dash: DashboardDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                self.catalog.upsert_dashboard(dash);
            }
            "page" => {
                qefro_core::reject_unsafe_page_payload(payload)?;
                let mut page: PageDef = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                page.normalize();
                self.catalog.upsert_page(page);
            }
            "print_format" => {
                let pf: PrintFormat = serde_json::from_value(payload.clone())
                    .map_err(|e| QefroError::bad_request(e.to_string()))?;
                self.catalog.upsert_print_format(pf);
            }
            other => {
                return Err(QefroError::bad_request(format!(
                    "cannot publish kind '{other}'"
                )))
            }
        }
        Ok(())
    }

    async fn record_version(
        &self,
        ctx: &OpContext,
        kind: &str,
        target: &str,
        payload: &Option<Value>,
        summary: &str,
    ) -> QefroResult<()> {
        let Some(payload) = payload else {
            return Ok(());
        };
        let next: (i32,) = sqlx::query_as(
            r#"
            SELECT COALESCE(MAX(version), 0) + 1
            FROM qefro_studio_versions
            WHERE tenant_id = $1 AND kind = $2 AND target = $3
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(kind)
        .bind(target)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO qefro_studio_versions (
                id, tenant_id, kind, target, version, payload, summary, created_by, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8, now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctx.tenant_id)
        .bind(kind)
        .bind(target)
        .bind(next.0)
        .bind(payload)
        .bind(summary)
        .bind(ctx.user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
        Ok(())
    }
}

fn apply_field_payload(entity: &mut EntityDef, payload: &Value) -> QefroResult<EntityDef> {
    if let Ok(field) = serde_json::from_value::<FieldDef>(payload.clone()) {
        upsert_field(entity, field);
        return Ok(entity.clone());
    }
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| QefroError::bad_request("field patch requires name"))?;
    if let Some(ty) = payload.get("type") {
        if let Ok(new_ty) = serde_json::from_value::<qefro_core::FieldType>(json!({ "type": ty })) {
            if let Some(existing) = entity.get_field(name) {
                if std::mem::discriminant(&existing.field_type) != std::mem::discriminant(&new_ty)
                    || existing.field_type.as_str() != new_ty.as_str()
                {
                    let mut field = existing.clone();
                    field.field_type = new_ty;
                    upsert_field(entity, field);
                    return Ok(entity.clone());
                }
            }
        }
    }
    if payload.get("type").is_some() && entity.get_field(name).is_none() {
        let field: FieldDef = serde_json::from_value(payload.clone())
            .map_err(|e| QefroError::bad_request(format!("invalid field: {e}")))?;
        upsert_field(entity, field);
        return Ok(entity.clone());
    }
    let patch: FieldUiPatch = serde_json::from_value(payload.clone())
        .map_err(|e| QefroError::bad_request(format!("invalid field patch: {e}")))?;
    if let Some(formula) = &patch.formula {
        if !formula.is_empty() {
            validate_formula_on_entity(entity, name, formula)?;
        }
    }
    let Some(field) = entity.fields.iter_mut().find(|f| f.name == name) else {
        return Err(QefroError::not_found(format!(
            "field '{name}' not found on {}",
            entity.name
        )));
    };
    apply_field_ui_patch(field, &patch);
    if let Some(rel) = payload.get("relation") {
        if let Some(target) = rel.get("target_entity").and_then(|v| v.as_str()) {
            if let Some(existing) = field.relation.as_mut() {
                existing.target_entity = target.to_string();
            }
        }
        if let Some(display) = rel
            .get("display_field")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("display_field").and_then(|v| v.as_str()))
        {
            field.ui.widget_options.display_field = Some(display.to_string());
        }
        if let Some(search) = rel
            .get("search_fields")
            .and_then(|v| v.as_array())
            .or_else(|| payload.get("search_fields").and_then(|v| v.as_array()))
        {
            field.ui.widget_options.search_fields = Some(
                search
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
            );
        }
    }
    Ok(entity.clone())
}

const VIEW_OVERLAY_KEYS: &[&str] = &["list", "card", "form", "detail", "kanban", "calendar"];
const VIEW_OVERLAY_REJECT: &[&str] = &[
    "permissions",
    "workflow",
    "fields",
    "name",
    "table",
    "slug",
    "type",
    "actions",
    "operations",
    "module",
];

fn analyze_views_overlay(target: &str, payload: &Value) -> QefroResult<ChangeAnalysis> {
    reject_non_presentation_view_keys(payload)?;
    let mut analysis = ChangeAnalysis::safe();
    analysis.diff.push(format!("~ {target} views"));
    Ok(analysis)
}

fn reject_non_presentation_view_keys(payload: &Value) -> QefroResult<()> {
    let obj = payload
        .as_object()
        .ok_or_else(|| QefroError::bad_request("entity.views payload must be an object"))?;
    for key in obj.keys() {
        if VIEW_OVERLAY_REJECT.iter().any(|k| k == key) {
            return Err(QefroError::bad_request(format!(
                "entity.views rejects non-presentation key '{key}'"
            )));
        }
        if !VIEW_OVERLAY_KEYS.iter().any(|k| k == key) {
            return Err(QefroError::bad_request(format!(
                "entity.views unknown presentation key '{key}'"
            )));
        }
    }
    Ok(())
}

fn apply_views_payload(entity: &mut EntityDef, payload: &Value) -> QefroResult<EntityDef> {
    reject_non_presentation_view_keys(payload)?;
    let patch: EntityViews = serde_json::from_value(payload.clone())
        .map_err(|e| QefroError::bad_request(format!("invalid views overlay: {e}")))?;
    let mut views = entity.views.clone().unwrap_or_default();
    if payload.get("list").is_some() {
        views.list = patch.list;
    }
    if payload.get("card").is_some() {
        views.card = patch.card;
    }
    if payload.get("form").is_some() {
        views.form = patch.form;
    }
    if payload.get("detail").is_some() {
        views.detail = patch.detail;
    }
    if payload.get("kanban").is_some() {
        views.kanban = patch.kanban;
    }
    if payload.get("calendar").is_some() {
        views.calendar = patch.calendar;
    }
    entity.views = Some(views);
    Ok(entity.clone())
}

fn upsert_field(entity: &mut EntityDef, field: FieldDef) {
    if let Some(existing) = entity.fields.iter_mut().find(|f| f.name == field.name) {
        *existing = field;
    } else {
        entity.fields.push(field);
    }
    entity.normalize();
}

fn validate_entity(registry: &EntityRegistry, after: &EntityDef) -> QefroResult<()> {
    after.validate_idents()?;
    detect_cycles(&after.fields)?;
    after.validate_ui_layout()?;
    for field in &after.fields {
        if let Some(rel) = &field.relation {
            if rel.target_entity != after.name && registry.try_get(&rel.target_entity).is_none() {
                return Err(QefroError::bad_request(format!(
                    "relation '{}' references unavailable entity '{}'",
                    field.name, rel.target_entity
                )));
            }
        }
        if field.computed {
            if let Some(formula) = &field.formula {
                validate_formula_on_entity(after, &field.name, formula)?;
            } else {
                return Err(QefroError::bad_request(format!(
                    "computed field '{}.{}' is missing a formula",
                    after.name, field.name
                )));
            }
        }
    }
    let _ = entity_referrers(registry, &after.name);
    Ok(())
}

fn maybe_write_yaml(module: Option<&str>, def: &EntityDef) -> QefroResult<()> {
    let Some(app) = module else {
        return Ok(());
    };
    let Some(root) = find_app_root(app) else {
        return Ok(());
    };
    let dir = root.join("entities");
    if !dir.is_dir() {
        return Ok(());
    }
    let path = dir.join(format!("{}.yaml", snake_case(&def.name)));
    let yaml = serde_yaml::to_string(def)
        .map_err(|e| QefroError::internal(format!("yaml serialize: {e}")))?;
    std::fs::write(&path, yaml)
        .map_err(|e| QefroError::internal(format!("write {}: {e}", path.display())))?;
    Ok(())
}

pub fn to_yaml(value: &impl Serialize) -> QefroResult<String> {
    serde_yaml::to_string(value).map_err(|e| QefroError::internal(format!("yaml: {e}")))
}
