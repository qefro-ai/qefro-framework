use crate::error::{FieldError, QefroResult};
use crate::ident::snake_case;
use crate::ui::{UiFieldMeta, UiWhen, UiWidget};
use crate::validation::ValidationRules;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Supported field types. New variants can be added without breaking existing
/// entity definitions that serialize with `#[serde(tag = "type")]`.
///
/// Presentation is not a field type. Use [`FieldDef::ui`] / `widget` for that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    String,
    Text,
    Integer,
    Decimal,
    Boolean,
    #[serde(alias = "datetime")]
    DateTime,
    Date,
    Time,
    Uuid,
    Enum {
        values: Vec<String>,
    },
    Json,
    Relation,
    /// Nested child collection. Does not store a column on the parent.
    ChildTable,
}

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::DateTime => "datetime",
            Self::Date => "date",
            Self::Time => "time",
            Self::Uuid => "uuid",
            Self::Enum { .. } => "enum",
            Self::Json => "json",
            Self::Relation => "relation",
            Self::ChildTable => "child_table",
        }
    }

    pub fn sql_type(&self) -> &'static str {
        match self {
            Self::String | Self::Text | Self::Enum { .. } => "TEXT",
            Self::Integer => "BIGINT",
            Self::Decimal => "NUMERIC(18,6)",
            Self::Boolean => "BOOLEAN",
            Self::DateTime => "TIMESTAMPTZ",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Uuid | Self::Relation => "UUID",
            Self::Json | Self::ChildTable => "JSONB",
        }
    }

    pub fn default_widget(&self) -> UiWidget {
        match self {
            Self::String => UiWidget::Text,
            Self::Text => UiWidget::Textarea,
            Self::Integer | Self::Decimal => UiWidget::Number,
            Self::Boolean => UiWidget::Checkbox,
            Self::DateTime => UiWidget::DateTime,
            Self::Date => UiWidget::Date,
            Self::Time => UiWidget::Time,
            Self::Uuid => UiWidget::Text,
            Self::Enum { .. } => UiWidget::Select,
            Self::Json => UiWidget::Json,
            Self::Relation => UiWidget::Relation,
            Self::ChildTable => UiWidget::ChildTable,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Integer | Self::Decimal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    #[default]
    ManyToOne,
    OneToMany,
    ManyToMany,
    ChildTable,
}

/// Referential action when the related record is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDelete {
    /// Reject the delete if related rows exist (PostgreSQL default).
    #[default]
    Restrict,
    Cascade,
    SetNull,
}

impl OnDelete {
    pub fn sql_clause(self) -> &'static str {
        match self {
            Self::Restrict => "",
            Self::Cascade => " ON DELETE CASCADE",
            Self::SetNull => " ON DELETE SET NULL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationDef {
    pub target_entity: String,
    pub kind: RelationKind,
    /// For one-to-many, the field on the target that points back.
    #[serde(default)]
    pub inverse_field: Option<String>,
    #[serde(default)]
    pub on_delete: OnDelete,
    /// Unique many-to-one (one-to-one).
    #[serde(default)]
    pub unique: bool,
}

impl Default for RelationDef {
    fn default() -> Self {
        Self {
            target_entity: String::new(),
            kind: RelationKind::ManyToOne,
            inverse_field: None,
            on_delete: OnDelete::Restrict,
            unique: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(flatten)]
    pub field_type: FieldType,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub required: bool,
    /// Conditional required. Server-authoritative; the generic UI mirrors it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_when: Option<UiWhen>,
    #[serde(default)]
    pub unique: bool,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub indexed: bool,
    #[serde(default)]
    pub searchable: bool,
    /// Higher values rank earlier in global and entity search. Default 1.
    #[serde(default = "default_search_weight")]
    pub search_weight: i32,
    /// Match the whole field value, not a substring.
    #[serde(default)]
    pub search_exact: bool,
    /// Search through the related entity's searchable / display fields
    /// instead of ILIKE on the foreign key UUID.
    #[serde(default)]
    pub search_related: bool,
    #[serde(default)]
    pub default: Option<Value>,
    /// Dynamic default: `current_user`, `current_date`, `current_datetime`,
    /// `tenant_timezone`, `tenant_currency`.
    #[serde(default)]
    pub default_from: Option<String>,
    #[serde(default)]
    pub validation: ValidationRules,
    #[serde(default)]
    pub relation: Option<RelationDef>,
    #[serde(default)]
    pub ui: UiFieldMeta,
    /// System fields (id, tenant_id, timestamps) are not accepted from clients.
    #[serde(default)]
    pub system: bool,
    /// Server-calculated. Client writes are discarded.
    #[serde(default)]
    pub computed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// 0 = normal, 1 = restricted, 2 = sensitive, 3 = highly sensitive.
    #[serde(default)]
    pub permission_level: u8,
    /// Remains writable while the document is in a lock state.
    #[serde(default)]
    pub allow_on_submit: bool,
    /// Never returned by EntityService, search, audit payloads, or meta GET.
    /// Write-only on create/update (passwords).
    #[serde(default)]
    pub secret: bool,
    /// No database column. UI/action flags such as `create_account`.
    #[serde(default)]
    pub ephemeral: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildTableUi {
    #[serde(default = "default_true")]
    pub editable: bool,
    #[serde(default = "default_true")]
    pub addable: bool,
    #[serde(default = "default_true")]
    pub deletable: bool,
    #[serde(default = "default_true")]
    pub reorderable: bool,
    /// Visible child columns, in order. Empty means all form-visible fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
}

impl Default for ChildTableUi {
    fn default() -> Self {
        Self {
            editable: true,
            addable: true,
            deletable: true,
            reorderable: true,
            columns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildTableDef {
    pub name: String,
    pub child_entity: String,
    /// Foreign key on the child pointing at the parent. Defaults to `parent_id`.
    #[serde(default = "default_parent_id")]
    pub parent_field: String,
    #[serde(default = "default_true")]
    pub cascade_delete: bool,
    #[serde(default)]
    pub ui: ChildTableUi,
}

fn default_parent_id() -> String {
    "parent_id".into()
}

impl ChildTableDef {
    pub fn new(name: impl Into<String>, child_entity: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            child_entity: child_entity.into(),
            parent_field: default_parent_id(),
            cascade_delete: true,
            ui: ChildTableUi::default(),
        }
    }

    pub fn parent_field(mut self, name: impl Into<String>) -> Self {
        self.parent_field = name.into();
        self
    }

    pub fn cascade_delete(mut self, cascade: bool) -> Self {
        self.cascade_delete = cascade;
        self
    }

    pub fn columns(mut self, columns: &[&str]) -> Self {
        self.ui.columns = columns.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn editable(mut self, editable: bool) -> Self {
        self.ui.editable = editable;
        self
    }
}

fn default_true() -> bool {
    true
}

fn default_search_weight() -> i32 {
    1
}

impl FieldDef {
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        let name = name.into();
        let label = humanize(&name);
        let widget = field_type.default_widget().as_str().to_string();
        Self {
            name: name.clone(),
            field_type,
            label: label.clone(),
            required: false,
            required_when: None,
            unique: false,
            nullable: true,
            indexed: false,
            searchable: false,
            search_weight: 1,
            search_exact: false,
            search_related: false,
            default: None,
            default_from: None,
            validation: ValidationRules::default(),
            relation: None,
            ui: UiFieldMeta {
                label,
                widget,
                ..UiFieldMeta::default()
            },
            system: false,
            computed: false,
            formula: None,
            permission_level: 0,
            allow_on_submit: false,
            secret: false,
            ephemeral: false,
        }
    }

    pub fn string(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::String)
    }

    pub fn text(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Text)
    }

    pub fn integer(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Integer)
    }

    pub fn decimal(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Decimal)
    }

    /// Decimal stored as currency. `FieldDef::currency("subtotal").computed("...")`.
    pub fn currency(name: impl Into<String>) -> Self {
        Self::decimal(name).with_currency()
    }

    pub fn with_currency(mut self) -> Self {
        self.ui.widget = UiWidget::Currency.as_str().into();
        if self.ui.widget_options.precision.is_none() {
            self.ui.widget_options.precision = Some(2);
        }
        self
    }

    pub fn boolean(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Boolean)
    }

    pub fn datetime(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::DateTime)
    }

    pub fn date(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Date)
    }

    pub fn time(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Time)
    }

    pub fn uuid(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Uuid)
    }

    pub fn json(name: impl Into<String>) -> Self {
        Self::new(name, FieldType::Json)
    }

    pub fn enum_values(name: impl Into<String>, values: Vec<impl Into<String>>) -> Self {
        Self::new(
            name,
            FieldType::Enum {
                values: values.into_iter().map(Into::into).collect(),
            },
        )
    }

    /// Alias used in application definitions: `FieldDef::enum_("status", [...])`.
    pub fn enum_(name: impl Into<String>, values: Vec<impl Into<String>>) -> Self {
        Self::enum_values(name, values)
    }

    /// Many-to-one relation. `FieldDef::relation("customer", "Customer")`.
    pub fn relation(name: impl Into<String>, target: impl Into<String>) -> Self {
        Self::many_to_one(name, target)
    }

    pub fn many_to_one(name: impl Into<String>, target: impl Into<String>) -> Self {
        let target = target.into();
        Self::new(name, FieldType::Relation)
            .with_relation(RelationDef {
                target_entity: target.clone(),
                kind: RelationKind::ManyToOne,
                inverse_field: None,
                ..Default::default()
            })
            .indexed()
            .ui_display_entity(target)
    }

    /// Unique many-to-one (one-to-one).
    pub fn one_to_one(name: impl Into<String>, target: impl Into<String>) -> Self {
        let mut field = Self::many_to_one(name, target);
        field.unique = true;
        if let Some(rel) = field.relation.as_mut() {
            rel.unique = true;
        }
        field
    }

    pub fn on_delete(mut self, policy: OnDelete) -> Self {
        if let Some(rel) = self.relation.as_mut() {
            rel.on_delete = policy;
        }
        self
    }

    /// Generic assignment convention: `assigned_to` → User.
    pub fn assigned_to() -> Self {
        Self::many_to_one("assigned_to", "User")
            .label("Assigned to")
            .nullable()
            .filterable()
            .search_related()
    }

    pub fn one_to_many(
        name: impl Into<String>,
        target: impl Into<String>,
        inverse_field: impl Into<String>,
    ) -> Self {
        let target = target.into();
        let mut field = Self::new(name, FieldType::Relation).with_relation(RelationDef {
            target_entity: target,
            kind: RelationKind::OneToMany,
            inverse_field: Some(inverse_field.into()),
            ..Default::default()
        });
        field.ui.form = false;
        field.ui.list = false;
        field
    }

    pub fn many_to_many(name: impl Into<String>, target: impl Into<String>) -> Self {
        let mut field = Self::new(name, FieldType::Relation).with_relation(RelationDef {
            target_entity: target.into(),
            kind: RelationKind::ManyToMany,
            inverse_field: None,
            ..Default::default()
        });
        field.ui.form = true;
        field.ui.list = false;
        field.ui.widget = UiWidget::Relation.as_str().into();
        field
    }

    pub fn child_table_field(def: &ChildTableDef) -> Self {
        let mut field = Self::new(&def.name, FieldType::ChildTable).with_relation(RelationDef {
            target_entity: def.child_entity.clone(),
            kind: RelationKind::ChildTable,
            inverse_field: Some(def.parent_field.clone()),
            on_delete: OnDelete::Cascade,
            unique: false,
        });
        field.field_type = FieldType::ChildTable;
        field.ui.widget = UiWidget::ChildTable.as_str().into();
        field.ui.form = true;
        field.ui.list = false;
        field.ui.detail = true;
        field.ui.widget_options.entity = Some(def.child_entity.clone());
        field.ui.widget_options.editable = Some(def.ui.editable);
        field.ui.widget_options.addable = Some(def.ui.addable);
        field.ui.widget_options.deletable = Some(def.ui.deletable);
        field.ui.widget_options.reorderable = Some(def.ui.reorderable);
        if !def.ui.columns.is_empty() {
            field.ui.widget_options.column_fields = Some(def.ui.columns.clone());
        }
        field
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self.nullable = false;
        self
    }

    /// Require this field when `field` equals `equals`. Backend enforces it.
    pub fn required_when(mut self, field: impl Into<String>, equals: Value) -> Self {
        self.required_when = Some(UiWhen::new(field, equals));
        self
    }

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self.indexed = true;
        self
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self.required = false;
        self
    }

    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }

    pub fn search_weight(mut self, weight: i32) -> Self {
        self.searchable = true;
        self.search_weight = weight.max(1);
        self
    }

    pub fn search_exact(mut self) -> Self {
        self.searchable = true;
        self.search_exact = true;
        self
    }

    /// Search the related record's display / searchable fields (many-to-one).
    pub fn search_related(mut self) -> Self {
        self.searchable = true;
        self.search_related = true;
        self
    }

    pub fn permission_level(mut self, level: u8) -> Self {
        self.permission_level = level.min(3);
        self
    }

    pub fn allow_on_submit(mut self) -> Self {
        self.allow_on_submit = true;
        self
    }

    pub fn default_value(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    pub fn default_from(mut self, source: impl Into<String>) -> Self {
        self.default_from = Some(source.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = label.clone();
        self.ui.label = label;
        self
    }

    pub fn max_length(mut self, n: usize) -> Self {
        self.validation.max_length = Some(n);
        self
    }

    pub fn min_length(mut self, n: usize) -> Self {
        self.validation.min_length = Some(n);
        self
    }

    pub fn min(mut self, n: f64) -> Self {
        self.validation.min = Some(n);
        self.ui.widget_options.min = Some(Value::from(n));
        self
    }

    pub fn max(mut self, n: f64) -> Self {
        self.validation.max = Some(n);
        self.ui.widget_options.max = Some(Value::from(n));
        self
    }

    /// Exclusive lower bound. Inclusive bounds use [`Self::min`].
    pub fn greater_than(mut self, n: f64) -> Self {
        self.validation.greater_than = Some(n);
        self
    }

    /// Exclusive upper bound. Inclusive bounds use [`Self::max`].
    pub fn less_than(mut self, n: f64) -> Self {
        self.validation.less_than = Some(n);
        self
    }

    pub fn greater_or_equal(self, n: f64) -> Self {
        self.min(n)
    }

    pub fn less_or_equal(self, n: f64) -> Self {
        self.max(n)
    }

    pub fn range(self, min: f64, max: f64) -> Self {
        self.min(min).max(max)
    }

    pub fn email(mut self) -> Self {
        self.validation.email = true;
        self.ui.widget = UiWidget::Email.as_str().into();
        self
    }

    pub fn phone(mut self) -> Self {
        self.validation.phone = true;
        self.ui.widget = UiWidget::Phone.as_str().into();
        self
    }

    pub fn url(mut self) -> Self {
        self.validation.url = true;
        self.ui.widget = UiWidget::Url.as_str().into();
        self
    }

    pub fn color(mut self) -> Self {
        self.validation.color = true;
        self.ui.widget = UiWidget::Color.as_str().into();
        self
    }

    pub fn percentage(mut self) -> Self {
        self.ui.widget = UiWidget::Percentage.as_str().into();
        if self.validation.min.is_none() {
            self.validation.min = Some(0.0);
        }
        if self.validation.max.is_none() {
            self.validation.max = Some(100.0);
        }
        self
    }

    pub fn computed(mut self, formula: impl Into<String>) -> Self {
        self.computed = true;
        self.formula = Some(formula.into());
        self.ui.readonly = true;
        self.required = false;
        self.nullable = true;
        self
    }

    pub fn tags(mut self) -> Self {
        self.field_type = FieldType::Json;
        self.ui.widget = UiWidget::Tags.as_str().into();
        self
    }

    pub fn rich_text(mut self) -> Self {
        self.field_type = FieldType::Text;
        self.ui.widget = UiWidget::RichText.as_str().into();
        self
    }

    pub fn file(mut self) -> Self {
        self.ui.widget = UiWidget::File.as_str().into();
        self
    }

    pub fn image(mut self) -> Self {
        self.ui.widget = UiWidget::Image.as_str().into();
        self
    }

    pub fn regex(mut self, pattern: impl Into<String>) -> Self {
        self.validation.regex = Some(pattern.into());
        self
    }

    pub fn with_relation(mut self, relation: RelationDef) -> Self {
        self.relation = Some(relation.clone());
        if !matches!(self.field_type, FieldType::ChildTable) {
            self.field_type = FieldType::Relation;
        }
        let _ = relation;
        self
    }

    pub fn ui(mut self, mut ui: UiFieldMeta) -> Self {
        if ui.label.is_empty() {
            ui.label = self.ui.label.clone();
        } else {
            self.label = ui.label.clone();
        }
        // Preserve visibility defaults if the caller only set the widget.
        if ui.list && ui.form && ui.detail {
            ui.list = self.ui.list;
            ui.form = self.ui.form;
            ui.detail = self.ui.detail;
        }
        self.ui = ui;
        self
    }

    pub fn list(mut self, show: bool) -> Self {
        self.ui.list = show;
        self
    }

    pub fn detail(mut self, show: bool) -> Self {
        self.ui.detail = show;
        self
    }

    pub fn filterable(mut self) -> Self {
        self.ui.filter = true;
        self
    }

    pub fn sortable(mut self) -> Self {
        self.ui.sortable = true;
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.ui.placeholder = Some(text.into());
        self
    }

    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.ui.help = Some(text.into());
        self
    }

    pub fn section(mut self, name: impl Into<String>) -> Self {
        self.ui.section = Some(name.into());
        self
    }

    pub fn tab(mut self, name: impl Into<String>) -> Self {
        self.ui.tab = Some(name.into());
        self
    }

    pub fn width(mut self, width: impl Into<String>) -> Self {
        self.ui.width = Some(width.into());
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.ui.order = order;
        self
    }

    pub fn visible_when(mut self, field: impl Into<String>, equals: Value) -> Self {
        self.ui.visible_when = Some(UiWhen::new(field, equals));
        self
    }

    pub fn readonly_when(mut self, field: impl Into<String>, equals: Value) -> Self {
        self.ui.readonly_when = Some(UiWhen::new(field, equals));
        self
    }

    pub fn hidden(mut self) -> Self {
        self.ui.hidden = true;
        self.ui.form = false;
        self.ui.list = false;
        self.ui.detail = false;
        self
    }

    pub fn readonly(mut self) -> Self {
        self.ui.readonly = true;
        self
    }

    pub fn system(mut self) -> Self {
        self.system = true;
        self.ui.readonly = true;
        self
    }

    /// Write-only. Stripped from every outbound record. No database column.
    pub fn write_only(mut self) -> Self {
        self.secret = true;
        self.ephemeral = true;
        self.ui.list = false;
        self.ui.detail = false;
        self.ui.form = true;
        self.required = false;
        self.nullable = true;
        self
    }

    /// In-memory / request-only field. `apply_schema` does not add a column.
    pub fn ephemeral(mut self) -> Self {
        self.ephemeral = true;
        self.ui.list = false;
        self.ui.detail = false;
        self
    }

    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    fn ui_display_entity(mut self, entity: String) -> Self {
        self.ui.widget_options.entity = Some(entity);
        self
    }

    pub fn stores_column(&self) -> bool {
        if self.system {
            return true;
        }
        if self.secret || self.ephemeral {
            return false;
        }
        if matches!(self.field_type, FieldType::ChildTable) {
            return false;
        }
        match &self.relation {
            Some(rel)
                if matches!(
                    rel.kind,
                    RelationKind::OneToMany | RelationKind::ManyToMany | RelationKind::ChildTable
                ) =>
            {
                false
            }
            _ => true,
        }
    }

    pub fn is_child_table(&self) -> bool {
        matches!(self.field_type, FieldType::ChildTable)
            || matches!(
                self.relation.as_ref().map(|r| r.kind),
                Some(RelationKind::ChildTable)
            )
    }

    pub fn column_name(&self) -> String {
        snake_case(&self.name)
    }

    pub fn validate_name(&self) -> QefroResult<()> {
        crate::ident::assert_safe_ident(&self.column_name())?;
        Ok(())
    }

    pub fn is_rich_text(&self) -> bool {
        self.ui.widget == "rich_text"
    }

    pub fn type_error(&self, value: &Value) -> Option<FieldError> {
        if value.is_null() {
            return None;
        }
        let ok = match &self.field_type {
            FieldType::String
            | FieldType::Text
            | FieldType::Enum { .. }
            | FieldType::Date
            | FieldType::Time
            | FieldType::DateTime => value.is_string(),
            FieldType::Integer => value.is_i64() || value.is_u64(),
            FieldType::Decimal => value.is_number() || value.is_string(),
            FieldType::Boolean => value.is_boolean(),
            FieldType::Uuid | FieldType::Relation => {
                value.is_string() || (self.ui.widget == "multiselect" && value.is_array())
            }
            FieldType::Json => true,
            FieldType::ChildTable => value.is_array(),
        };
        if ok {
            None
        } else {
            Some(FieldError::new(
                &self.name,
                "invalid_type",
                format!("expected {}", self.field_type.as_str()),
            ))
        }
    }
}

fn humanize(name: &str) -> String {
    snake_case(name)
        .split('_')
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UiConfig;
    use serde_json::json;

    #[test]
    fn json_type_datetime_deserializes() {
        let field: FieldDef =
            serde_json::from_value(json!({"name": "rate", "type": "datetime"})).unwrap();
        assert_eq!(field.field_type.as_str(), "datetime");
    }

    #[test]
    fn field_builder_and_serde() {
        let field = FieldDef::string("email")
            .required()
            .unique()
            .email()
            .searchable();
        assert!(field.required);
        assert!(field.validation.email);
        assert_eq!(field.ui.widget, "email");
        let json = serde_json::to_value(&field).unwrap();
        let back: FieldDef = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "email");
    }

    #[test]
    fn data_type_is_independent_of_widget() {
        let price = FieldDef::currency("price");
        assert_eq!(price.field_type, FieldType::Decimal);
        assert_eq!(price.ui.widget, "currency");
        let color = FieldDef::string("brand_color").ui(UiConfig::color());
        assert_eq!(color.field_type, FieldType::String);
        assert_eq!(color.ui.widget, "color");
        let when = FieldDef::datetime("appointment_at").ui(UiConfig::datetime().tenant_timezone());
        assert_eq!(when.field_type, FieldType::DateTime);
        assert_eq!(when.ui.widget_options.timezone.as_deref(), Some("tenant"));
    }

    #[test]
    fn relation_alias_matches_docs() {
        let field = FieldDef::relation("customer", "Customer").required();
        assert_eq!(field.field_type, FieldType::Relation);
        assert_eq!(
            field.relation.as_ref().map(|r| r.target_entity.as_str()),
            Some("Customer")
        );
    }

    #[test]
    fn computed_fields_are_nullable_and_readonly() {
        let field = FieldDef::currency("amount").computed("quantity * rate");
        assert!(field.computed);
        assert!(field.nullable);
        assert!(!field.required);
        assert!(field.ui.readonly);
        assert_eq!(field.formula.as_deref(), Some("quantity * rate"));
    }

    #[test]
    fn time_sql_type() {
        assert_eq!(FieldType::Time.sql_type(), "TIME");
        let field = FieldDef::time("reservation_time");
        assert_eq!(field.ui.widget, "time");
        assert!(field.type_error(&json!("18:30")).is_none());
        assert!(field.type_error(&json!(18)).is_some());
    }

    #[test]
    fn on_delete_and_one_to_one() {
        let restrict = FieldDef::many_to_one("customer_id", "Customer");
        assert_eq!(
            restrict.relation.as_ref().unwrap().on_delete,
            OnDelete::Restrict
        );
        let cascade = FieldDef::many_to_one("order_id", "Order").on_delete(OnDelete::Cascade);
        assert_eq!(
            cascade.relation.as_ref().unwrap().on_delete,
            OnDelete::Cascade
        );
        let pair = FieldDef::one_to_one("profile_id", "Profile");
        assert!(pair.unique);
        assert!(pair.relation.as_ref().unwrap().unique);
        assert_eq!(OnDelete::SetNull.sql_clause(), " ON DELETE SET NULL");
    }
}
