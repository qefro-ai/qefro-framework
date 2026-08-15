use qefro_core::{EntityDef, FieldDef};
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
        .field(FieldDef::decimal("amount").nullable().min(0.0))
        .field(FieldDef::date("close_date").nullable().filterable())
        .field(
            FieldDef::enum_values("status", vec!["Open", "Qualified", "Won", "Lost"])
                .required()
                .default_value(json!("Open"))
                .filterable(),
        )
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
        .build()
}
