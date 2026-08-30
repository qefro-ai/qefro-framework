//! Composed business pages. Metadata that arranges existing Qefro components.
//!
//! Pages are not a second application runtime. They compose Entity views,
//! dashboards, reports, actions, and related lists already driven by
//! `EntityDef` → `EntityService` → REST / generic UI.

use crate::error::{QefroError, QefroResult};
use crate::ident::{kebab_case, slugify};
use crate::registry::EntityRegistry;
use crate::ui::DashboardCard;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub const PAGE_LAYOUTS: &[&str] = &["stack", "two_column", "three_column", "grid", "split"];
pub const PAGE_TEMPLATES: &[&str] = &[
    "operations_dashboard",
    "sales_workspace",
    "customer_workspace",
];
pub const PAGE_SECTION_KINDS: &[&str] = &[
    "entity_view",
    "related",
    "report",
    "widget",
    "activity",
    "attachments",
    "action",
];
pub const PAGE_VIEWS: &[&str] = &[
    "list", "card", "kanban", "calendar", "chart", "detail", "form",
];
pub const PAGE_PANES: &[&str] = &["master", "detail", "main"];

/// Application-level composed page. Sibling of [`crate::ui::DashboardDef`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDef {
    pub name: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(
        default = "default_layout",
        deserialize_with = "deserialize_layout",
        skip_serializing_if = "is_default_layout"
    )]
    pub layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// When non-empty, only these roles see the page. Section permissions still apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_param: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<PageTab>,
    #[serde(default, alias = "components", skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<PageSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<PageActionRef>,
    /// Shared filter field names consumed by embedded lists/widgets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
}

fn default_layout() -> String {
    "stack".into()
}

fn is_default_layout(value: &str) -> bool {
    value == "stack"
}

fn deserialize_layout<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let raw = match value {
        Value::String(s) => s,
        Value::Object(map) => map
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("stack")
            .to_string(),
        _ => "stack".into(),
    };
    Ok(normalize_layout(&raw))
}

pub fn normalize_layout(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "2_column" | "two_column" | "twocolumn" | "2column" => "two_column".into(),
        "3_column" | "three_column" | "threecolumn" | "3column" => "three_column".into(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageTab {
    pub name: String,
    pub label: String,
}

impl PageTab {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
        }
    }
}

/// One composed region. References existing entities/views/reports/widgets.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageSection {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub title: String,
    /// `entity_view`, `related`, `report`, `widget`, `activity`, `attachments`, `action`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Query string applied to the embedded list, e.g. `status=Preparing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Inline dashboard card for `widget` sections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<DashboardCard>,
    /// Existing dashboard name. The renderer loads that dashboard once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<String>,
    /// Card title inside `dashboard` when `card` is omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    /// `master`, `detail`, or `main` for split layouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageActionRef {
    pub entity: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl PageActionRef {
    pub fn new(entity: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            entity: entity.into(),
            action: action.into(),
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl PageDef {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        let name = name.into();
        let slug = slugify(&name);
        Self {
            name,
            label: label.into(),
            slug,
            description: None,
            module: None,
            layout: default_layout(),
            template: None,
            roles: Vec::new(),
            context_entity: None,
            context_param: None,
            tabs: Vec::new(),
            sections: Vec::new(),
            actions: Vec::new(),
            filters: Vec::new(),
        }
    }

    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn layout(mut self, layout: impl Into<String>) -> Self {
        self.layout = normalize_layout(&layout.into());
        self
    }

    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn slug_name(mut self, slug: impl Into<String>) -> Self {
        self.slug = slug.into();
        self
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn context(mut self, entity: impl Into<String>, param: impl Into<String>) -> Self {
        self.context_entity = Some(entity.into());
        self.context_param = Some(param.into());
        self
    }

    pub fn tab(mut self, tab: PageTab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn section(mut self, section: PageSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn action(mut self, action: PageActionRef) -> Self {
        self.actions.push(action);
        self
    }

    pub fn filter_fields(mut self, fields: &[&str]) -> Self {
        self.filters = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn route(&self) -> String {
        format!("/pages/{}", self.slug())
    }

    pub fn slug(&self) -> &str {
        if self.slug.is_empty() {
            self.name.as_str()
        } else {
            self.slug.as_str()
        }
    }

    pub fn normalize(&mut self) {
        if self.slug.is_empty() {
            self.slug = slugify(&self.name);
        }
        self.layout = normalize_layout(&self.layout);
        for (i, section) in self.sections.iter_mut().enumerate() {
            section.normalize(i);
        }
        if let Some(template) = &self.template {
            if self.layout == "stack" {
                match template.as_str() {
                    "operations_dashboard" | "sales_workspace" => self.layout = "grid".into(),
                    "customer_workspace" => self.layout = "split".into(),
                    _ => {}
                }
            }
        }
    }
}

impl PageSection {
    pub fn entity_view(
        title: impl Into<String>,
        entity: impl Into<String>,
        view: impl Into<String>,
    ) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "entity_view".into(),
            entity: Some(entity.into()),
            view: Some(view.into()),
            ..Default::default()
        }
    }

    pub fn related(
        title: impl Into<String>,
        entity: impl Into<String>,
        relation: impl Into<String>,
    ) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "related".into(),
            entity: Some(entity.into()),
            relation: Some(relation.into()),
            view: Some("list".into()),
            ..Default::default()
        }
    }

    pub fn report(title: impl Into<String>, report: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "report".into(),
            report: Some(report.into()),
            ..Default::default()
        }
    }

    pub fn widget(title: impl Into<String>, card: DashboardCard) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "widget".into(),
            entity: Some(card.entity.clone()),
            card: Some(card),
            ..Default::default()
        }
    }

    pub fn widget_from(
        title: impl Into<String>,
        dashboard: impl Into<String>,
        widget: impl Into<String>,
    ) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "widget".into(),
            dashboard: Some(dashboard.into()),
            widget: Some(widget.into()),
            ..Default::default()
        }
    }

    pub fn activity(title: impl Into<String>, entity: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "activity".into(),
            entity: Some(entity.into()),
            ..Default::default()
        }
    }

    pub fn attachments(title: impl Into<String>, entity: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            name: kebab_case(&title),
            title,
            kind: "attachments".into(),
            entity: Some(entity.into()),
            ..Default::default()
        }
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    pub fn size(mut self, size: impl Into<String>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn tab(mut self, tab: impl Into<String>) -> Self {
        self.tab = Some(tab.into());
        self
    }

    pub fn pane(mut self, pane: impl Into<String>) -> Self {
        self.pane = Some(pane.into());
        self
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn resolved_kind(&self) -> String {
        if !self.kind.is_empty() {
            return self.kind.clone();
        }
        if self.card.is_some() || self.widget.is_some() || self.dashboard.is_some() {
            return "widget".into();
        }
        if self.report.is_some() {
            return "report".into();
        }
        if self.relation.is_some() {
            return "related".into();
        }
        if self.action.is_some() {
            return "action".into();
        }
        if self.entity.is_some() {
            return "entity_view".into();
        }
        "entity_view".into()
    }

    pub fn normalize(&mut self, index: usize) {
        if self.kind.is_empty() {
            self.kind = self.resolved_kind();
        }
        if self.name.is_empty() {
            self.name = if self.title.is_empty() {
                format!("section-{}", index + 1)
            } else {
                kebab_case(&self.title)
            };
        }
        if let Some(view) = &self.view {
            self.view = Some(view.trim().to_ascii_lowercase());
        }
        if let Some(pane) = &self.pane {
            self.pane = Some(pane.trim().to_ascii_lowercase());
        }
        if self.entity.is_none() {
            if let Some(card) = &self.card {
                self.entity = Some(card.entity.clone());
            }
        }
    }

    pub fn entity_name(&self) -> Option<&str> {
        self.entity
            .as_deref()
            .or_else(|| self.card.as_ref().map(|c| c.entity.as_str()))
            .filter(|name| !name.is_empty() && !name.starts_with('_'))
    }
}

/// Validate a page against the live registries. Does not redefine fields,
/// permissions, workflow, or validation — it only checks references.
pub fn validate_page(
    page: &PageDef,
    registry: &EntityRegistry,
    reports: &[crate::document::ReportDef],
    dashboards: &[crate::ui::DashboardDef],
    entity_slugs: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    if page.name.trim().is_empty() {
        errors.push("page is missing name".into());
    }
    if page.label.trim().is_empty() {
        errors.push(format!("page '{}' is missing label", page.name));
    }
    let layout = normalize_layout(&page.layout);
    if !PAGE_LAYOUTS.contains(&layout.as_str()) {
        errors.push(format!(
            "page '{}' has invalid layout '{}'",
            page.name, page.layout
        ));
    }
    if let Some(template) = &page.template {
        if !PAGE_TEMPLATES.contains(&template.as_str()) {
            errors.push(format!(
                "page '{}' has unknown template '{}'",
                page.name, template
            ));
        }
    }
    let slug = if page.slug.is_empty() {
        slugify(&page.name)
    } else {
        page.slug.clone()
    };
    if entity_slugs.iter().any(|s| s == &slug) {
        errors.push(format!(
            "page '{}' slug '{}' collides with an entity slug",
            page.name, slug
        ));
    }
    if RESERVED_PAGE_SLUGS.contains(&slug.as_str()) {
        errors.push(format!(
            "page '{}' uses reserved route slug '{}'",
            page.name, slug
        ));
    }
    if let Some(entity) = &page.context_entity {
        if registry.try_get(entity).is_none() {
            errors.push(format!(
                "page '{}' context_entity '{}' is unknown",
                page.name, entity
            ));
        }
    }
    let tab_names: Vec<&str> = page.tabs.iter().map(|t| t.name.as_str()).collect();
    let mut section_names = std::collections::HashSet::new();
    for (i, section) in page.sections.iter().enumerate() {
        let mut section = section.clone();
        section.normalize(i);
        if !section_names.insert(section.name.clone()) {
            errors.push(format!(
                "page '{}' has duplicate section '{}'",
                page.name, section.name
            ));
        }
        let kind = section.resolved_kind();
        if !PAGE_SECTION_KINDS.contains(&kind.as_str()) {
            errors.push(format!(
                "page '{}' section '{}' has unknown component '{}'",
                page.name,
                section.title_or_name(),
                kind
            ));
        }
        if let Some(view) = &section.view {
            if !PAGE_VIEWS.contains(&view.as_str()) {
                errors.push(format!(
                    "page '{}' section '{}' has unknown view '{}'",
                    page.name,
                    section.title_or_name(),
                    view
                ));
            }
        }
        if let Some(pane) = &section.pane {
            if !PAGE_PANES.contains(&pane.as_str()) {
                errors.push(format!(
                    "page '{}' section '{}' has invalid pane '{}'",
                    page.name,
                    section.title_or_name(),
                    pane
                ));
            }
        }
        if let Some(tab) = &section.tab {
            if !tab_names.is_empty() && !tab_names.contains(&tab.as_str()) {
                errors.push(format!(
                    "page '{}' section '{}' references unknown tab '{}'",
                    page.name,
                    section.title_or_name(),
                    tab
                ));
            }
        }
        match kind.as_str() {
            "entity_view" | "related" | "activity" | "attachments" | "action" => {
                match section.entity_name() {
                    None => errors.push(format!(
                        "page '{}' section '{}' is missing entity",
                        page.name,
                        section.title_or_name()
                    )),
                    Some(entity) => {
                        if registry.try_get(entity).is_none() {
                            errors.push(format!(
                                "page '{}' section '{}' references unknown entity '{}'",
                                page.name,
                                section.title_or_name(),
                                entity
                            ));
                        }
                    }
                }
            }
            "widget" => {
                if let Some(card) = &section.card {
                    if !card.entity.is_empty()
                        && !card.entity.starts_with('_')
                        && registry.try_get(&card.entity).is_none()
                    {
                        errors.push(format!(
                            "page '{}' widget '{}' references unknown entity '{}'",
                            page.name, card.title, card.entity
                        ));
                    }
                } else if section.dashboard.is_none() && section.entity_name().is_none() {
                    errors.push(format!(
                        "page '{}' section '{}' widget is missing entity or dashboard",
                        page.name,
                        section.title_or_name()
                    ));
                }
                if let Some(dash) = &section.dashboard {
                    if !dashboards.iter().any(|d| d.name == *dash) {
                        errors.push(format!(
                            "page '{}' section '{}' references unknown dashboard '{}'",
                            page.name,
                            section.title_or_name(),
                            dash
                        ));
                    }
                }
            }
            "report" => match &section.report {
                None => errors.push(format!(
                    "page '{}' section '{}' is missing report",
                    page.name,
                    section.title_or_name()
                )),
                Some(name) => {
                    if !reports.iter().any(|r| r.name == *name) {
                        errors.push(format!(
                            "page '{}' section '{}' references unknown report '{}'",
                            page.name,
                            section.title_or_name(),
                            name
                        ));
                    }
                }
            },
            _ => {}
        }
        if kind == "related" {
            if let (Some(entity), Some(relation)) =
                (section.entity_name(), section.relation.as_deref())
            {
                if let Some(def) = registry.try_get(entity) {
                    let known = def.fields.iter().any(|f| {
                        f.name == relation
                            || f.relation
                                .as_ref()
                                .is_some_and(|r| r.inverse_field.as_deref() == Some(relation))
                    }) || def.links.iter().any(|l| l.relation == relation);
                    if !known {
                        errors.push(format!(
                            "page '{}' section '{}' has invalid relation '{}'",
                            page.name,
                            section.title_or_name(),
                            relation
                        ));
                    }
                }
            } else if section.relation.is_none() {
                errors.push(format!(
                    "page '{}' section '{}' related component is missing relation",
                    page.name,
                    section.title_or_name()
                ));
            }
        }
        if let Some(view) = &section.view {
            if let Some(entity) = section.entity_name() {
                if let Some(def) = registry.try_get(entity) {
                    if matches!(view.as_str(), "kanban" | "calendar" | "card" | "chart") {
                        if let Some(views) = &def.views {
                            let enabled = match view.as_str() {
                                "kanban" => views.kanban.as_ref().is_some_and(|v| v.enabled),
                                "calendar" => views.calendar.as_ref().is_some_and(|v| v.enabled),
                                "card" => views.card.as_ref().is_some_and(|v| v.enabled),
                                "chart" => views.chart.as_ref().is_some_and(|v| v.enabled),
                                _ => true,
                            };
                            if !enabled && def.views.is_some() {
                                // Optional views: warn via error only when explicitly disabled.
                                let explicitly_disabled = match view.as_str() {
                                    "kanban" => views.kanban.as_ref().is_some_and(|v| !v.enabled),
                                    "calendar" => {
                                        views.calendar.as_ref().is_some_and(|v| !v.enabled)
                                    }
                                    "card" => views.card.as_ref().is_some_and(|v| !v.enabled),
                                    "chart" => views.chart.as_ref().is_some_and(|v| !v.enabled),
                                    _ => false,
                                };
                                if explicitly_disabled {
                                    errors.push(format!(
                                        "page '{}' section '{}' view '{}' is disabled on '{}'",
                                        page.name,
                                        section.title_or_name(),
                                        view,
                                        entity
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(action) = &section.action {
            if let Some(entity) = section.entity_name() {
                if let Some(def) = registry.try_get(entity) {
                    let known = def.actions.iter().any(|a| a.name == *action)
                        || action == "create"
                        || action == "export"
                        || action == "refresh";
                    if !known {
                        errors.push(format!(
                            "page '{}' section '{}' references unknown action '{}' on '{}'",
                            page.name,
                            section.title_or_name(),
                            action,
                            entity
                        ));
                    }
                }
            }
        }
    }
    for action in &page.actions {
        if registry.try_get(&action.entity).is_none() {
            errors.push(format!(
                "page '{}' action '{}' references unknown entity '{}'",
                page.name, action.action, action.entity
            ));
            continue;
        }
        if let Some(def) = registry.try_get(&action.entity) {
            let known = def.actions.iter().any(|a| a.name == action.action)
                || action.action == "create"
                || action.action == "export"
                || action.action == "refresh";
            if !known {
                errors.push(format!(
                    "page '{}' references unknown action '{}' on '{}'",
                    page.name, action.action, action.entity
                ));
            }
        }
    }
    errors
}

impl PageSection {
    fn title_or_name(&self) -> &str {
        if self.title.is_empty() {
            &self.name
        } else {
            &self.title
        }
    }
}

const RESERVED_PAGE_SLUGS: &[&str] = &[
    "settings", "reports", "studio", "login", "pages", "auth", "meta",
];

/// Studio composition stays declarative. Reject executable or query payloads.
pub fn reject_unsafe_page_payload(payload: &Value) -> QefroResult<()> {
    walk_reject(payload)
}

fn walk_reject(value: &Value) -> QefroResult<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let key_l = key.to_ascii_lowercase();
                if matches!(
                    key_l.as_str(),
                    "javascript"
                        | "script"
                        | "html"
                        | "sql"
                        | "query_sql"
                        | "raw_sql"
                        | "onclick"
                        | "href"
                        | "src"
                        | "url"
                        | "endpoint"
                        | "handler"
                        | "code"
                ) {
                    return Err(QefroError::bad_request(format!(
                        "page composition rejects '{key}'"
                    )));
                }
                walk_reject(child)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                walk_reject(item)?;
            }
            Ok(())
        }
        Value::String(s) => reject_unsafe_string(s),
        _ => Ok(()),
    }
}

fn reject_unsafe_string(s: &str) -> QefroResult<()> {
    let lower = s.to_ascii_lowercase();
    if lower.contains("<script")
        || lower.contains("javascript:")
        || lower.contains("onerror=")
        || lower.contains("onload=")
    {
        return Err(QefroError::bad_request(
            "page composition rejects custom JavaScript or HTML",
        ));
    }
    if lower.contains("<html") || lower.contains("<iframe") || lower.contains("</") {
        return Err(QefroError::bad_request(
            "page composition rejects custom HTML",
        ));
    }
    let trimmed = lower.trim_start();
    if trimmed.starts_with("select ")
        || trimmed.starts_with("insert ")
        || trimmed.starts_with("update ")
        || trimmed.starts_with("delete ")
        || trimmed.starts_with("drop ")
        || trimmed.contains(" union select ")
    {
        return Err(QefroError::bad_request(
            "page composition rejects custom SQL",
        ));
    }
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.contains("://")
    {
        return Err(QefroError::bad_request(
            "page composition rejects arbitrary URLs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ReportDef;
    use crate::entity::EntityDef;
    use crate::field::FieldDef;
    use crate::ui::DashboardDef;
    use serde_json::json;

    fn registry() -> EntityRegistry {
        let mut registry = EntityRegistry::new();
        registry
            .register(
                EntityDef::new("Opportunity")
                    .slug_name("opportunities")
                    .field(FieldDef::string("name").required())
                    .field(FieldDef::enum_("status", vec!["Open", "Won"]).filterable())
                    .build(),
            )
            .unwrap();
        registry
            .register(
                EntityDef::new("Task")
                    .field(FieldDef::string("title").required())
                    .build(),
            )
            .unwrap();
        registry
    }

    #[test]
    fn yaml_layout_object_and_components_alias() {
        let page: PageDef = serde_yaml::from_str(
            r#"
name: sales_workspace
label: Sales Workspace
layout:
  type: split
components:
  - entity: Opportunity
    view: kanban
  - entity: Task
    view: list
  - report: sales_pipeline
"#,
        )
        .unwrap();
        assert_eq!(page.layout, "split");
        assert_eq!(page.sections.len(), 3);
        assert_eq!(page.sections[0].entity.as_deref(), Some("Opportunity"));
        assert_eq!(page.sections[2].report.as_deref(), Some("sales_pipeline"));
    }

    #[test]
    fn validate_unknown_entity_and_layout() {
        let registry = registry();
        let mut page = PageDef::new("sales_workspace", "Sales")
            .layout("canvas")
            .section(PageSection::entity_view("Ghost", "Missing", "list"));
        page.normalize();
        let errors = validate_page(&page, &registry, &[], &[], &[]);
        assert!(errors.iter().any(|e| e.contains("invalid layout")));
        assert!(errors.iter().any(|e| e.contains("unknown entity")));
    }

    #[test]
    fn validate_unknown_report_and_view() {
        let registry = registry();
        let page = PageDef::new("sales_workspace", "Sales")
            .section(PageSection::entity_view(
                "Pipeline",
                "Opportunity",
                "timeline",
            ))
            .section(PageSection::report("Pipeline", "missing_report"));
        let errors = validate_page(&page, &registry, &[], &[], &[]);
        assert!(errors.iter().any(|e| e.contains("unknown view")));
        assert!(errors.iter().any(|e| e.contains("unknown report")));
    }

    #[test]
    fn valid_sales_workspace() {
        let registry = registry();
        let reports = vec![ReportDef::new("sales_pipeline", "Opportunity")];
        let dashboards = vec![DashboardDef::new("crm-ops", "CRM")];
        let mut page = PageDef::new("sales_workspace", "Sales Workspace")
            .template("sales_workspace")
            .layout("grid")
            .section(PageSection::widget_from("Revenue", "crm-ops", "Pipeline"))
            .section(PageSection::entity_view(
                "Pipeline",
                "Opportunity",
                "kanban",
            ))
            .section(PageSection::entity_view("Tasks", "Task", "list"))
            .section(PageSection::report("Pipeline report", "sales_pipeline"))
            .action(PageActionRef::new("Opportunity", "create").label("New Opportunity"));
        page.normalize();
        let errors = validate_page(&page, &registry, &reports, &dashboards, &[]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(page.route(), "/pages/sales-workspace");
    }

    #[test]
    fn duplicate_route_against_entity_slug() {
        let registry = registry();
        let page = PageDef::new("opportunities", "Oops");
        let errors = validate_page(&page, &registry, &[], &[], &["opportunities".into()]);
        assert!(errors.iter().any(|e| e.contains("collides")));
    }

    #[test]
    fn studio_rejects_javascript_html_sql_and_urls() {
        assert!(reject_unsafe_page_payload(&json!({"script": "alert(1)"})).is_err());
        assert!(reject_unsafe_page_payload(&json!({"title": "<script>x</script>"})).is_err());
        assert!(reject_unsafe_page_payload(&json!({"query": "SELECT * FROM orders"})).is_err());
        assert!(reject_unsafe_page_payload(&json!({"src": "https://evil.example"})).is_err());
        assert!(reject_unsafe_page_payload(&json!({
            "name": "sales_workspace",
            "sections": [{ "entity": "Opportunity", "view": "kanban" }]
        }))
        .is_ok());
    }

    #[test]
    fn two_column_alias_normalizes() {
        assert_eq!(normalize_layout("2-column"), "two_column");
        assert_eq!(normalize_layout("3-column"), "three_column");
    }
}
