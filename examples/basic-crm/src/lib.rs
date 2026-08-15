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
