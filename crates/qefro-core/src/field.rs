use crate::error::{FieldError, QefroResult};
use crate::ident::snake_case;
use crate::ui::{UiFieldMeta, UiWidget};
use crate::validation::ValidationRules;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Supported field types. New variants can be added without breaking existing
/// entity definitions that serialize with `#[serde(tag = "type")]`.
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
            Self::Uuid | Self::Relation => "UUID",
            Self::Json => "JSONB",
        }
    }

    pub fn default_widget(&self) -> UiWidget {
        match self {
            Self::String => UiWidget::Text,
            Self::Text => UiWidget::Textarea,
            Self::Integer | Self::Decimal => UiWidget::Number,
            Self::Boolean => UiWidget::Boolean,
            Self::DateTime => UiWidget::DateTime,
            Self::Date => UiWidget::Date,
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
        let widget = field_type.default_widget();
        Self {
            name: name.clone(),
            field_type,
            label,
            required: false,
            unique: false,
            nullable: true,
            indexed: false,
            searchable: false,
            default: None,
            validation: ValidationRules::default(),
            relation: None,
            ui: UiFieldMeta {
                label: humanize(&name),
                description: None,
                widget,
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

    pub fn many_to_one(name: impl Into<String>, target: impl Into<String>) -> Self {
        let target = target.into();
        Self::new(name, FieldType::Relation)
            .relation(RelationDef {
                target_entity: target,
                kind: RelationKind::ManyToOne,
                inverse_field: None,
            })
            .indexed()
    }

    pub fn one_to_many(
        name: impl Into<String>,
        target: impl Into<String>,
        inverse_field: impl Into<String>,
    ) -> Self {
        let target = target.into();
        let mut field = Self::new(name, FieldType::Relation).relation(RelationDef {
            target_entity: target,
            kind: RelationKind::OneToMany,
            inverse_field: Some(inverse_field.into()),
        });
        field.ui.form = false;
        field.ui.list = false;
        field
    }

    pub fn many_to_many(name: impl Into<String>, target: impl Into<String>) -> Self {
        let mut field = Self::new(name, FieldType::Relation).relation(RelationDef {
            target_entity: target.into(),
            kind: RelationKind::ManyToMany,
            inverse_field: None,
        });
        field.ui.form = true;
        field.ui.list = false;
        field.ui.widget = UiWidget::Relation;
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
        self
    }

    pub fn max(mut self, n: f64) -> Self {
        self.validation.max = Some(n);
        self
    }

    pub fn email(mut self) -> Self {
        self.validation.email = true;
        self.ui.widget = UiWidget::Email;
        self
    }

    pub fn regex(mut self, pattern: impl Into<String>) -> Self {
        self.validation.regex = Some(pattern.into());
        self
    }

    pub fn relation(mut self, relation: RelationDef) -> Self {
        self.relation = Some(relation);
        self.field_type = FieldType::Relation;
        self
    }

    pub fn list(mut self, show: bool) -> Self {
        self.ui.list = show;
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

    pub fn section(mut self, name: impl Into<String>) -> Self {
        self.ui.section = Some(name.into());
        self
    }

    pub fn width(mut self, width: impl Into<String>) -> Self {
        self.ui.width = Some(width.into());
        self
    }

    pub fn hidden(mut self) -> Self {
        self.ui.hidden = true;
        self.ui.form = false;
        self.ui.list = false;
        self
    }

    pub fn system(mut self) -> Self {
        self.system = true;
        self.ui.readonly = true;
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

    pub fn type_error(&self, value: &Value) -> Option<FieldError> {
        if value.is_null() {
            return None;
        }
        let ok = match &self.field_type {
            FieldType::String
            | FieldType::Text
            | FieldType::Enum { .. }
            | FieldType::Date
            | FieldType::DateTime => value.is_string(),
            FieldType::Integer => value.is_i64() || value.is_u64(),
            FieldType::Decimal => value.is_number() || value.is_string(),
            FieldType::Boolean => value.is_boolean(),
            FieldType::Uuid | FieldType::Relation => value.is_string(),
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

    #[test]
    fn field_builder_and_serde() {
        let field = FieldDef::string("email")
            .required()
            .unique()
            .email()
            .searchable();
        assert!(field.required);
        assert!(field.validation.email);
        let json = serde_json::to_value(&field).unwrap();
        let back: FieldDef = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "email");
    }
}
