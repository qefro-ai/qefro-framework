use qefro_core::{
    CalendarViewSpec, ChildTableDef, EntityDef, EntityViews, FieldDef, KanbanCardSpec,
    KanbanViewSpec, UiConfig,
};
use serde_json::json;

pub fn crm_customer() -> EntityDef {
    EntityDef::new("CrmCustomer")
        .label("Customer")
        .label_plural("Customers")
        .table_name("crm_customers")
        .slug_name("crm-customers")
        .icon("briefcase")
        .field(
            FieldDef::string("name")
                .required()
                .searchable()
                .max_length(200)
                .filterable(),
        )
        .field(FieldDef::string("email").nullable().email().searchable())
        .field(FieldDef::string("phone").nullable())
        .field(FieldDef::string("industry").nullable().filterable())
        .field(FieldDef::text("notes").nullable().list(false))
        .field(FieldDef::one_to_many("contacts", "Contact", "customer_id"))
        .field(FieldDef::one_to_many("opportunities", "Opportunity", "customer_id"))
        .field(FieldDef::one_to_many("activities", "Activity", "customer_id"))
        .build()
}

pub fn lead() -> EntityDef {
    EntityDef::new("Lead")
        .label("Lead")
        .label_plural("Leads")
        .table_name("leads")
        .workflow("lead")
        .field(FieldDef::string("title").required().searchable())
        .field(FieldDef::string("email").nullable().email().searchable())
        .field(FieldDef::string("phone").nullable())
        .field(FieldDef::string("company").nullable().searchable())
        .field(FieldDef::string("source").nullable().filterable())
        .field(
            FieldDef::enum_values(
                "status",
                vec!["New", "Contacted", "Qualified", "Unqualified"],
            )
            .required()
            .default_value(json!("New"))
            .filterable(),
        )
        .field(
            FieldDef::decimal("expected_value")
                .nullable()
                .min(0.0)
                .with_currency()
                .label("Expected value"),
        )
        .field(
            FieldDef::date("follow_up_date")
                .nullable()
                .ui(UiConfig::date())
                .filterable()
                .label("Follow-up date"),
        )
        .field(
            FieldDef::relation("contact_id", "Contact")
                .nullable()
                .label("Contact"),
        )
        .views(EntityViews {
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("title".into()),
                    subtitle: Some("company".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            calendar: Some(CalendarViewSpec {
                enabled: false,
                ..Default::default()
            }),
            ..Default::default()
        })
        .build()
}

pub fn contact() -> EntityDef {
    EntityDef::new("Contact")
        .label("Contact")
        .label_plural("Contacts")
        .table_name("contacts")
        .field(FieldDef::string("first_name").required().searchable())
        .field(FieldDef::string("last_name").required().searchable())
        .field(
            FieldDef::string("email")
                .nullable()
                .email()
                .searchable()
                .unique(),
        )
        .field(FieldDef::string("phone").nullable())
        .field(
            FieldDef::many_to_one("customer_id", "CrmCustomer")
                .nullable()
                .label("Customer"),
        )
        .field(FieldDef::string("title").nullable())
        .build()
}

pub fn opportunity() -> EntityDef {
    EntityDef::new("Opportunity")
        .label("Opportunity")
        .label_plural("Opportunities")
        .table_name("opportunities")
        .workflow("opportunity")
        .field(FieldDef::string("name").required().searchable())
        .field(
            FieldDef::many_to_one("customer_id", "CrmCustomer")
                .nullable()
                .label("Customer"),
        )
        .field(
            FieldDef::many_to_one("contact_id", "Contact")
                .nullable()
                .label("Contact"),
        )
        .field(
            FieldDef::decimal("probability")
                .nullable()
                .percentage()
                .label("Probability"),
        )
        .field(
            FieldDef::date("close_date")
                .nullable()
                .filterable()
                .ui(UiConfig::date())
                .label("Expected close"),
        )
        .field(
            FieldDef::enum_values("status", vec!["Open", "Qualified", "Won", "Lost"])
                .required()
                .default_value(json!("Open"))
                .filterable(),
        )
        .child_table(ChildTableDef::new("lines", "OpportunityItem").parent_field("opportunity_id"))
        .field(
            FieldDef::currency("amount")
                .computed("SUM(lines.amount)")
                .label("Value"),
        )
        .views(EntityViews {
            kanban: Some(KanbanViewSpec {
                group_by: Some("status".into()),
                card: Some(KanbanCardSpec {
                    title: Some("name".into()),
                    subtitle: Some("status".into()),
                    fields: vec!["amount".into()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            calendar: Some(CalendarViewSpec {
                enabled: false,
                ..Default::default()
            }),
            ..Default::default()
        })
        .build()
}

pub fn opportunity_item() -> EntityDef {
    EntityDef::new("OpportunityItem")
        .label("Opportunity Line")
        .label_plural("Opportunity Lines")
        .table_name("opportunity_items")
        .child_of("Opportunity", "lines")
        .field(
            FieldDef::many_to_one("opportunity_id", "Opportunity")
                .required()
                .hidden(),
        )
        .field(FieldDef::string("description").required().searchable())
        .field(
            FieldDef::integer("quantity")
                .required()
                .min(1.0)
                .default_value(json!(1)),
        )
        .field(FieldDef::currency("rate").required().min(0.0))
        .field(FieldDef::currency("amount").computed("quantity * rate"))
        .build()
}

pub fn activity() -> EntityDef {
    EntityDef::new("Activity")
        .label("Activity")
        .label_plural("Activities")
        .table_name("activities")
        .field(
            FieldDef::enum_values("kind", vec!["call", "email", "meeting", "note"])
                .required()
                .filterable(),
        )
        .field(FieldDef::string("subject").required().searchable())
        .field(FieldDef::text("body").nullable().list(false))
        .field(FieldDef::datetime("due_at").nullable())
        .field(
            FieldDef::many_to_one("customer_id", "CrmCustomer")
                .nullable()
                .label("Customer"),
        )
        .field(
            FieldDef::many_to_one("opportunity_id", "Opportunity")
                .nullable()
                .label("Opportunity"),
        )
        .field(
            FieldDef::boolean("done")
                .required()
                .default_value(json!(false))
                .filterable(),
        )
        .views(EntityViews {
            calendar: Some(CalendarViewSpec {
                start: Some("due_at".into()),
                title: Some("subject".into()),
                subtitle: Some("kind".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .build()
}
