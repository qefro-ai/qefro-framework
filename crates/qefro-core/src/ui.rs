use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiWidget {
    #[default]
    Text,
    Textarea,
    Number,
    Checkbox,
    Select,
    Date,
    DateTime,
    Email,
    Relation,
    Json,
    Boolean,
}

impl UiWidget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Textarea => "textarea",
            Self::Number => "number",
            Self::Checkbox | Self::Boolean => "boolean",
            Self::Select => "select",
            Self::Date => "date",
            Self::DateTime => "datetime",
            Self::Email => "email",
            Self::Relation => "relation",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiFieldMeta {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub widget: UiWidget,
    #[serde(default = "default_true", alias = "list_visible")]
    pub list: bool,
    #[serde(default = "default_true", alias = "form_visible")]
    pub form: bool,
    #[serde(default)]
    pub filter: bool,
    #[serde(default)]
    pub sortable: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub order: i32,
}

fn default_true() -> bool {
    true
}

impl Default for UiFieldMeta {
    fn default() -> Self {
        Self {
            label: String::new(),
            description: None,
            widget: UiWidget::Text,
            list: true,
            form: true,
            filter: false,
            sortable: false,
            readonly: false,
            hidden: false,
            placeholder: None,
            help: None,
            section: None,
            width: None,
            order: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiEntityMeta {
    pub entity: String,
    pub label: String,
    pub label_plural: String,
    pub slug: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub searchable: bool,
    pub workflow: Option<String>,
    pub display_field: String,
    pub module: Option<String>,
    pub fields: Vec<UiFieldView>,
}

impl UiEntityMeta {
    pub fn apply_terminology(&mut self, terms: &std::collections::HashMap<String, String>) {
        if let Some(label) = terms.get(&self.entity).or_else(|| terms.get(&self.label)) {
            self.label = label.clone();
        }
        let plural_key = format!("{}.plural", self.entity);
        if let Some(plural) = terms.get(&plural_key).or_else(|| terms.get(&self.label_plural)) {
            self.label_plural = plural.clone();
        } else if terms.contains_key(&self.entity) {
            self.label_plural = format!("{}s", self.label);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFieldView {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub required: bool,
    pub list: bool,
    pub list_visible: bool,
    pub form: bool,
    pub form_visible: bool,
    pub filter: bool,
    pub filterable: bool,
    pub searchable: bool,
    pub sortable: bool,
    pub hidden: bool,
    pub widget: UiWidget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    pub order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse_field: Option<String>,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardCard {
    pub title: String,
    pub entity: String,
    /// `count` or `sum`
    pub metric: String,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub filters: Vec<DashboardFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardFilter {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardDef {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub cards: Vec<DashboardCard>,
}

impl DashboardDef {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            module: None,
            cards: Vec::new(),
        }
    }

    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn card(mut self, card: DashboardCard) -> Self {
        self.cards.push(card);
        self
    }
}

impl DashboardCard {
    pub fn count(title: impl Into<String>, entity: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entity: entity.into(),
            metric: "count".into(),
            field: None,
            filters: Vec::new(),
        }
    }

    pub fn sum(title: impl Into<String>, entity: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entity: entity.into(),
            metric: "sum".into(),
            field: Some(field.into()),
            filters: Vec::new(),
        }
    }

    pub fn filter(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push(DashboardFilter {
            field: field.into(),
            value: value.into(),
        });
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantBranding {
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub primary_color: Option<String>,
    #[serde(default)]
    pub secondary_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<String>,
    #[serde(default)]
    pub company_name: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
}

impl TenantBranding {
    pub fn display_name(&self) -> Option<&str> {
        self.company_name.as_deref().or(self.app_name.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantUiConfig {
    #[serde(default)]
    pub navigation: Vec<String>,
    #[serde(default)]
    pub hidden_entities: Vec<String>,
    #[serde(default)]
    pub default_dashboard: Option<String>,
    /// Entity name → presentation label. Underlying entity names stay stable.
    #[serde(default)]
    pub terminology: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantBusinessConfig {
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_tz")]
    pub timezone: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_date_format")]
    pub date_format: String,
    #[serde(default = "default_number_format")]
    pub number_format: String,
}

fn default_currency() -> String {
    "USD".into()
}
fn default_tz() -> String {
    "UTC".into()
}
fn default_locale() -> String {
    "en-US".into()
}
fn default_date_format() -> String {
    "YYYY-MM-DD".into()
}
fn default_number_format() -> String {
    "1,234.56".into()
}

impl Default for TenantBusinessConfig {
    fn default() -> Self {
        Self {
            currency: default_currency(),
            timezone: default_tz(),
            locale: default_locale(),
            date_format: default_date_format(),
            number_format: default_number_format(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantFeatures {
    #[serde(default)]
    pub flags: std::collections::HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantConfig {
    #[serde(default)]
    pub branding: TenantBranding,
    #[serde(default)]
    pub ui_config: TenantUiConfig,
    #[serde(default)]
    pub enabled_apps: Vec<String>,
    #[serde(default)]
    pub business: TenantBusinessConfig,
    /// Legacy JSON bag. Prefer `business` for typed fields.
    #[serde(default)]
    pub business_config: serde_json::Value,
    #[serde(default)]
    pub features: TenantFeatures,
    #[serde(default)]
    pub plan: Option<String>,
}
