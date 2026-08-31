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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintFormat {
    pub name: String,
    pub entity: String,
    #[serde(default)]
    pub title: Option<String>,
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
}

fn default_true() -> bool {
    true
}

impl PrintFormat {
    pub fn new(name: impl Into<String>, entity: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity: entity.into(),
            title: None,
            header: true,
            items: true,
            totals: true,
            footer: true,
            item_table: None,
            total_fields: Vec::new(),
            module: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
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
