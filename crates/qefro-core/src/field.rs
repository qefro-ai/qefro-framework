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
    DateTime,
    Date,
    Time,
    Uuid,
    Enum { values: Vec<String> },
    Json,
    Relation,
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
            Self::Json => "JSONB",
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    ManyToOne,
    OneToMany,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationDef {
    pub target_entity: String,
    pub kind: RelationKind,
    /// For one-to-many, the field on the target that points back.
    #[serde(default)]
    pub inverse_field: Option<String>,
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
    #[serde(default)]
    pub unique: bool,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub indexed: bool,
    #[serde(default)]
    pub searchable: bool,
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
}

fn default_true() -> bool {
    true
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
            unique: false,
            nullable: true,
            indexed: false,
            searchable: false,
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
            })
            .indexed()
            .ui_display_entity(target)
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
        });
        field.ui.form = true;
        field.ui.list = false;
        field.ui.widget = UiWidget::Relation.as_str().into();
        field
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self.nullable = false;
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

    pub fn currency(mut self) -> Self {
        self.ui.widget = UiWidget::Currency.as_str().into();
        if self.ui.widget_options.precision.is_none() {
            self.ui.widget_options.precision = Some(2);
        }
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
        self.relation = Some(relation);
        self.field_type = FieldType::Relation;
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

    fn ui_display_entity(mut self, entity: String) -> Self {
        self.ui.widget_options.entity = Some(entity);
        self
    }

    pub fn stores_column(&self) -> bool {
        if self.system {
            return true;
        }
        match &self.relation {
            Some(rel) if matches!(rel.kind, RelationKind::OneToMany | RelationKind::ManyToMany) => {
                false
            }
            _ => true,
        }
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
        let price = FieldDef::decimal("price").currency();
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
    fn time_sql_type() {
        assert_eq!(FieldType::Time.sql_type(), "TIME");
        let field = FieldDef::time("reservation_time");
        assert_eq!(field.ui.widget, "time");
        assert!(field.type_error(&json!("18:30")).is_none());
        assert!(field.type_error(&json!(18)).is_some());
    }
}
