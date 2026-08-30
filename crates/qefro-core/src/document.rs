//! Document, numbering, print, and report metadata.
//!
//! These types describe behavior. Execution stays in `EntityService`, the
//! workflow engine, and the existing filter/aggregation SQL builders.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildOf {
    pub parent_entity: String,
    /// Child-table field name on the parent, e.g. `items`.
    pub parent_field: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentConfig {
    #[serde(default)]
    pub submit_enabled: bool,
    #[serde(default)]
    pub cancel_enabled: bool,
    #[serde(default)]
    pub amend_enabled: bool,
    #[serde(default)]
    pub duplicate_enabled: bool,
    /// Workflow states in which business fields cannot be PATCHed.
    #[serde(default)]
    pub lock_states: Vec<String>,
    /// `create` or `submit`.
    #[serde(default)]
    pub number_on: String,
}

impl DocumentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit(mut self) -> Self {
        self.submit_enabled = true;
        self
    }

    pub fn cancel(mut self) -> Self {
        self.cancel_enabled = true;
        self
    }

    pub fn amend(mut self) -> Self {
        self.amend_enabled = true;
        self
    }

    pub fn duplicate(mut self) -> Self {
        self.duplicate_enabled = true;
        self
    }

    pub fn lock_states(mut self, states: &[&str]) -> Self {
        self.lock_states = states.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn number_on(mut self, when: impl Into<String>) -> Self {
        self.number_on = when.into();
        self
    }

    pub fn is_locked(&self, status: &str) -> bool {
        self.lock_states.iter().any(|s| s == status)
    }
}

impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            submit_enabled: false,
            cancel_enabled: false,
            amend_enabled: false,
            duplicate_enabled: false,
            lock_states: Vec::new(),
            number_on: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamingConfig {
    pub pattern: String,
    /// Stored field that receives the generated number. Defaults to `doc_no`.
    #[serde(default = "default_doc_no")]
    pub field: String,
    /// `create` or `submit`.
    #[serde(default = "default_assign_create")]
    pub assign_on: String,
}

fn default_doc_no() -> String {
    "doc_no".into()
}

fn default_assign_create() -> String {
    "create".into()
}

impl NamingConfig {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            field: default_doc_no(),
            assign_on: default_assign_create(),
        }
    }

    pub fn field(mut self, name: impl Into<String>) -> Self {
        self.field = name.into();
        self
    }

    pub fn assign_on(mut self, when: impl Into<String>) -> Self {
        self.assign_on = when.into();
        self
    }
}

pub const PRINT_VARIANTS: &[&str] = &["default", "compact", "professional"];
pub const PRINT_SECTION_KINDS: &[&str] = &[
    "header", "customer", "address", "items", "totals", "notes", "terms", "footer", "text", "image",
];

/// One printable region. Fields and text resolve against EntityDef relations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrintSection {
    /// `header`, `customer`, `address`, `items`, `totals`, `notes`, `terms`, `footer`, `text`, `image`.
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Field or relation paths, e.g. `customer.name`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    /// Safe template snippet (`{{ path }}`, `{% for %}`, `{% if %}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    /// Child table to iterate. Defaults to the format `item_table`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_over: Option<String>,
    /// Simple path or `path > 0` condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_when: Option<String>,
}

impl PrintSection {
    pub fn kind(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            ..Default::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn fields(mut self, fields: &[&str]) -> Self {
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn relation(mut self, relation: impl Into<String>) -> Self {
        self.relation = Some(relation.into());
        self
    }

    pub fn loop_over(mut self, name: impl Into<String>) -> Self {
        self.loop_over = Some(name.into());
        self
    }

    pub fn show_when(mut self, expr: impl Into<String>) -> Self {
        self.show_when = Some(expr.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintFormat {
    pub name: String,
    pub entity: String,
    #[serde(default)]
    pub title: Option<String>,
    /// `default`, `compact`, or `professional`. Presentation only.
    #[serde(default = "default_variant")]
    pub variant: String,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_true")]
    pub header: bool,
    #[serde(default = "default_true")]
    pub items: bool,
    #[serde(default = "default_true")]
    pub totals: bool,
    #[serde(default = "default_true")]
    pub footer: bool,
    /// Child table to render as the items grid. Defaults to the first child table.
    #[serde(default)]
    pub item_table: Option<String>,
    #[serde(default)]
    pub total_fields: Vec<String>,
    #[serde(default)]
    pub module: Option<String>,
    /// Optional field used for PDF filenames (`doc_no` by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename_field: Option<String>,
    /// Optional full-document template. When set, it wraps the composed sections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<PrintSection>,
}

fn default_true() -> bool {
    true
}

fn default_variant() -> String {
    "default".into()
}

fn default_version() -> u32 {
    1
}

impl PrintFormat {
    pub fn new(name: impl Into<String>, entity: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity: entity.into(),
            title: None,
            variant: default_variant(),
            version: default_version(),
            header: true,
            items: true,
            totals: true,
            footer: true,
            item_table: None,
            total_fields: Vec::new(),
            module: None,
            filename_field: None,
            body: None,
            sections: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = variant.into();
        self
    }

    pub fn item_table(mut self, name: impl Into<String>) -> Self {
        self.item_table = Some(name.into());
        self
    }

    pub fn total_fields(mut self, fields: &[&str]) -> Self {
        self.total_fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn filename_field(mut self, field: impl Into<String>) -> Self {
        self.filename_field = Some(field.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn section(mut self, section: PrintSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn document_title(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.name.clone())
    }

    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// True when this format defines a printable document for `entity`.
    pub fn matches_entity(&self, entity: &str) -> bool {
        self.entity.eq_ignore_ascii_case(entity)
    }
}

/// Resolve a named format, otherwise the first format for the entity.
pub fn resolve_print_format<'a>(
    entity_name: &str,
    format_name: Option<&str>,
    entity_formats: &'a [PrintFormat],
    extra: &'a [PrintFormat],
) -> Option<PrintFormat> {
    let mut all: Vec<&PrintFormat> = entity_formats
        .iter()
        .chain(extra.iter().filter(|f| f.matches_entity(entity_name)))
        .collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(name) = format_name.filter(|n| !n.is_empty()) {
        return all.into_iter().find(|f| f.name == name).cloned();
    }
    entity_formats.first().cloned().or_else(|| {
        extra
            .iter()
            .find(|f| f.matches_entity(entity_name))
            .cloned()
    })
}

/// Validate print format references against live entity metadata.
pub fn validate_print_format(
    format: &PrintFormat,
    registry: &crate::registry::EntityRegistry,
) -> Vec<String> {
    let mut errors = Vec::new();
    if format.name.trim().is_empty() {
        errors.push("print format is missing name".into());
    }
    if registry.try_get(&format.entity).is_none() {
        errors.push(format!(
            "print format '{}' references unknown entity '{}'",
            format.name, format.entity
        ));
        return errors;
    }
    if !PRINT_VARIANTS.contains(&format.variant.as_str()) && !format.variant.is_empty() {
        errors.push(format!(
            "print format '{}' has invalid variant '{}'",
            format.name, format.variant
        ));
    }
    let entity = registry.try_get(&format.entity).unwrap();
    if let Some(table) = &format.item_table {
        let known = entity
            .fields
            .iter()
            .any(|f| f.name == *table && f.is_child_table());
        if !known {
            errors.push(format!(
                "print format '{}' has unknown item table '{}'",
                format.name, table
            ));
        }
    }
    if let Some(field) = &format.filename_field {
        if entity.get_field(field).is_none() {
            errors.push(format!(
                "print format '{}' filename_field '{}' is unknown",
                format.name, field
            ));
        }
    }
    for total in &format.total_fields {
        if entity.get_field(total).is_none() {
            errors.push(format!(
                "print format '{}' total field '{}' is unknown",
                format.name, total
            ));
        }
    }
    if let Some(body) = &format.body {
        for err in crate::template::validate_template_paths(body, &format.entity, registry) {
            errors.push(format!("print format '{}': {err}", format.name));
        }
    }
    for (i, section) in format.sections.iter().enumerate() {
        if !section.kind.is_empty() && !PRINT_SECTION_KINDS.contains(&section.kind.as_str()) {
            errors.push(format!(
                "print format '{}' section {} has unknown kind '{}'",
                format.name,
                i + 1,
                section.kind
            ));
        }
        if let Some(rel) = &section.relation {
            let known =
                entity.get_field(rel).is_some() || entity.get_field(&format!("{rel}_id")).is_some();
            if !known {
                errors.push(format!(
                    "print format '{}' section has invalid relation '{rel}'",
                    format.name
                ));
            }
        }
        if let Some(loop_over) = &section.loop_over {
            let known = entity
                .fields
                .iter()
                .any(|f| f.name == *loop_over && f.is_child_table());
            if !known {
                errors.push(format!(
                    "print format '{}' has invalid loop '{}'",
                    format.name, loop_over
                ));
            }
        }
        if let Some(text) = &section.text {
            for err in crate::template::validate_template_paths(text, &format.entity, registry) {
                errors.push(format!("print format '{}': {err}", format.name));
            }
        }
        for field in &section.fields {
            for err in crate::template::validate_template_paths(
                &format!("{{{{ {field} }}}}"),
                &format.entity,
                registry,
            ) {
                errors.push(format!("print format '{}': {err}", format.name));
            }
        }
        if let Some(when) = &section.show_when {
            let path = when.split(['>', '<', '=', '!']).next().unwrap_or("").trim();
            if !path.is_empty() {
                for err in crate::template::validate_template_paths(
                    &format!("{{{{ {path} }}}}"),
                    &format.entity,
                    registry,
                ) {
                    errors.push(format!("print format '{}': {err}", format.name));
                }
            }
        }
    }
    errors
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDef {
    pub name: String,
    #[serde(default)]
    pub label: String,
    pub entity: String,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub group_by: Vec<String>,
    #[serde(default)]
    pub aggregations: HashMap<String, String>,
    #[serde(default)]
    pub chart: Option<String>,
    #[serde(default)]
    pub module: Option<String>,
    /// Default filters merged into every run. Same JSON as the report API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Value>,
}

impl ReportDef {
    pub fn new(name: impl Into<String>, entity: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: humanize(&name),
            entity: entity.into(),
            fields: Vec::new(),
            group_by: Vec::new(),
            aggregations: HashMap::new(),
            chart: None,
            module: None,
            filters: Vec::new(),
            name,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn fields(mut self, fields: &[&str]) -> Self {
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn group_by(mut self, fields: &[&str]) -> Self {
        self.group_by = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn sum(mut self, field: impl Into<String>) -> Self {
        self.aggregations.insert(field.into(), "SUM".into());
        self
    }

    pub fn count(mut self, field: impl Into<String>) -> Self {
        self.aggregations.insert(field.into(), "COUNT".into());
        self
    }

    pub fn avg(mut self, field: impl Into<String>) -> Self {
        self.aggregations.insert(field.into(), "AVG".into());
        self
    }

    pub fn min(mut self, field: impl Into<String>) -> Self {
        self.aggregations.insert(field.into(), "MIN".into());
        self
    }

    pub fn max(mut self, field: impl Into<String>) -> Self {
        self.aggregations.insert(field.into(), "MAX".into());
        self
    }

    pub fn chart(mut self, kind: impl Into<String>) -> Self {
        self.chart = Some(kind.into());
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }

    pub fn filter_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.filters.push(serde_json::json!({
            "field": field.into(),
            "op": "eq",
            "value": value.into(),
        }));
        self
    }
}

fn humanize(name: &str) -> String {
    name.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
