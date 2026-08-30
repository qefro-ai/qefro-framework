use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Known widget names. Custom applications may register additional names
/// as strings without changing this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiWidget {
    #[default]
    Text,
    Textarea,
    Number,
    Currency,
    Percentage,
    Checkbox,
    Switch,
    Radio,
    Select,
    Multiselect,
    Date,
    Time,
    DateTime,
    Color,
    Email,
    Phone,
    Url,
    Relation,
    Tags,
    RichText,
    File,
    Image,
    Json,
    Boolean,
    ChildTable,
}

impl UiWidget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Textarea => "textarea",
            Self::Number => "number",
            Self::Currency => "currency",
            Self::Percentage => "percentage",
            Self::Checkbox => "checkbox",
            Self::Switch => "switch",
            Self::Radio => "radio",
            Self::Select => "select",
            Self::Multiselect => "multiselect",
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "datetime",
            Self::Color => "color",
            Self::Email => "email",
            Self::Phone => "phone",
            Self::Url => "url",
            Self::Relation => "relation",
            Self::Tags => "tags",
            Self::RichText => "rich_text",
            Self::File => "file",
            Self::Image => "image",
            Self::Json => "json",
            Self::Boolean => "checkbox",
            Self::ChildTable => "child_table",
        }
    }
}

/// Presentation options. Independent of the field's storage type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WidgetOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u32>,
    /// `tenant`, `utc`, or an IANA timezone name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour12: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minute_step: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_selections: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_fields: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_create: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reorderable: Option<bool>,
    /// Child-table column field names. Distinct from `columns` (layout count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_fields: Option<Vec<String>>,
}

/// Presentation-only condition. Server validation still applies when hidden.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiWhen {
    pub field: String,
    pub equals: Value,
}

impl UiWhen {
    pub fn new(field: impl Into<String>, equals: impl Into<Value>) -> Self {
        Self {
            field: field.into(),
            equals: equals.into(),
        }
    }

    pub fn matches(&self, record: &Value) -> bool {
        crate::condition::values_equal(crate::condition::lookup(record, &self.field), &self.equals)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiFieldMeta {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Widget registry key. Not a database type. Custom names are allowed.
    #[serde(default = "default_widget")]
    pub widget: String,
    #[serde(default)]
    pub widget_options: WidgetOptions,
    #[serde(default = "default_true", alias = "list_visible")]
    pub list: bool,
    #[serde(default = "default_true", alias = "form_visible")]
    pub form: bool,
    #[serde(default = "default_true", alias = "detail_visible")]
    pub detail: bool,
    #[serde(default)]
    pub filter: bool,
    #[serde(default)]
    pub sortable: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default, alias = "help_text")]
    pub help: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<UiWhen>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "read_only_when"
    )]
    pub readonly_when: Option<UiWhen>,
}

fn default_true() -> bool {
    true
}

fn default_widget() -> String {
    "text".into()
}

impl Default for UiFieldMeta {
    fn default() -> Self {
        Self {
            label: String::new(),
            description: None,
            widget: default_widget(),
            widget_options: WidgetOptions::default(),
            list: true,
            form: true,
            detail: true,
            filter: false,
            sortable: false,
            readonly: false,
            disabled: false,
            hidden: false,
            placeholder: None,
            help: None,
            section: None,
            tab: None,
            width: None,
            order: 0,
            visible_when: None,
            readonly_when: None,
        }
    }
}

impl UiFieldMeta {
    pub fn widget(name: impl Into<String>) -> Self {
        Self {
            widget: name.into(),
            ..Default::default()
        }
    }

    pub fn tenant_timezone(mut self) -> Self {
        self.widget_options.timezone = Some("tenant".into());
        self
    }

    pub fn utc_timezone(mut self) -> Self {
        self.widget_options.timezone = Some("utc".into());
        self
    }

    pub fn currency_code(mut self, code: impl Into<String>) -> Self {
        self.widget_options.currency = Some(code.into());
        self
    }

    pub fn precision(mut self, digits: u32) -> Self {
        self.widget_options.precision = Some(digits);
        self
    }

    pub fn display_field(mut self, name: impl Into<String>) -> Self {
        self.widget_options.display_field = Some(name.into());
        self
    }

    pub fn minute_step(mut self, step: u32) -> Self {
        self.widget_options.minute_step = Some(step);
        self
    }

    pub fn max_size(mut self, bytes: u64) -> Self {
        self.widget_options.max_size = Some(bytes);
        self
    }

    pub fn accept(mut self, types: Vec<impl Into<String>>) -> Self {
        self.widget_options.accept = Some(types.into_iter().map(Into::into).collect());
        self
    }
}

/// Ergonomic constructors used in entity definitions.
///
/// ```ignore
/// FieldDef::datetime("created_at").ui(UiConfig::datetime().tenant_timezone())
/// FieldDef::string("brand_color").ui(UiConfig::color())
/// FieldDef::decimal("price").ui(UiConfig::currency().precision(2))
/// ```
pub struct UiConfig;

impl UiConfig {
    pub fn text() -> UiFieldMeta {
        UiFieldMeta::widget("text")
    }
    pub fn textarea() -> UiFieldMeta {
        UiFieldMeta::widget("textarea")
    }
    pub fn number() -> UiFieldMeta {
        UiFieldMeta::widget("number")
    }
    pub fn currency() -> UiFieldMeta {
        UiFieldMeta::widget("currency")
    }
    pub fn percentage() -> UiFieldMeta {
        UiFieldMeta::widget("percentage")
    }
    pub fn date() -> UiFieldMeta {
        UiFieldMeta::widget("date")
    }
    pub fn time() -> UiFieldMeta {
        UiFieldMeta::widget("time")
    }
    pub fn datetime() -> UiFieldMeta {
        UiFieldMeta::widget("datetime")
    }
    pub fn color() -> UiFieldMeta {
        UiFieldMeta::widget("color")
    }
    pub fn select() -> UiFieldMeta {
        UiFieldMeta::widget("select")
    }
    pub fn multiselect() -> UiFieldMeta {
        UiFieldMeta::widget("multiselect")
    }
    pub fn relation() -> UiFieldMeta {
        UiFieldMeta::widget("relation")
    }
    pub fn checkbox() -> UiFieldMeta {
        UiFieldMeta::widget("checkbox")
    }
    pub fn switch() -> UiFieldMeta {
        UiFieldMeta::widget("switch")
    }
    pub fn radio() -> UiFieldMeta {
        UiFieldMeta::widget("radio")
    }
    pub fn tags() -> UiFieldMeta {
        UiFieldMeta::widget("tags")
    }
    pub fn phone() -> UiFieldMeta {
        UiFieldMeta::widget("phone")
    }
    pub fn email() -> UiFieldMeta {
        UiFieldMeta::widget("email")
    }
    pub fn url() -> UiFieldMeta {
        UiFieldMeta::widget("url")
    }
    pub fn rich_text() -> UiFieldMeta {
        UiFieldMeta::widget("rich_text")
    }
    pub fn file() -> UiFieldMeta {
        UiFieldMeta::widget("file")
    }
    pub fn image() -> UiFieldMeta {
        UiFieldMeta::widget("image")
    }
    pub fn json() -> UiFieldMeta {
        UiFieldMeta::widget("json")
    }
    pub fn password() -> UiFieldMeta {
        UiFieldMeta::widget("password")
    }
    /// Application-specific widget registered on the frontend.
    pub fn named(name: impl Into<String>) -> UiFieldMeta {
        UiFieldMeta::widget(name)
    }
}

pub const UI_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiEntityMeta {
    #[serde(default = "schema_version")]
    pub schema_version: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<String>,
    #[serde(default = "default_true_standalone")]
    pub standalone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<crate::document::DocumentConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<crate::document::NamingConfig>,
    #[serde(default)]
    pub singleton: bool,
    #[serde(default)]
    pub attachments: bool,
    /// Metadata-driven capability discovery for the generic UI. Additive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<EntityCapabilities>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::platform::EntityActionDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<crate::platform::LinkDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_form: Option<crate::platform::PublicFormDef>,
    /// Presentation-only view configuration. Omitted entities use automatic defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<EntityViews>,
    /// Session-scoped chrome hints. Server still authorizes writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<EntityPermissions>,
}

/// Per-user entity capabilities for UI chrome. Not a grant dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityPermissions {
    pub list: bool,
    pub create: bool,
    pub read: bool,
    pub update: bool,
    pub delete: bool,
    #[serde(default)]
    pub export: bool,
}

/// Which business-object surfaces the generic UI may offer. Not authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EntityCapabilities {
    #[serde(default)]
    pub workflow: bool,
    #[serde(default)]
    pub activity: bool,
    #[serde(default)]
    pub comments: bool,
    #[serde(default)]
    pub attachments: bool,
    #[serde(default)]
    pub audit: bool,
    #[serde(default)]
    pub relations: bool,
    #[serde(default)]
    pub actions: bool,
    #[serde(default)]
    pub archive: bool,
    #[serde(default)]
    pub assignment: bool,
    #[serde(default)]
    pub import: bool,
    #[serde(default)]
    pub export: bool,
    #[serde(default)]
    pub bulk: bool,
}

fn default_true_standalone() -> bool {
    true
}

fn schema_version() -> String {
    UI_SCHEMA_VERSION.into()
}

/// Additive view metadata. `schema_version` stays `"1"`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityViews {
    /// Default collection view: `list`, `card`, `kanban`, `calendar`, or `chart`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<ListViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<CardViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<FormViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kanban: Option<KanbanViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<CalendarViewSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartViewSpec>,
}

/// Generic chart view. Renderer must not branch on entity name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChartViewSpec {
    #[serde(default = "default_view_enabled")]
    pub enabled: bool,
    /// `bar`, `line`, `area`, `pie`, `donut`.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub chart_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<ChartMeasureSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChartMeasureSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// `count`, `sum`, `avg`, `min`, `max`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListViewSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ListColumnSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sort: Option<SortSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListColumnSpec {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortSpec {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FormViewSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<ViewSectionSpec>,
}

impl FormViewSpec {
    pub fn sections(sections: Vec<ViewSectionSpec>) -> Self {
        Self { sections }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DetailViewSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<ViewSectionSpec>,
}

impl DetailViewSpec {
    pub fn sections(sections: Vec<ViewSectionSpec>) -> Self {
        Self { sections }
    }
}

/// One column inside a section. Fields stack within the column; the renderer
/// places columns side-by-side on desktop and stacks them on mobile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewColumnSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

impl ViewColumnSpec {
    pub fn fields(fields: &[&str]) -> Self {
        Self {
            fields: fields.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewSectionSpec {
    pub title: String,
    /// Flat field list. Used when `columns` is empty. Kept for compatibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    /// Optional two- (or more) column grouping. Renderer stacks on mobile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ViewColumnSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<UiWhen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
}

impl ViewSectionSpec {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn fields(mut self, fields: &[&str]) -> Self {
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn columns(mut self, columns: &[ViewColumnSpec]) -> Self {
        self.columns = columns.to_vec();
        self
    }

    pub fn tab(mut self, tab: impl Into<String>) -> Self {
        self.tab = Some(tab.into());
        self
    }

    pub fn visible_when(mut self, field: impl Into<String>, equals: impl Into<Value>) -> Self {
        self.visible_when = Some(UiWhen::new(field, equals));
        self
    }

    pub fn collapsed(mut self) -> Self {
        self.collapsed = Some(true);
        self
    }

    /// Field names in layout order, from columns or the flat list.
    pub fn field_names(&self) -> Vec<&str> {
        if self.columns.is_empty() {
            return self.fields.iter().map(String::as_str).collect();
        }
        self.columns
            .iter()
            .flat_map(|c| c.fields.iter().map(String::as_str))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KanbanViewSpec {
    #[serde(default = "default_view_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<KanbanCardSpec>,
}

impl Default for KanbanViewSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            group_by: None,
            card: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KanbanCardSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

/// Opt-in collection card view. Omitted entities do not show a Cards tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardViewSpec {
    #[serde(default = "default_view_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Image, file, or relation field name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

impl Default for CardViewSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            title: None,
            subtitle: None,
            image: None,
            fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarViewSpec {
    #[serde(default = "default_view_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
}

impl Default for CalendarViewSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            start: None,
            end: None,
            time: None,
            title: None,
            subtitle: None,
        }
    }
}

fn default_view_enabled() -> bool {
    true
}

impl UiEntityMeta {
    pub fn apply_terminology(&mut self, terms: &std::collections::HashMap<String, String>) {
        if let Some(label) = terms.get(&self.entity).or_else(|| terms.get(&self.label)) {
            self.label = label.clone();
        }
        let plural_key = format!("{}.plural", self.entity);
        if let Some(plural) = terms
            .get(&plural_key)
            .or_else(|| terms.get(&self.label_plural))
        {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_when: Option<UiWhen>,
    pub list: bool,
    pub list_visible: bool,
    pub form: bool,
    pub form_visible: bool,
    pub detail: bool,
    pub detail_visible: bool,
    pub filter: bool,
    pub filterable: bool,
    pub searchable: bool,
    #[serde(default = "default_search_weight_view")]
    pub search_weight: i32,
    #[serde(default)]
    pub search_exact: bool,
    pub sortable: bool,
    pub hidden: bool,
    pub disabled: bool,
    pub widget: String,
    #[serde(default, skip_serializing_if = "widget_options_empty")]
    pub widget_options: WidgetOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<UiWhen>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "read_only_when")]
    pub readonly_when: Option<UiWhen>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_from: Option<String>,
    #[serde(default)]
    pub computed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(default)]
    pub permission_level: u8,
    #[serde(default)]
    pub allow_on_submit: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_entity: Option<String>,
}

fn widget_options_empty(opts: &WidgetOptions) -> bool {
    opts == &WidgetOptions::default()
}

fn default_search_weight_view() -> i32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardCard {
    pub title: String,
    pub entity: String,
    /// `count`, `sum`, `avg`, `min`, `max`. Charts use `group_by`.
    #[serde(default = "default_metric")]
    pub metric: String,
    /// `metric`/`kpi`, `table`, `chart`, `list`, `status_breakdown`/`workflow`,
    /// `activity`, `saved_view`, `report`, `audit`.
    #[serde(default = "default_card_kind")]
    pub kind: String,
    /// `bar`, `line`, `area`, `pie`, `donut`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub filters: Vec<DashboardFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Layout hint: `sm`, `md`, `lg`, `xl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// When non-empty, only these roles see the card. Others skip it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// Saved view name for `saved_view` widgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_view: Option<String>,
    /// Report name for `report` widgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
}

fn default_metric() -> String {
    "count".into()
}

fn default_card_kind() -> String {
    "metric".into()
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
    fn base(
        title: impl Into<String>,
        entity: impl Into<String>,
        metric: impl Into<String>,
        kind: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            entity: entity.into(),
            metric: metric.into(),
            kind: kind.into(),
            chart: None,
            group_by: None,
            field: None,
            filters: Vec::new(),
            limit: None,
            size: None,
            roles: Vec::new(),
            saved_view: None,
            report: None,
        }
    }

    pub fn count(title: impl Into<String>, entity: impl Into<String>) -> Self {
        Self::base(title, entity, "count", "metric")
    }

    pub fn kpi(title: impl Into<String>, entity: impl Into<String>) -> Self {
        Self::base(title, entity, "count", "kpi")
    }

    pub fn sum(
        title: impl Into<String>,
        entity: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        let mut card = Self::base(title, entity, "sum", "metric");
        card.field = Some(field.into());
        card
    }

    pub fn chart(
        title: impl Into<String>,
        entity: impl Into<String>,
        chart: impl Into<String>,
        group_by: impl Into<String>,
    ) -> Self {
        let mut card = Self::base(title, entity, "count", "chart");
        card.chart = Some(chart.into());
        card.group_by = Some(group_by.into());
        card
    }

    pub fn status_breakdown(
        title: impl Into<String>,
        entity: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        let mut card = Self::base(title, entity, "count", "status_breakdown");
        card.chart = Some("bar".into());
        card.group_by = Some(field.into());
        card
    }

    pub fn workflow(title: impl Into<String>, entity: impl Into<String>) -> Self {
        let mut card = Self::status_breakdown(title, entity, "status");
        card.kind = "workflow".into();
        card
    }

    pub fn recent(title: impl Into<String>, entity: impl Into<String>, limit: u32) -> Self {
        let mut card = Self::base(title, entity, "count", "list");
        card.limit = Some(limit);
        card
    }

    pub fn table(title: impl Into<String>, entity: impl Into<String>, limit: u32) -> Self {
        let mut card = Self::recent(title, entity, limit);
        card.kind = "table".into();
        card
    }

    pub fn activity(title: impl Into<String>, entity: impl Into<String>, limit: u32) -> Self {
        let mut card = Self::base(title, entity, "count", "activity");
        card.limit = Some(limit);
        card
    }

    pub fn saved_view(
        title: impl Into<String>,
        entity: impl Into<String>,
        view: impl Into<String>,
    ) -> Self {
        let mut card = Self::base(title, entity, "count", "saved_view");
        card.saved_view = Some(view.into());
        card.limit = Some(8);
        card
    }

    pub fn report_card(
        title: impl Into<String>,
        entity: impl Into<String>,
        report: impl Into<String>,
    ) -> Self {
        let mut card = Self::base(title, entity, "count", "report");
        card.report = Some(report.into());
        card
    }

    pub fn audit(title: impl Into<String>) -> Self {
        Self::base(title, "_audit", "count", "audit")
    }

    pub fn filter(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push(DashboardFilter {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn size(mut self, size: impl Into<String>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn measure_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn metric_name(mut self, metric: impl Into<String>) -> Self {
        self.metric = metric.into();
        self
    }
}

/// Workspace navigation item. Labels come from the app module, not hardcoded restaurant names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceNavItem {
    pub label: String,
    pub entity: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Workspace section heading, e.g. Operations / Catalog / Analytics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
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

    fn blank(value: &Option<String>) -> bool {
        value.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true)
    }

    pub fn is_empty(&self) -> bool {
        Self::blank(&self.logo)
            && Self::blank(&self.favicon)
            && Self::blank(&self.primary_color)
            && Self::blank(&self.secondary_color)
            && Self::blank(&self.accent_color)
            && Self::blank(&self.company_name)
            && Self::blank(&self.app_name)
    }

    /// Fill empty fields from app or other defaults. Stored tenant values win.
    pub fn fill_from(&mut self, other: &Self) {
        if Self::blank(&self.logo) {
            self.logo = other.logo.clone();
        }
        if Self::blank(&self.favicon) {
            self.favicon = other.favicon.clone();
        }
        if Self::blank(&self.primary_color) {
            self.primary_color = other.primary_color.clone();
        }
        if Self::blank(&self.secondary_color) {
            self.secondary_color = other.secondary_color.clone();
        }
        if Self::blank(&self.accent_color) {
            self.accent_color = other.accent_color.clone();
        }
        if Self::blank(&self.company_name) {
            self.company_name = other.company_name.clone();
        }
        if Self::blank(&self.app_name) {
            self.app_name = other.app_name.clone();
        }
    }

    pub fn apply_tenant_name(&mut self, name: Option<&str>) {
        if Self::blank(&self.company_name) {
            if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
                self.company_name = Some(name.to_string());
            }
        }
    }

    /// Overlay enabled-app defaults, then the tenant display name, onto stored branding.
    pub fn resolve(
        stored: &Self,
        app_defaults: impl IntoIterator<Item = Self>,
        tenant_name: Option<&str>,
    ) -> Self {
        let mut defaults = Self::default();
        for other in app_defaults {
            defaults.fill_from(&other);
        }
        let mut branding = stored.clone();
        branding.fill_from(&defaults);
        branding.apply_tenant_name(tenant_name);
        branding
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
    /// Chart of accounts codes for this tenant. Never hardcoded account IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash_account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receivable_account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payable_account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sales_account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cogs_account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory_account: Option<String>,
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
            cash_account: None,
            receivable_account: None,
            payable_account: None,
            sales_account: None,
            cogs_account: None,
            inventory_account: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn visible_when_is_presentation_only() {
        let when = UiWhen::new("status", json!("Cancelled"));
        assert!(when.matches(&json!({"status": "Cancelled"})));
        assert!(!when.matches(&json!({"status": "Pending"})));
    }

    #[test]
    fn widget_is_a_string_so_apps_can_register_custom_names() {
        let meta = UiConfig::named("table-status");
        assert_eq!(meta.widget, "table-status");
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["widget"], "table-status");
    }

    #[test]
    fn branding_overlay_prefers_stored_then_app_then_tenant_name() {
        let stored = TenantBranding {
            company_name: Some("Seeni Bhai".into()),
            ..Default::default()
        };
        let app = TenantBranding {
            company_name: Some("Qefro Kitchen".into()),
            app_name: Some("Restaurant".into()),
            primary_color: Some("#9a3412".into()),
            accent_color: Some("#c2410c".into()),
            secondary_color: Some("#f4f1ea".into()),
            logo: Some("data:image/svg+xml,app".into()),
            favicon: Some("data:image/svg+xml,app".into()),
            ..Default::default()
        };
        let resolved = TenantBranding::resolve(&stored, [app], Some("Ignored Name"));
        assert_eq!(resolved.company_name.as_deref(), Some("Seeni Bhai"));
        assert_eq!(resolved.app_name.as_deref(), Some("Restaurant"));
        assert_eq!(resolved.primary_color.as_deref(), Some("#9a3412"));
        assert_eq!(resolved.accent_color.as_deref(), Some("#c2410c"));
        let empty = TenantBranding::resolve(&TenantBranding::default(), [], Some("Demo Kitchen"));
        assert_eq!(empty.company_name.as_deref(), Some("Demo Kitchen"));
        assert!(TenantBranding::default().is_empty());
    }

    #[test]
    fn card_view_is_omitted_when_unset_and_schema_stays_one() {
        let views = EntityViews::default();
        let json = serde_json::to_value(&views).unwrap();
        assert!(json.get("card").is_none());
        assert_eq!(UI_SCHEMA_VERSION, "1");
        let card = CardViewSpec {
            enabled: true,
            title: Some("name".into()),
            subtitle: Some("status".into()),
            image: None,
            fields: vec!["status".into()],
        };
        let round =
            serde_json::from_value::<CardViewSpec>(serde_json::to_value(&card).unwrap()).unwrap();
        assert_eq!(round.title.as_deref(), Some("name"));
        assert!(round.enabled);
    }

    #[test]
    fn read_only_when_alias_round_trips() {
        let meta: UiFieldMeta = serde_json::from_value(json!({
            "label": "Customer",
            "read_only_when": { "field": "status", "equals": "completed" }
        }))
        .unwrap();
        assert_eq!(meta.readonly_when.as_ref().unwrap().field, "status");
        assert_eq!(UI_SCHEMA_VERSION, "1");
    }

    #[test]
    fn view_section_columns_are_additive() {
        let section = ViewSectionSpec::new("Customer Information")
            .columns(&[
                ViewColumnSpec::fields(&["name", "email", "phone"]),
                ViewColumnSpec::fields(&["party_type", "person_id"]),
            ])
            .tab("Details");
        let json = serde_json::to_value(&section).unwrap();
        assert_eq!(json["title"], "Customer Information");
        assert_eq!(json["tab"], "Details");
        assert_eq!(json["columns"][0]["fields"][0], "name");
        let names = section.field_names();
        assert_eq!(
            names,
            vec!["name", "email", "phone", "party_type", "person_id"]
        );
        let legacy: ViewSectionSpec = serde_json::from_value(json!({
            "title": "Customer",
            "fields": ["name", "email"]
        }))
        .unwrap();
        assert!(legacy.columns.is_empty());
        assert_eq!(legacy.field_names(), vec!["name", "email"]);
    }
}
