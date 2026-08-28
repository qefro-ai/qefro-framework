mod dashboard;
mod entities;
mod operations;
mod permissions;
mod workflows;

use qefro_api::InstalledApp;
use qefro_core::AppModule;
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;

pub fn module() -> AppModule {
    AppModule::new("crm")
        .version("1.0.0")
        .label("CRM")
        .description("Leads, contacts, opportunities, and activities")
        .entity(entities::crm_customer())
        .entity(entities::lead())
        .entity(entities::contact())
        .entity(entities::opportunity())
        .entity(entities::opportunity_item())
        .entity(entities::activity())
        .dashboard(dashboard::ops())
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
        assert!(person.ui.help.as_ref().is_some_and(|h| h.contains("company contact")));
        assert!(person.ui.list);
        assert_eq!(customer.fields[0].name, "person_id");
        let columns = &customer.views.as_ref().unwrap().list.as_ref().unwrap().columns;
        assert!(columns.iter().any(|c| c.field == "person_id"));
        assert!(columns.iter().any(|c| c.field == "name"));
    }
}
