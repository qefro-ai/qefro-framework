use crate::document::{ChildOf, DocumentConfig, NamingConfig, PrintFormat};
use crate::error::{QefroError, QefroResult};
use crate::field::{ChildTableDef, FieldDef, FieldType, RelationKind};
use crate::ident::to_plural_slug;
use crate::platform::{EntityActionDef, LinkDef, PublicFormDef};
use crate::scheduling::SchedulingConfig;
use crate::ui::{UiEntityMeta, UiFieldView, UI_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Opt-in record lifecycle besides workflow states and soft delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RecordLifecycle {
    /// Maintain `archived_at` and expose archive/restore bulk actions.
    #[serde(default)]
    pub archive: bool,
}

/// Declarative row visibility. Admins bypass. Not arbitrary SQL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowPolicy {
    AssignedTo,
    CreatedBy,
    /// Visible when the user is the assignee or the creator.
    AssignedToOrCreatedBy,
}

/// Serializable entity metadata. Applications register these at startup; YAML
/// and JSON definitions deserialize into the same type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    #[serde(default)]
    pub table: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub label_plural: String,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    #[serde(default = "default_true")]
    pub tenant_owned: bool,
    #[serde(default)]
    pub soft_delete: bool,
    #[serde(default = "default_true")]
    pub audit: bool,
    #[serde(default)]
    pub workflow: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Module / application that owns this entity.
    #[serde(default)]
    pub module: Option<String>,
    /// Field used as the human label in relation pickers. Defaults to `name`.
    #[serde(default)]
    pub display_field: String,
    /// When set, this entity is a nested child of another document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_of: Option<ChildOf>,
    /// Independent top-level document. Child entities default to false.
    #[serde(default = "default_true")]
    pub standalone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<NamingConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub print_formats: Vec<PrintFormat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_tables: Vec<ChildTableDef>,
    /// One row per tenant. Collection POST is rejected.
    #[serde(default)]
    pub singleton: bool,
    /// When true, the generic UI and attachment API are enabled for records.
    #[serde(default)]
    pub attachments: bool,
    /// Business-facing timeline. Default on so every entity can show Activity.
    #[serde(default = "default_true")]
    pub activity: bool,
    /// Comments stored as Activity records. Default on for standalone documents.
    #[serde(default = "default_true")]
    pub comments: bool,
    /// Optional archive/restore beside [`EntityDef::soft_delete`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<RecordLifecycle>,
    /// Record visibility beyond tenant isolation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_policy: Option<RowPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<EntityActionDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_form: Option<PublicFormDef>,
    /// Presentation-only. Does not affect permissions, workflow, or validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<crate::ui::EntityViews>,
    /// Server-side declarative rules. Complements per-field ValidationRules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation: Vec<crate::validation::ValidationRule>,
    /// Opt-in generic scheduling (start/end, resources, conflicts, calendar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduling: Option<SchedulingConfig>,
    /// Schema is owned elsewhere (auth `users` table). EntityService still
    /// exposes this entity; `apply_schema` does not emit DDL for it.
    #[serde(default)]
    pub skip_ddl: bool,
}

fn default_true() -> bool {
    true
}

impl EntityDef {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let table = to_plural_slug(&name).replace('-', "_");
        let slug = to_plural_slug(&name);
        let label = name.clone();
        Self {
            name: name.clone(),
            table,
            slug,
            label: label.clone(),
            label_plural: format!("{label}s"),
            fields: Vec::new(),
            tenant_owned: true,
            soft_delete: true,
            audit: true,
            workflow: None,
            icon: None,
            description: None,
            module: None,
            display_field: String::new(),
            child_of: None,
            standalone: true,
            document: None,
            naming: None,
            print_formats: Vec::new(),
            child_tables: Vec::new(),
            singleton: false,
            attachments: false,
            activity: true,
            comments: true,
            lifecycle: None,
            row_policy: None,
            actions: Vec::new(),
            links: Vec::new(),
            public_form: None,
            views: None,
            validation: Vec::new(),
            scheduling: None,
            skip_ddl: false,
        }
    }

    /// One document per tenant.
    pub fn single(name: impl Into<String>) -> Self {
        let mut def = Self::new(name);
        def.singleton = true;
        def.standalone = true;
        def
    }

    pub fn singleton(mut self) -> Self {
        self.singleton = true;
        self
    }

    pub fn attachments(mut self) -> Self {
        self.attachments = true;
        self
    }

    pub fn no_activity(mut self) -> Self {
        self.activity = false;
        self
    }

    pub fn no_comments(mut self) -> Self {
        self.comments = false;
        self
    }

    /// Optional Person / Organization identity fields (`party_type`, `person_id`, `organization_id`).
    pub fn with_party(mut self) -> Self {
        crate::identity::apply_party_fields(&mut self);
        self
    }

    /// Related Tasks panel (`entity_type` / `entity_id` on Task). Metadata only.
    pub fn with_tasks(mut self) -> Self {
        crate::task::apply_task_link(&mut self);
        self
    }

    /// Related Quotes / Sales Orders / Invoices / Payments / Returns (polymorphic customer).
    pub fn with_commerce(mut self) -> Self {
        crate::commerce::apply_commerce_links(&mut self);
        self
    }

    pub fn action(mut self, action: EntityActionDef) -> Self {
        self.actions.push(action);
        self
    }

    pub fn link(mut self, link: LinkDef) -> Self {
        self.links.push(link);
        self
    }

    pub fn public_form(mut self, form: PublicFormDef) -> Self {
        self.public_form = Some(form);
        self
    }

    pub fn table_name(mut self, table: impl Into<String>) -> Self {
        self.table = table.into();
        self
    }

    pub fn slug_name(mut self, slug: impl Into<String>) -> Self {
        self.slug = slug.into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn label_plural(mut self, label: impl Into<String>) -> Self {
        self.label_plural = label.into();
        self
    }

    pub fn field(mut self, field: FieldDef) -> Self {
        self.fields.push(field);
        self
    }

    /// Application-level custom field. Stored in the JSONB bag, not a new column.
    /// Tenant Studio fields use the same [`FieldDef`] shape and are merged at request time.
    pub fn custom_field(mut self, mut field: FieldDef) -> Self {
        field.custom = true;
        field.ui.sortable = false;
        if field.ui.section.is_none() {
            field.ui.section = Some("Custom".into());
        }
        self.fields.push(field);
        self
    }

    pub fn child_table(mut self, def: ChildTableDef) -> Self {
        if !self.fields.iter().any(|f| f.name == def.name) {
            self.fields.push(FieldDef::child_table_field(&def));
        }
        self.child_tables.push(def);
        self
    }

    pub fn child_of(mut self, parent: impl Into<String>, field: impl Into<String>) -> Self {
        self.child_of = Some(ChildOf {
            parent_entity: parent.into(),
            parent_field: field.into(),
        });
        self.standalone = false;
        self
    }

    pub fn standalone(mut self) -> Self {
        self.standalone = true;
        self
    }

    pub fn document(mut self, config: DocumentConfig) -> Self {
        self.document = Some(config);
        self
    }

    pub fn naming(mut self, config: NamingConfig) -> Self {
        self.naming = Some(config);
        self
    }

    pub fn print_format(mut self, format: PrintFormat) -> Self {
        self.print_formats.push(format);
        self
    }

    pub fn scheduling(mut self, config: SchedulingConfig) -> Self {
        self.scheduling = Some(config);
        self
    }

    pub fn soft_delete(mut self) -> Self {
        self.soft_delete = true;
        self
    }

    pub fn no_soft_delete(mut self) -> Self {
        self.soft_delete = false;
        self
    }

    pub fn archives(&self) -> bool {
        self.lifecycle.as_ref().is_some_and(|l| l.archive)
    }

    pub fn with_archive(mut self) -> Self {
        self.lifecycle = Some(RecordLifecycle { archive: true });
        self
    }

    pub fn row_policy(mut self, policy: RowPolicy) -> Self {
        self.row_policy = Some(policy);
        self
    }

    pub fn audit(mut self) -> Self {
        self.audit = true;
        self
    }

    pub fn no_audit(mut self) -> Self {
        self.audit = false;
        self
    }

    pub fn workflow(mut self, name: impl Into<String>) -> Self {
        self.workflow = Some(name.into());
        self
    }

    /// Presentation-only view metadata. Omitted entities use automatic defaults.
    pub fn views(mut self, views: crate::ui::EntityViews) -> Self {
        self.views = Some(views);
        self
    }

    pub fn validation_rule(mut self, rule: crate::validation::ValidationRule) -> Self {
        self.validation.push(rule);
        self
    }

    pub fn validation(mut self, rules: Vec<crate::validation::ValidationRule>) -> Self {
        self.validation = rules;
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn display_field(mut self, name: impl Into<String>) -> Self {
        self.display_field = name.into();
        self
    }

    pub fn no_tenant(mut self) -> Self {
        self.tenant_owned = false;
        self
    }

    /// Do not generate or migrate a table for this entity. Used for User,
    /// whose columns live in the auth schema.
    pub fn skip_ddl(mut self) -> Self {
        self.skip_ddl = true;
        self
    }

    pub fn build(mut self) -> Self {
        self.normalize();
        self
    }

    pub fn normalize(&mut self) {
        if self.table.is_empty() {
            self.table = to_plural_slug(&self.name).replace('-', "_");
        }
        if self.slug.is_empty() {
            self.slug = to_plural_slug(&self.name);
        }
        if self.label.is_empty() {
            self.label = self.name.clone();
        }
        if self.label_plural.is_empty() {
            self.label_plural = format!("{}s", self.label);
        }
        // `with_party()` calls normalize() before contact fields are added.
        // Re-pick when still empty or still the id fallback so later `.field("name")`
        // is not stuck on UUID labels.
        if self.display_field.is_empty() || self.display_field == "id" {
            self.display_field = Self::preferred_display_field(&self.fields);
        }
        if let Some(child_of) = &self.child_of {
            let parent = child_of.parent_entity.clone();
            let has_parent_fk = self.fields.iter().any(|f| {
                f.relation
                    .as_ref()
                    .map(|r| r.kind == RelationKind::ManyToOne && r.target_entity == parent)
                    .unwrap_or(false)
            });
            if !has_parent_fk {
                self.fields.insert(
                    0,
                    FieldDef::many_to_one("parent_id", parent)
                        .required()
                        .hidden(),
                );
            }
            if !self.fields.iter().any(|f| f.name == "sort_order") {
                self.fields.push(
                    FieldDef::integer("sort_order")
                        .nullable()
                        .hidden()
                        .default_value(serde_json::json!(0))
                        .label("Sort"),
                );
            }
        }
        if let Some(naming) = &self.naming {
            if !self.fields.iter().any(|f| f.name == naming.field) {
                self.fields.insert(
                    0,
                    FieldDef::string(&naming.field)
                        .unique()
                        .nullable()
                        .readonly()
                        .label("Number"),
                );
            }
        }
        for table in &self.child_tables {
            if !self.fields.iter().any(|f| f.name == table.name) {
                self.fields.push(FieldDef::child_table_field(table));
            }
        }
        for (i, field) in self.fields.iter_mut().enumerate() {
            if field.label.is_empty() {
                field.label = field.name.clone();
            }
            if field.ui.label.is_empty() {
                field.ui.label = field.label.clone();
            }
            if field.ui.order == 0 {
                field.ui.order = (i as i32) + 1;
            }
        }
    }

    pub fn system_fields(&self) -> Vec<FieldDef> {
        let mut fields = vec![FieldDef::uuid("id").required().system().label("ID")];
        if self.tenant_owned {
            fields.push(
                FieldDef::uuid("tenant_id")
                    .required()
                    .system()
                    .label("Tenant"),
            );
        }
        fields.push(
            FieldDef::datetime("created_at")
                .required()
                .system()
                .label("Created"),
        );
        fields.push(
            FieldDef::datetime("updated_at")
                .required()
                .system()
                .label("Updated"),
        );
        if self.soft_delete {
            fields.push(
                FieldDef::datetime("deleted_at")
                    .nullable()
                    .system()
                    .label("Deleted"),
            );
        }
        if self.archives() {
            fields.push(
                FieldDef::datetime("archived_at")
                    .nullable()
                    .system()
                    .label("Archived"),
            );
        }
        fields.push(
            FieldDef::uuid("created_by")
                .nullable()
                .system()
                .label("Created By"),
        );
        fields.push(
            FieldDef::uuid("updated_by")
                .nullable()
                .system()
                .label("Updated By"),
        );
        fields
    }

    pub fn stored_fields(&self) -> Vec<&FieldDef> {
        self.fields.iter().filter(|f| f.stores_column()).collect()
    }

    pub fn business_fields(&self) -> &[FieldDef] {
        &self.fields
    }

    pub fn get_field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn has_column(&self, name: &str) -> bool {
        matches!(
            name,
            "id" | "tenant_id"
                | "created_at"
                | "updated_at"
                | "deleted_at"
                | "archived_at"
                | "created_by"
                | "updated_by"
                | crate::custom::CUSTOM_BAG_COLUMN
        ) || self
            .stored_fields()
            .iter()
            .any(|f| f.name == name || f.column_name() == name)
    }

    /// Filterable through a real column or the JSONB bag (`->>` equality).
    pub fn is_filterable_field(&self, name: &str) -> bool {
        self.has_column(name)
            || self
                .get_field(name)
                .is_some_and(|f| f.custom && f.custom_status.in_effective_metadata())
    }

    /// JSONB custom fields are not sortable (no per-key btree index).
    pub fn is_sortable_field(&self, name: &str) -> bool {
        self.has_column(name) && self.get_field(name).map(|f| !f.custom).unwrap_or(true)
    }

    pub fn searchable_fields(&self) -> Vec<&FieldDef> {
        let mut fields: Vec<&FieldDef> = self
            .fields
            .iter()
            .filter(|f| f.searchable && !f.secret)
            .collect();
        fields.sort_by(|a, b| {
            b.search_weight
                .cmp(&a.search_weight)
                .then(a.name.cmp(&b.name))
        });
        fields
    }

    pub fn validate_idents(&self) -> QefroResult<()> {
        crate::ident::assert_safe_ident(&self.table)?;
        for field in self.stored_fields() {
            field.validate_name()?;
        }
        self.validate_ui_layout()?;
        self.validate_rules()?;
        for err in crate::scheduling::validate_scheduling(self, None) {
            return Err(QefroError::bad_request(err));
        }
        Ok(())
    }

    /// Unknown fields, invalid operators, type mismatches, and formula cycles.
    pub fn validate_rules(&self) -> QefroResult<()> {
        crate::formula::detect_cycles(&self.fields)?;
        for field in &self.fields {
            if field.computed {
                let Some(formula) = &field.formula else {
                    return Err(QefroError::bad_request(format!(
                        "computed field '{}.{}' is missing a formula",
                        self.name, field.name
                    )));
                };
                crate::formula::parse_formula(formula).map_err(|e| {
                    QefroError::bad_request(format!(
                        "invalid formula on '{}.{}': {e}",
                        self.name, field.name
                    ))
                })?;
            }
        }
        for (i, rule) in self.validation.iter().enumerate() {
            if let Some(name) = &rule.field {
                ensure_rule_field(self, name, i)?;
            }
            for name in &rule.require {
                ensure_rule_field(self, name, i)?;
            }
            if let Some(when) = &rule.when {
                ensure_rule_field(self, &when.field, i)?;
            }
            if let Some(compare) = &rule.compare {
                ensure_rule_field(self, &compare.field, i)?;
                for other in [
                    compare.greater_than.as_deref(),
                    compare.less_than.as_deref(),
                    compare.greater_or_equal.as_deref(),
                    compare.less_or_equal.as_deref(),
                    compare.equals.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    ensure_rule_field(self, other, i)?;
                    if let (Some(left), Some(right)) =
                        (self.get_field(&compare.field), self.get_field(other))
                    {
                        if !types_comparable(&left.field_type, &right.field_type) {
                            return Err(QefroError::bad_request(format!(
                                "validation rule {i} on '{}': cannot compare {} ({}) with {} ({})",
                                self.name,
                                left.name,
                                left.field_type.as_str(),
                                right.name,
                                right.field_type.as_str()
                            )));
                        }
                    }
                }
            }
            if let Some(op) = rule.rule.as_deref() {
                let normalized = crate::condition::normalize_op(op);
                if !matches!(
                    normalized,
                    "required"
                        | "email"
                        | "phone"
                        | "url"
                        | "regex"
                        | "min_length"
                        | "max_length"
                        | "greater_than"
                        | "less_than"
                        | "greater_or_equal"
                        | "less_or_equal"
                        | "range"
                        | "exists"
                        | "equals"
                        | "not_equals"
                        | "in"
                        | "not_in"
                        | "is_empty"
                        | "is_not_empty"
                ) {
                    return Err(QefroError::bad_request(format!(
                        "validation rule {i} on '{}' uses unknown operator '{op}'",
                        self.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Reject unknown fields, duplicates, and invalid conditions in view metadata.
    pub fn validate_ui_layout(&self) -> QefroResult<()> {
        if let Some(views) = &self.views {
            if let Some(form) = &views.form {
                validate_layout_sections(self, "form", &form.sections)?;
            }
            if let Some(detail) = &views.detail {
                validate_layout_sections(self, "detail", &detail.sections)?;
            }
            if let Some(list) = &views.list {
                for col in &list.columns {
                    ensure_known_field(self, &col.field, "list")?;
                }
            }
            if let Some(card) = &views.card {
                if let Some(title) = &card.title {
                    ensure_known_field(self, title, "card")?;
                }
                if let Some(subtitle) = &card.subtitle {
                    ensure_known_field(self, subtitle, "card")?;
                }
                if let Some(image) = &card.image {
                    ensure_known_field(self, image, "card")?;
                }
                for field in &card.fields {
                    ensure_known_field(self, field, "card")?;
                }
            }
            if let Some(kanban) = &views.kanban {
                if let Some(group) = &kanban.group_by {
                    ensure_known_field(self, group, "kanban")?;
                }
            }
        }
        for field in &self.fields {
            if let Some(when) = &field.ui.visible_when {
                ensure_condition_field(self, when, &field.name, "visible_when")?;
            }
            if let Some(when) = &field.ui.readonly_when {
                ensure_condition_field(self, when, &field.name, "readonly_when")?;
            }
            if let Some(when) = &field.required_when {
                ensure_condition_field(self, when, &field.name, "required_when")?;
            }
            if let Some(width) = &field.ui.width {
                if !matches!(width.as_str(), "full" | "half" | "third") {
                    return Err(crate::error::QefroError::bad_request(format!(
                        "field '{}.{}' has invalid width '{width}' (use full, half, or third)",
                        self.name, field.name
                    )));
                }
            }
        }
        Ok(())
    }

    fn preferred_display_field(fields: &[crate::field::FieldDef]) -> String {
        for name in ["name", "title", "code", "doc_no", "guest_name", "label"] {
            if fields.iter().any(|f| f.name == name) {
                return name.into();
            }
        }
        "id".into()
    }

    fn json_label(value: Option<&serde_json::Value>) -> Option<String> {
        let value = value?;
        if let Some(s) = value.as_str() {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        if value.is_null() || value.is_object() || value.is_array() {
            return None;
        }
        let t = value.to_string().trim_matches('"').trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }

    pub fn display_label<'a>(&self, record: &'a serde_json::Value) -> String {
        let id = Self::json_label(record.get("id")).unwrap_or_default();
        let expanded_label = |field: &str| -> Option<String> {
            record
                .get("_expanded")
                .and_then(|v| v.as_object())
                .and_then(|m| m.get(field))
                .and_then(|rel| Self::json_label(rel.get("label")))
        };
        let try_field = |field: &str| -> Option<String> {
            expanded_label(field)
                .or_else(|| Self::json_label(record.get(field)))
                .filter(|s| !s.is_empty() && *s != id)
        };
        for key in [
            self.display_field.as_str(),
            "name",
            "title",
            "code",
            "doc_no",
            "guest_name",
            "label",
        ] {
            if let Some(label) = try_field(key) {
                return label;
            }
        }
        id
    }

    pub fn to_ui_meta(&self) -> UiEntityMeta {
        let mut fields: Vec<UiFieldView> = self
            .fields
            .iter()
            .filter(|f| !f.system)
            .map(|f| {
                let mut widget_options = f.ui.widget_options.clone();
                if widget_options.entity.is_none() {
                    widget_options.entity = f.relation.as_ref().map(|r| r.target_entity.clone());
                }
                UiFieldView {
                    name: f.name.clone(),
                    field_type: f.field_type.as_str().to_string(),
                    label: f.ui.label.clone(),
                    description: f.ui.description.clone().or(f.ui.help.clone()),
                    required: f.required,
                    required_when: f.required_when.clone(),
                    list: f.ui.list,
                    list_visible: f.ui.list && !f.ui.hidden,
                    form: f.ui.form,
                    form_visible: f.ui.form && !f.ui.hidden,
                    detail: f.ui.detail,
                    detail_visible: f.ui.detail && !f.ui.hidden,
                    filter: f.ui.filter,
                    filterable: f.ui.filter,
                    searchable: f.searchable && !f.secret,
                    search_weight: f.search_weight,
                    search_exact: f.search_exact,
                    sortable: f.ui.sortable
                        || matches!(f.name.as_str(), "created_at" | "updated_at" | "name"),
                    hidden: f.ui.hidden,
                    disabled: f.ui.disabled,
                    widget: f.ui.widget.clone(),
                    widget_options,
                    placeholder: f.ui.placeholder.clone(),
                    help: f.ui.help.clone(),
                    help_text: f.ui.help.clone(),
                    section: f.ui.section.clone(),
                    tab: f.ui.tab.clone(),
                    width: f.ui.width.clone(),
                    order: f.ui.order,
                    enum_values: match &f.field_type {
                        FieldType::Enum { values } => Some(values.clone()),
                        _ => None,
                    },
                    relation: f.relation.as_ref().map(|r| r.target_entity.clone()),
                    relation_kind: f.relation.as_ref().map(|r| match r.kind {
                        crate::field::RelationKind::ManyToOne => "many_to_one".into(),
                        crate::field::RelationKind::OneToMany => "one_to_many".into(),
                        crate::field::RelationKind::ManyToMany => "many_to_many".into(),
                        crate::field::RelationKind::ChildTable => "child_table".into(),
                    }),
                    inverse_field: f.relation.as_ref().and_then(|r| r.inverse_field.clone()),
                    readonly: f.ui.readonly || f.computed,
                    visible_when: f.ui.visible_when.clone(),
                    readonly_when: f.ui.readonly_when.clone(),
                    default: f.default.clone(),
                    default_from: f.default_from.clone(),
                    computed: f.computed,
                    formula: f.formula.clone(),
                    permission_level: f.permission_level,
                    allow_on_submit: f.allow_on_submit,
                    secret: f.secret,
                    custom: f.custom,
                    custom_status: f.custom.then(|| f.custom_status.as_str().to_string()),
                    child_entity: f.relation.as_ref().and_then(|r| {
                        if f.is_child_table() {
                            Some(r.target_entity.clone())
                        } else {
                            None
                        }
                    }),
                }
            })
            .collect();
        fields.sort_by_key(|f| f.order);
        let mut tabs = Vec::new();
        let mut sections = Vec::new();
        for f in &fields {
            if let Some(tab) = &f.tab {
                if !tabs.iter().any(|t| t == tab) {
                    tabs.push(tab.clone());
                }
            }
            if let Some(section) = &f.section {
                if !sections.iter().any(|s| s == section) {
                    sections.push(section.clone());
                }
            }
        }
        UiEntityMeta {
            schema_version: UI_SCHEMA_VERSION.into(),
            entity: self.name.clone(),
            label: self.label.clone(),
            label_plural: self.label_plural.clone(),
            slug: self.slug.clone(),
            icon: self.icon.clone(),
            description: self.description.clone(),
            searchable: !self.searchable_fields().is_empty(),
            workflow: self.workflow.clone(),
            display_field: self.display_field.clone(),
            module: self.module.clone(),
            fields,
            tabs,
            sections,
            standalone: self.standalone,
            child_of: self.child_of.as_ref().map(|c| c.parent_entity.clone()),
            document: self.document.clone(),
            naming: self.naming.clone(),
            singleton: self.singleton,
            attachments: self.attachments,
            capabilities: Some(crate::ui::EntityCapabilities {
                workflow: self.workflow.is_some(),
                activity: self.activity,
                comments: self.comments,
                attachments: self.attachments,
                audit: self.audit,
                relations: self.fields.iter().any(|f| f.relation.is_some()),
                actions: !self.actions.is_empty() || self.workflow.is_some(),
                archive: self.archives(),
                assignment: self.get_field("assigned_to").is_some(),
                import: self.standalone && !self.singleton,
                export: self.standalone,
                bulk: self.standalone && !self.singleton,
                print: !self.print_formats.is_empty() || self.document.is_some(),
                communication: false,
                scheduling: self.scheduling.is_some(),
            }),
            print_formats: self
                .print_formats
                .iter()
                .map(|f| crate::ui::PrintFormatSummary {
                    name: f.name.clone(),
                    title: f.document_title(),
                    variant: f.variant.clone(),
                    version: f.version,
                })
                .collect(),
            communications: Vec::new(),
            scheduling: self.scheduling.as_ref().map(|s| s.to_summary()),
            actions: self.actions.clone(),
            links: self.links.clone(),
            public_form: self.public_form.clone(),
            views: self.views.clone(),
            permissions: None,
        }
    }

    pub fn is_child(&self) -> bool {
        self.child_of.is_some() && !self.standalone
    }

    pub fn child_table_named(&self, name: &str) -> Option<&ChildTableDef> {
        self.child_tables.iter().find(|t| t.name == name)
    }

    pub fn parent_fk(&self, parent: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| {
            f.relation
                .as_ref()
                .map(|r| r.kind == RelationKind::ManyToOne && r.target_entity == parent)
                .unwrap_or(false)
        })
    }

    pub fn from_yaml(text: &str) -> QefroResult<Self> {
        let mut def: Self = serde_yaml::from_str(text)
            .map_err(|e| QefroError::bad_request(format!("invalid entity yaml: {e}")))?;
        def.normalize();
        Ok(def)
    }

    pub fn from_json(text: &str) -> QefroResult<Self> {
        let mut def: Self = serde_json::from_str(text)
            .map_err(|e| QefroError::bad_request(format!("invalid entity json: {e}")))?;
        def.normalize();
        Ok(def)
    }

    pub fn from_file(path: &Path) -> QefroResult<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", path.display())))?;
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            Self::from_json(&text)
        } else {
            Self::from_yaml(&text)
        }
    }
}

fn ensure_known_field(entity: &EntityDef, name: &str, surface: &str) -> QefroResult<()> {
    if name.is_empty() {
        return Ok(());
    }
    if entity.get_field(name).is_some() || entity.has_column(name) {
        return Ok(());
    }
    Err(QefroError::bad_request(format!(
        "{surface} on '{}' references unknown field '{name}'",
        entity.name
    )))
}

fn ensure_condition_field(
    entity: &EntityDef,
    when: &crate::ui::UiWhen,
    field: &str,
    kind: &str,
) -> QefroResult<()> {
    if entity.get_field(&when.field).is_some() || entity.has_column(&when.field) {
        return Ok(());
    }
    Err(QefroError::bad_request(format!(
        "{kind} on '{}.{}' references unknown field '{}'",
        entity.name, field, when.field
    )))
}

fn ensure_rule_field(entity: &EntityDef, name: &str, index: usize) -> QefroResult<()> {
    if entity.get_field(name).is_some() || entity.has_column(name) {
        return Ok(());
    }
    Err(QefroError::bad_request(format!(
        "validation rule {index} on '{}' references unknown field '{name}'",
        entity.name
    )))
}

fn types_comparable(left: &FieldType, right: &FieldType) -> bool {
    if left.is_numeric() && right.is_numeric() {
        return true;
    }
    let temporal =
        |t: &FieldType| matches!(t, FieldType::Date | FieldType::DateTime | FieldType::Time);
    if temporal(left) && temporal(right) {
        return true;
    }
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn validate_layout_sections(
    entity: &EntityDef,
    surface: &str,
    sections: &[crate::ui::ViewSectionSpec],
) -> QefroResult<()> {
    let mut seen = std::collections::HashSet::new();
    for section in sections {
        if section.title.trim().is_empty() {
            return Err(QefroError::bad_request(format!(
                "{surface} layout on '{}' has a section with no title",
                entity.name
            )));
        }
        if section.columns.is_empty() && section.fields.is_empty() {
            return Err(QefroError::bad_request(format!(
                "{surface} section '{}' on '{}' has no fields",
                section.title, entity.name
            )));
        }
        if !section.columns.is_empty() && section.columns.iter().all(|c| c.fields.is_empty()) {
            return Err(QefroError::bad_request(format!(
                "{surface} section '{}' on '{}' has empty columns",
                section.title, entity.name
            )));
        }
        if let Some(when) = &section.visible_when {
            ensure_condition_field(entity, when, &section.title, &format!("{surface} section"))?;
        }
        for name in section.field_names() {
            ensure_known_field(entity, name, surface)?;
            if !seen.insert(name.to_string()) {
                return Err(QefroError::bad_request(format!(
                    "{surface} layout on '{}' lists field '{name}' more than once",
                    entity.name
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;

    #[test]
    fn yaml_file_shape() {
        let parsed = EntityDef::from_yaml(
            r#"
name: Customer
fields:
  - name: name
    type: string
    required: true
    searchable: true
    validation:
      max_length: 200
  - name: email
    type: string
    required: true
    unique: true
    validation:
      email: true
"#,
        )
        .unwrap();
        assert_eq!(parsed.name, "Customer");
        assert_eq!(parsed.fields.len(), 2);
        assert!(parsed.fields[1].validation.email);
        assert_eq!(parsed.fields[0].validation.max_length, Some(200));
    }

    #[test]
    fn yaml_roundtrip() {
        let def = EntityDef::new("Customer")
            .label("Customer")
            .label_plural("Customers")
            .field(FieldDef::string("name").required().searchable())
            .field(FieldDef::string("email").required().email().unique())
            .build();
        let yaml = serde_yaml::to_string(&def).unwrap();
        let parsed = EntityDef::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.name, "Customer");
        assert_eq!(parsed.table, "customers");
        assert_eq!(parsed.fields.len(), 2);
    }

    #[test]
    fn ui_meta_hides_system_fields() {
        let def = EntityDef::new("Lead")
            .field(FieldDef::string("title").required())
            .build();
        let ui = def.to_ui_meta();
        assert!(ui.fields.iter().all(|f| f.name != "id"));
        assert_eq!(ui.slug, "leads");
        let title = ui.fields.iter().find(|f| f.name == "title").unwrap();
        assert!(title.list_visible);
        assert!(title.form_visible);
        assert_eq!(title.widget, "text");
        assert_eq!(ui.schema_version, "1");
    }

    #[test]
    fn ui_meta_exposes_relations() {
        let def = EntityDef::new("Reservation")
            .field(FieldDef::many_to_one("customer_id", "Customer").required())
            .field(FieldDef::one_to_many("orders", "Order", "reservation_id"))
            .build();
        let ui = def.to_ui_meta();
        let customer = ui.fields.iter().find(|f| f.name == "customer_id").unwrap();
        assert_eq!(customer.relation.as_deref(), Some("Customer"));
        assert_eq!(customer.relation_kind.as_deref(), Some("many_to_one"));
        assert_eq!(customer.widget, "relation");
        let orders = ui.fields.iter().find(|f| f.name == "orders").unwrap();
        assert_eq!(orders.relation_kind.as_deref(), Some("one_to_many"));
        assert!(!orders.form_visible);
    }

    #[test]
    fn child_table_and_computed_ui() {
        use crate::field::ChildTableDef;
        let def = EntityDef::new("Order")
            .child_table(ChildTableDef::new("items", "OrderItem"))
            .field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
            .build();
        let ui = def.to_ui_meta();
        let items = ui.fields.iter().find(|f| f.name == "items").unwrap();
        assert_eq!(items.field_type, "child_table");
        assert_eq!(items.widget, "child_table");
        assert_eq!(items.relation_kind.as_deref(), Some("child_table"));
        let subtotal = ui.fields.iter().find(|f| f.name == "subtotal").unwrap();
        assert!(subtotal.computed);
        assert!(subtotal.readonly);
    }

    #[test]
    fn searchable_fields_skip_secrets_and_rank_by_weight() {
        let def = EntityDef::new("Account")
            .field(FieldDef::string("email").searchable().search_weight(2))
            .field(FieldDef::string("name").searchable().search_weight(10))
            .field(FieldDef::string("password").searchable().secret())
            .build();
        let names: Vec<_> = def
            .searchable_fields()
            .into_iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, vec!["name", "email"]);
        let ui = def.to_ui_meta();
        let password = ui.fields.iter().find(|f| f.name == "password").unwrap();
        assert!(!password.searchable);
        let name = ui.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name.search_weight, 10);
        assert_eq!(ui.schema_version, "1");
    }

    #[test]
    fn with_party_then_name_uses_name_as_display_field() {
        let def = EntityDef::new("Customer")
            .with_party()
            .field(
                crate::field::FieldDef::string("name")
                    .required()
                    .searchable(),
            )
            .build();
        assert_eq!(def.display_field, "name");
        let label = def.display_label(&serde_json::json!({
            "id": "8b3f900d-4ebc-4e46-9083-90b8ead44a83",
            "name": "Ahmed Khan"
        }));
        assert_eq!(label, "Ahmed Khan");
    }

    #[test]
    fn form_layout_rejects_unknown_and_duplicate_fields() {
        use crate::ui::{FormViewSpec, ViewSectionSpec};
        let unknown = EntityDef::new("Customer")
            .field(FieldDef::string("name"))
            .views(crate::ui::EntityViews {
                form: Some(FormViewSpec::sections(vec![ViewSectionSpec::new(
                    "Contact",
                )
                .fields(&["name", "missing"])])),
                ..Default::default()
            })
            .build();
        let err = unknown.validate_ui_layout().unwrap_err();
        assert!(err.to_string().contains("unknown field 'missing'"), "{err}");

        let dup = EntityDef::new("Customer")
            .field(FieldDef::string("name"))
            .field(FieldDef::string("email"))
            .views(crate::ui::EntityViews {
                form: Some(FormViewSpec::sections(vec![
                    ViewSectionSpec::new("A").fields(&["name"]),
                    ViewSectionSpec::new("B").fields(&["name"]),
                ])),
                ..Default::default()
            })
            .build();
        let err = dup.validate_ui_layout().unwrap_err();
        assert!(err.to_string().contains("more than once"), "{err}");
    }

    #[test]
    fn form_layout_rejects_invalid_width_and_conditions() {
        use crate::ui::{FormViewSpec, ViewSectionSpec};
        let named = FieldDef::string("name").width("wide");
        assert_eq!(named.ui.width.as_deref(), Some("wide"));
        let width = EntityDef::new("Customer").field(named).build();
        let err = width.validate_ui_layout().unwrap_err();
        assert!(err.to_string().contains("invalid width"), "{err}");

        let when = EntityDef::new("Customer")
            .field(FieldDef::string("name").visible_when("missing", serde_json::json!("x")))
            .build();
        let err = when.validate_ui_layout().unwrap_err();
        assert!(err.to_string().contains("visible_when"), "{err}");

        let section = EntityDef::new("Customer")
            .field(FieldDef::string("name"))
            .views(crate::ui::EntityViews {
                form: Some(FormViewSpec::sections(vec![ViewSectionSpec::new("Org")
                    .fields(&["name"])
                    .visible_when("party_type", serde_json::json!("Organization"))])),
                ..Default::default()
            })
            .build();
        let err = section.validate_ui_layout().unwrap_err();
        assert!(
            err.to_string().contains("unknown field 'party_type'"),
            "{err}"
        );
    }

    #[test]
    fn archive_lifecycle_adds_archived_at() {
        let def = EntityDef::new("Customer").with_archive().build();
        assert!(def.archives());
        assert!(def.system_fields().iter().any(|f| f.name == "archived_at"));
        let caps = def.to_ui_meta().capabilities.unwrap();
        assert!(caps.archive);
        assert!(caps.bulk);
        assert!(caps.export);
    }

    #[test]
    fn assignment_capability_follows_assigned_to() {
        let def = EntityDef::new("Lead")
            .field(FieldDef::assigned_to())
            .row_policy(RowPolicy::AssignedTo)
            .build();
        let caps = def.to_ui_meta().capabilities.unwrap();
        assert!(caps.assignment);
        assert_eq!(def.row_policy, Some(RowPolicy::AssignedTo));
    }

    #[test]
    fn validate_rules_rejects_unknown_field_and_bad_compare() {
        use crate::validation::ValidationRule;
        let unknown = EntityDef::new("Order")
            .field(FieldDef::integer("quantity"))
            .validation_rule(ValidationRule::compare(
                "end_date",
                "greater_than",
                "start_date",
            ))
            .build();
        let err = unknown.validate_rules().unwrap_err();
        assert!(err.to_string().contains("unknown field"), "{err}");

        let mismatch = EntityDef::new("Order")
            .field(FieldDef::integer("quantity"))
            .field(FieldDef::string("name"))
            .validation_rule(ValidationRule::compare("quantity", "greater_than", "name"))
            .build();
        let err = mismatch.validate_rules().unwrap_err();
        assert!(err.to_string().contains("cannot compare"), "{err}");

        let ok = EntityDef::new("Order")
            .field(FieldDef::date("start_date"))
            .field(FieldDef::date("end_date"))
            .validation_rule(ValidationRule::compare(
                "end_date",
                "greater_or_equal",
                "start_date",
            ))
            .build();
        ok.validate_rules().unwrap();
    }
}
