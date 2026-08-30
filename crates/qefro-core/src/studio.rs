//! Qefro Studio metadata change analysis.
//!
//! Studio inspects the same [`EntityRegistry`] the runtime uses. It does not
//! introduce a second metadata system. Schema-changing edits are classified so
//! callers can require confirmation instead of mutating production DDL.

use crate::document::{PrintFormat, ReportDef};
use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::field::{FieldDef, FieldType};
use crate::formula::{eval_formula, parse_formula, FormulaContext};
use crate::registry::EntityRegistry;
use crate::ui::{DashboardDef, WidgetOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

pub const CAP_VIEW: &str = "studio.view";
pub const CAP_EDIT: &str = "studio.edit";
pub const CAP_PUBLISH: &str = "studio.publish";
pub const CAP_MANAGE_APPS: &str = "studio.manage_apps";
pub const CAP_MANAGE_PERMISSIONS: &str = "studio.manage_permissions";
pub const CAP_MANAGE_WORKFLOWS: &str = "studio.manage_workflows";

pub const ROLE_STUDIO_VIEWER: &str = "StudioViewer";
pub const ROLE_STUDIO_EDITOR: &str = "StudioEditor";
pub const ROLE_STUDIO_PUBLISHER: &str = "StudioPublisher";
pub const ROLE_STUDIO_APP_MANAGER: &str = "StudioAppManager";
pub const ROLE_STUDIO_PERMISSION_MANAGER: &str = "StudioPermissionManager";
pub const ROLE_STUDIO_WORKFLOW_MANAGER: &str = "StudioWorkflowManager";
pub const ROLE_PLATFORM_ADMIN: &str = "PlatformAdmin";

const KNOWN_ROLES: &[&str] = &[
    "Admin",
    "Manager",
    "Staff",
    "Customer",
    "Worker",
    "Public",
    "HR",
    ROLE_STUDIO_VIEWER,
    ROLE_STUDIO_EDITOR,
    ROLE_STUDIO_PUBLISHER,
    ROLE_STUDIO_APP_MANAGER,
    ROLE_STUDIO_PERMISSION_MANAGER,
    ROLE_STUDIO_WORKFLOW_MANAGER,
    ROLE_PLATFORM_ADMIN,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaImpact {
    Safe,
    Additive,
    Destructive,
}

impl SchemaImpact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Additive => "additive",
            Self::Destructive => "destructive",
        }
    }

    pub fn merge(self, other: Self) -> Self {
        use SchemaImpact::*;
        match (self, other) {
            (Destructive, _) | (_, Destructive) => Destructive,
            (Additive, _) | (_, Additive) => Additive,
            _ => Safe,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAnalysis {
    pub impact: SchemaImpact,
    pub migration_required: bool,
    pub warnings: Vec<String>,
    pub diff: Vec<String>,
}

impl ChangeAnalysis {
    pub fn safe() -> Self {
        Self {
            impact: SchemaImpact::Safe,
            migration_required: false,
            warnings: Vec::new(),
            diff: Vec::new(),
        }
    }
}

/// Safe presentation patch. Type, uniqueness, and relation target are not
/// applied here — those require [`classify_entity_change`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldUiPatch {
    pub label: Option<String>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub readonly: Option<bool>,
    pub hidden: Option<bool>,
    pub searchable: Option<bool>,
    pub search_weight: Option<i32>,
    pub search_exact: Option<bool>,
    pub sortable: Option<bool>,
    pub filterable: Option<bool>,
    pub widget: Option<String>,
    pub placeholder: Option<String>,
    pub help: Option<String>,
    pub section: Option<String>,
    pub tab: Option<String>,
    pub width: Option<String>,
    pub order: Option<i32>,
    pub widget_options: Option<WidgetOptions>,
    pub formula: Option<String>,
    pub permission_level: Option<u8>,
    pub allow_on_submit: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity: String,
    pub field: String,
}

pub fn has_role(roles: &[String], name: &str) -> bool {
    roles.iter().any(|r| r.eq_ignore_ascii_case(name))
}

pub fn is_production(env: &str) -> bool {
    env.eq_ignore_ascii_case("production")
}

/// Resolve Studio capabilities from roles and runtime environment.
///
/// Tenant `Admin` can inspect and draft. Publishing application metadata and
/// managing installed apps also require a Studio/platform role in production.
/// Development mode grants Admin the full Studio set so local DX stays usable.
pub fn capabilities(roles: &[String], env: &str) -> Vec<String> {
    let mut caps = HashSet::new();
    let admin = has_role(roles, "Admin");
    let platform = has_role(roles, ROLE_PLATFORM_ADMIN);
    let prod = is_production(env);

    if has_role(roles, ROLE_STUDIO_VIEWER) || admin || platform {
        caps.insert(CAP_VIEW.to_string());
    }
    if has_role(roles, ROLE_STUDIO_EDITOR) || admin || platform {
        caps.insert(CAP_VIEW.to_string());
        caps.insert(CAP_EDIT.to_string());
    }
    if has_role(roles, ROLE_STUDIO_PUBLISHER) || platform || (admin && !prod) {
        caps.insert(CAP_VIEW.to_string());
        caps.insert(CAP_EDIT.to_string());
        caps.insert(CAP_PUBLISH.to_string());
    }
    if has_role(roles, ROLE_STUDIO_APP_MANAGER) || platform || (admin && !prod) {
        caps.insert(CAP_VIEW.to_string());
        caps.insert(CAP_MANAGE_APPS.to_string());
    }
    if has_role(roles, ROLE_STUDIO_PERMISSION_MANAGER) || platform || admin {
        caps.insert(CAP_VIEW.to_string());
        caps.insert(CAP_MANAGE_PERMISSIONS.to_string());
    }
    if has_role(roles, ROLE_STUDIO_WORKFLOW_MANAGER) || platform || admin {
        caps.insert(CAP_VIEW.to_string());
        caps.insert(CAP_MANAGE_WORKFLOWS.to_string());
    }
    let mut out: Vec<_> = caps.into_iter().collect();
    out.sort();
    out
}

pub fn require_cap(roles: &[String], env: &str, cap: &str) -> QefroResult<()> {
    if capabilities(roles, env).iter().any(|c| c == cap) {
        Ok(())
    } else {
        Err(QefroError::forbidden(format!(
            "missing Studio capability {cap}"
        )))
    }
}

pub fn known_role(name: &str) -> bool {
    KNOWN_ROLES.iter().any(|r| r.eq_ignore_ascii_case(name))
}

pub fn apply_field_ui_patch(field: &mut FieldDef, patch: &FieldUiPatch) {
    if let Some(v) = &patch.label {
        field.label = v.clone();
        field.ui.label = v.clone();
    }
    if let Some(v) = &patch.description {
        field.ui.description = Some(v.clone());
    }
    if let Some(v) = patch.required {
        field.required = v;
        if v {
            field.nullable = false;
        }
    }
    if let Some(v) = patch.readonly {
        field.ui.readonly = v;
    }
    if let Some(v) = patch.hidden {
        field.ui.hidden = v;
    }
    if let Some(v) = patch.searchable {
        field.searchable = v;
    }
    if let Some(v) = patch.search_weight {
        field.search_weight = v.max(1);
        field.searchable = true;
    }
    if let Some(v) = patch.search_exact {
        field.search_exact = v;
        if v {
            field.searchable = true;
        }
    }
    if let Some(v) = patch.sortable {
        field.ui.sortable = v;
    }
    if let Some(v) = patch.filterable {
        field.ui.filter = v;
    }
    if let Some(v) = &patch.widget {
        field.ui.widget = v.clone();
    }
    if let Some(v) = &patch.placeholder {
        field.ui.placeholder = Some(v.clone());
    }
    if let Some(v) = &patch.help {
        field.ui.help = Some(v.clone());
    }
    if let Some(v) = &patch.section {
        field.ui.section = Some(v.clone());
    }
    if let Some(v) = &patch.tab {
        field.ui.tab = Some(v.clone());
    }
    if let Some(v) = &patch.width {
        field.ui.width = Some(v.clone());
    }
    if let Some(v) = patch.order {
        field.ui.order = v;
    }
    if let Some(opts) = &patch.widget_options {
        field.ui.widget_options = opts.clone();
    }
    if let Some(formula) = &patch.formula {
        if formula.is_empty() {
            field.formula = None;
        } else {
            field.formula = Some(formula.clone());
            field.computed = true;
            field.nullable = true;
        }
    }
    if let Some(level) = patch.permission_level {
        field.permission_level = level.min(3);
    }
    if let Some(v) = patch.allow_on_submit {
        field.allow_on_submit = v;
    }
}

pub fn classify_entity_change(before: &EntityDef, after: &EntityDef) -> ChangeAnalysis {
    let mut analysis = ChangeAnalysis::safe();
    if before.name != after.name {
        analysis.impact = SchemaImpact::Destructive;
        analysis.migration_required = true;
        analysis.warnings.push(
            "Renaming an entity requires a database migration. Qefro will not rewrite the table."
                .into(),
        );
        analysis
            .diff
            .push(format!("~ name {} → {}", before.name, after.name));
    }
    let before_fields: HashMap<&str, &FieldDef> =
        before.fields.iter().map(|f| (f.name.as_str(), f)).collect();
    let after_fields: HashMap<&str, &FieldDef> =
        after.fields.iter().map(|f| (f.name.as_str(), f)).collect();

    for (name, field) in &after_fields {
        match before_fields.get(name) {
            None => {
                if field.is_child_table() {
                    analysis.diff.push(format!("+ child table {name}"));
                } else {
                    analysis.impact = analysis.impact.merge(SchemaImpact::Additive);
                    analysis.migration_required = true;
                    analysis
                        .warnings
                        .push(format!("Adding field '{name}' requires ADD COLUMN."));
                    analysis
                        .diff
                        .push(format!("+ field {name} ({})", field.field_type.as_str()));
                    if !field.ui.widget.is_empty() {
                        analysis
                            .diff
                            .push(format!("+ widget = {}", field.ui.widget));
                    }
                }
            }
            Some(prev) => classify_field(prev, field, &mut analysis),
        }
    }
    for (name, field) in &before_fields {
        if after_fields.contains_key(name) {
            continue;
        }
        analysis.impact = SchemaImpact::Destructive;
        analysis.migration_required = true;
        analysis.warnings.push(format!(
            "Deleting field '{name}' is potentially destructive. The column will not be dropped."
        ));
        analysis
            .diff
            .push(format!("- field {name} ({})", field.field_type.as_str()));
    }
    analysis
}

fn classify_field(before: &FieldDef, after: &FieldDef, analysis: &mut ChangeAnalysis) {
    if std::mem::discriminant(&before.field_type) != std::mem::discriminant(&after.field_type)
        || field_type_key(&before.field_type) != field_type_key(&after.field_type)
    {
        analysis.impact = SchemaImpact::Destructive;
        analysis.migration_required = true;
        analysis.warnings.push(format!(
            "Changing {}.{} from {} to {} requires a data migration.",
            "field",
            after.name,
            before.field_type.as_str(),
            after.field_type.as_str()
        ));
        analysis.diff.push(format!(
            "~ {}.type {} → {}",
            after.name,
            before.field_type.as_str(),
            after.field_type.as_str()
        ));
    }
    if let (FieldType::Enum { values: old }, FieldType::Enum { values: new }) =
        (&before.field_type, &after.field_type)
    {
        for v in new {
            if !old.contains(v) {
                analysis.impact = analysis.impact.merge(SchemaImpact::Additive);
                analysis.migration_required = true;
                analysis.diff.push(format!("+ {}.enum {v}", after.name));
            }
        }
        for v in old {
            if !new.contains(v) {
                analysis.impact = SchemaImpact::Destructive;
                analysis.migration_required = true;
                analysis.warnings.push(format!(
                    "Removing enum value '{v}' on '{}' may invalidate existing rows.",
                    after.name
                ));
                analysis.diff.push(format!("- {}.enum {v}", after.name));
            }
        }
    }
    if before.unique != after.unique && after.unique {
        analysis.impact = analysis.impact.merge(SchemaImpact::Additive);
        analysis.migration_required = true;
        analysis
            .warnings
            .push(format!("Making '{}' unique requires an index.", after.name));
    }
    if let (Some(a), Some(b)) = (&before.relation, &after.relation) {
        if a.target_entity != b.target_entity {
            analysis.impact = SchemaImpact::Destructive;
            analysis.migration_required = true;
            analysis.warnings.push(format!(
                "Retargeting '{}' from {} to {} may leave orphaned ids.",
                after.name, a.target_entity, b.target_entity
            ));
            analysis.diff.push(format!(
                "~ {}.target {} → {}",
                after.name, a.target_entity, b.target_entity
            ));
        }
    }
    if before.label != after.label {
        analysis.diff.push(format!(
            "~ {}.label {:?} → {:?}",
            after.name, before.label, after.label
        ));
    }
    if before.ui.widget != after.ui.widget {
        analysis.diff.push(format!(
            "~ {}.widget {} → {}",
            after.name, before.ui.widget, after.ui.widget
        ));
    }
    if before.formula != after.formula {
        analysis.diff.push(format!(
            "~ {}.formula {:?} → {:?}",
            after.name, before.formula, after.formula
        ));
    }
}

fn field_type_key(ty: &FieldType) -> String {
    match ty {
        FieldType::Enum { values } => format!("enum:{}", values.join(",")),
        other => other.as_str().to_string(),
    }
}

pub fn entity_referrers(registry: &EntityRegistry, name: &str) -> Vec<EntityRef> {
    let mut out = Vec::new();
    for entity in registry.list() {
        if entity.name == name {
            continue;
        }
        for field in &entity.fields {
            if let Some(rel) = &field.relation {
                if rel.target_entity == name {
                    out.push(EntityRef {
                        entity: entity.name.clone(),
                        field: field.name.clone(),
                    });
                }
            }
        }
        if entity
            .child_of
            .as_ref()
            .is_some_and(|c| c.parent_entity == name)
        {
            out.push(EntityRef {
                entity: entity.name.clone(),
                field: entity
                    .child_of
                    .as_ref()
                    .map(|c| c.parent_field.clone())
                    .unwrap_or_else(|| "parent".into()),
            });
        }
    }
    out
}

/// Metadata-level formula preview. The server evaluator remains authoritative
/// for persisted computed values.
pub fn preview_formula(formula: &str, record: &Value) -> QefroResult<f64> {
    let expr = parse_formula(formula)?;
    let children = HashMap::new();
    eval_formula(
        &expr,
        &FormulaContext {
            record,
            children: &children,
        },
    )
}

pub fn validate_formula_on_entity(
    entity: &EntityDef,
    field: &str,
    formula: &str,
) -> QefroResult<()> {
    let expr = parse_formula(formula)?;
    let deps = crate::formula::formula_dependencies(&expr);
    for dep in deps {
        if let Some((table, child_field)) = dep.split_once('.') {
            let child = entity
                .fields
                .iter()
                .find(|f| f.name == table && f.is_child_table())
                .ok_or_else(|| {
                    QefroError::bad_request(format!(
                        "formula on '{field}' references unknown child table '{table}'"
                    ))
                })?;
            let _ = child;
            let _ = child_field;
        } else if entity.get_field(&dep).is_none()
            && !entity
                .fields
                .iter()
                .any(|f| f.is_child_table() && f.name == dep)
        {
            return Err(QefroError::bad_request(format!(
                "formula on '{field}' references unknown field '{dep}'"
            )));
        }
    }
    let mut fields = entity.fields.clone();
    if let Some(existing) = fields.iter_mut().find(|f| f.name == field) {
        existing.formula = Some(formula.to_string());
        existing.computed = true;
    }
    crate::formula::detect_cycles(&fields)?;
    Ok(())
}

pub const FORMULA_FUNCTIONS: &[&str] = &[
    "SUM", "MIN", "MAX", "COUNT", "ROUND", "+", "-", "*", "/", "%", "()",
];

/// Live overlay for reports, dashboards, and print formats. Entity/workflow/
/// permission overlays live on those registries.
#[derive(Debug, Default)]
pub struct StudioCatalog {
    reports: RwLock<HashMap<String, ReportDef>>,
    dashboards: RwLock<HashMap<String, DashboardDef>>,
    print_formats: RwLock<HashMap<String, PrintFormat>>,
}

impl StudioCatalog {
    pub fn upsert_report(&self, def: ReportDef) {
        if let Ok(mut g) = self.reports.write() {
            g.insert(def.name.clone(), def);
        }
    }

    pub fn upsert_dashboard(&self, def: DashboardDef) {
        if let Ok(mut g) = self.dashboards.write() {
            g.insert(def.name.clone(), def);
        }
    }

    pub fn upsert_print_format(&self, def: PrintFormat) {
        if let Ok(mut g) = self.print_formats.write() {
            g.insert(def.name.clone(), def);
        }
    }

    pub fn report(&self, name: &str) -> Option<ReportDef> {
        self.reports.read().ok()?.get(name).cloned()
    }

    pub fn dashboard(&self, name: &str) -> Option<DashboardDef> {
        self.dashboards.read().ok()?.get(name).cloned()
    }

    pub fn print_format(&self, name: &str) -> Option<PrintFormat> {
        self.print_formats.read().ok()?.get(name).cloned()
    }

    pub fn merge_reports(&self, base: &[ReportDef]) -> Vec<ReportDef> {
        merge_named(base, self.reports.read().ok().as_deref(), |r| {
            r.name.clone()
        })
    }

    pub fn merge_dashboards(&self, base: &[DashboardDef]) -> Vec<DashboardDef> {
        merge_named(base, self.dashboards.read().ok().as_deref(), |d| {
            d.name.clone()
        })
    }

    pub fn merge_print_formats(&self, base: &[PrintFormat]) -> Vec<PrintFormat> {
        merge_named(base, self.print_formats.read().ok().as_deref(), |p| {
            p.name.clone()
        })
    }
}

fn merge_named<T: Clone>(
    base: &[T],
    overlay: Option<&HashMap<String, T>>,
    name: impl Fn(&T) -> String,
) -> Vec<T> {
    let mut map: HashMap<String, T> = base
        .iter()
        .cloned()
        .map(|item| (name(&item), item))
        .collect();
    if let Some(overlay) = overlay {
        for (k, v) in overlay {
            map.insert(k.clone(), v.clone());
        }
    }
    let mut items: Vec<_> = map.into_values().collect();
    items.sort_by_key(|item| name(item));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;

    #[test]
    fn admin_in_dev_can_publish() {
        let caps = capabilities(&["Admin".into()], "development");
        assert!(caps.contains(&CAP_PUBLISH.to_string()));
        assert!(caps.contains(&CAP_MANAGE_APPS.to_string()));
    }

    #[test]
    fn admin_in_production_cannot_publish() {
        let caps = capabilities(&["Admin".into()], "production");
        assert!(caps.contains(&CAP_VIEW.to_string()));
        assert!(caps.contains(&CAP_EDIT.to_string()));
        assert!(!caps.contains(&CAP_PUBLISH.to_string()));
        assert!(!caps.contains(&CAP_MANAGE_APPS.to_string()));
    }

    #[test]
    fn staff_has_no_studio() {
        let caps = capabilities(&["Staff".into()], "development");
        assert!(caps.is_empty());
        assert!(require_cap(&["Staff".into()], "development", CAP_VIEW).is_err());
    }

    #[test]
    fn label_change_is_safe() {
        let before = EntityDef::new("Order")
            .field(FieldDef::string("status").label("Status"))
            .build();
        let after = EntityDef::new("Order")
            .field(FieldDef::string("status").label("Booking Status"))
            .build();
        let a = classify_entity_change(&before, &after);
        assert_eq!(a.impact, SchemaImpact::Safe);
        assert!(!a.migration_required);
        assert!(a.diff.iter().any(|l| l.contains("label")));
    }

    #[test]
    fn add_field_is_additive() {
        let before = EntityDef::new("Reservation")
            .field(FieldDef::string("notes"))
            .build();
        let after = EntityDef::new("Reservation")
            .field(FieldDef::string("notes"))
            .field(FieldDef::enum_("source", vec!["Website", "WhatsApp", "Walk-in"]).nullable())
            .build();
        let a = classify_entity_change(&before, &after);
        assert_eq!(a.impact, SchemaImpact::Additive);
        assert!(a.migration_required);
    }

    #[test]
    fn type_change_is_destructive() {
        let before = EntityDef::new("Item")
            .field(FieldDef::string("amount"))
            .build();
        let after = EntityDef::new("Item")
            .field(FieldDef::datetime("amount"))
            .build();
        let a = classify_entity_change(&before, &after);
        assert_eq!(a.impact, SchemaImpact::Destructive);
    }

    #[test]
    fn formula_preview_multiplies() {
        let record = serde_json::json!({ "quantity": 2, "rate": 300 });
        assert_eq!(preview_formula("quantity * rate", &record).unwrap(), 600.0);
    }
}
