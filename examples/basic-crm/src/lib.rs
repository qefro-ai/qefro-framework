mod dashboard;
mod entities;
mod operations;
mod permissions;
mod workflows;

use qefro_api::InstalledApp;
use qefro_core::{
    AppModule, AutomationAction, AutomationDef, AutomationTrigger, Condition, NavItem, ReportDef,
};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;

pub fn module() -> AppModule {
    AppModule::new("crm")
        .version("1.0.0")
        .label("CRM")
        .description("Leads, contacts, opportunities, and activities")
        .nav(NavItem::new("Customers", "CrmCustomer"))
        .nav(NavItem::new("Opportunities", "Opportunity"))
        .nav(NavItem::new("Leads", "Lead"))
        .entity(entities::crm_customer())
        .entity(entities::lead())
        .entity(entities::contact())
        .entity(entities::opportunity())
        .entity(entities::opportunity_item())
        .entity(entities::activity())
        .dashboard(dashboard::ops())
        .report(
            ReportDef::new("pipeline-by-status", "Opportunity")
                .label("Pipeline By Status")
                .module("crm")
                .fields(&["status", "amount"])
                .group_by(&["status"])
                .sum("amount")
                .chart("bar"),
        )
        .automation(
            AutomationDef::new(
                "customer_created_activity",
                AutomationTrigger::event("entity.created"),
            )
            .description("Record activity when a CRM customer is created")
            .conditions(Condition::field_equals("entity", "CrmCustomer"))
            .action(AutomationAction::create_activity("Customer created")),
        )
        .build()
}

pub fn workflows() -> Vec<WorkflowDef> {
    vec![workflows::lead(), workflows::opportunity()]
}

pub fn permissions() -> Vec<PermissionGrant> {
    permissions::grants()
}

pub fn installed() -> InstalledApp {
    let mut app = InstalledApp::new(module());
    for wf in workflows() {
        app = app.workflow(wf);
    }
    for grant in permissions() {
        app = app.permission(grant);
    }
    operations::register(app)
}

#[cfg(test)]
mod tests {
    #[test]
    fn crm_customer_keeps_contact_columns_and_nullable_person() {
        let customer = crate::entities::crm_customer();
        assert!(customer.get_field("name").unwrap().required);
        assert!(customer.get_field("email").is_some());
        assert!(customer.get_field("phone").is_some());
        let person = customer.get_field("person_id").unwrap();
        assert!(!person.required);
        assert!(person.nullable);
        assert_eq!(person.ui.section.as_deref(), Some("Identity"));
        assert!(person
            .ui
            .help
            .as_ref()
            .is_some_and(|h| h.contains("company contact")));
        assert!(person.ui.list);
        assert!(customer.get_field("party_type").is_some());
        assert!(customer.get_field("organization_id").is_some());
        assert!(customer.get_field("tasks").is_some());
        assert!(customer.attachments);
        assert!(customer
            .links
            .iter()
            .any(|l| l.entity == "Task" && l.relation == "entity_id"));
        let identity = customer
            .fields
            .iter()
            .take(4)
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>();
        assert!(identity.contains(&"person_id"));
        let columns = &customer
            .views
            .as_ref()
            .unwrap()
            .list
            .as_ref()
            .unwrap()
            .columns;
        assert!(columns.iter().any(|c| c.field == "person_id"));
        assert!(columns.iter().any(|c| c.field == "name"));
    }
}
