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
    AppModule::new("restaurant")
        .version("0.3.0")
        .label("Restaurant")
        .description("Tables, reservations, menus, orders, and payments")
        .entity(entities::customer())
        .entity(entities::restaurant())
        .entity(entities::branch())
        .entity(entities::table())
        .entity(entities::menu_category())
        .entity(entities::menu_item())
        .entity(entities::reservation())
        .entity(entities::order())
        .entity(entities::order_item())
        .entity(entities::payment())
        .dashboard(dashboard::ops())
        .build()
}

pub fn workflows() -> Vec<WorkflowDef> {
    vec![workflows::reservation(), workflows::order()]
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
