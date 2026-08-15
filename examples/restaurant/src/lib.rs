mod dashboard;
mod entities;
mod operations;
mod permissions;
mod workflows;

use qefro_api::InstalledApp;
use qefro_core::{AppModule, NotificationDef, ReportDef, WebhookDef};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;

pub fn module() -> AppModule {
    AppModule::new("restaurant")
        .version("1.0.0")
        .label("Restaurant")
        .description("Tables, reservations, menus, orders, and payments")
        .entity(entities::customer())
        .entity(entities::restaurant())
        .entity(entities::restaurant_settings())
        .entity(entities::branch())
        .entity(entities::table())
        .entity(entities::menu_category())
        .entity(entities::menu_item())
        .entity(entities::reservation())
        .entity(entities::order())
        .entity(entities::order_item())
        .entity(entities::payment())
        .entity(entities::ui_showcase())
        .entity(entities::showcase_line())
        .dashboard(dashboard::ops())
        .report(
            ReportDef::new("sales-by-day", "Order")
                .label("Sales By Day")
                .module("restaurant")
                .fields(&["order_date", "grand_total"])
                .group_by(&["order_date"])
                .sum("grand_total")
                .chart("bar"),
        )
        .notification(
            NotificationDef::new("reservation_confirmed", "reservation.confirmed")
                .channels(&["in_app", "email"])
                .recipients(&["Staff", "Manager", "owner"])
                .title("Reservation confirmed")
                .module("restaurant"),
        )
        .webhook(
            WebhookDef::new(
                "reservation-created",
                "reservation.created",
                std::env::var("QEFRO_RESTAURANT_WEBHOOK_URL")
                    .unwrap_or_else(|_| "test://reservation".into()),
            )
            .module("restaurant"),
        )
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
    for grant in permissions::field_levels() {
        app = app.field_level(grant);
    }
    operations::register(app)
}

#[cfg(test)]
mod tests {
    #[test]
    fn showcase_metadata_covers_the_widget_set() {
        let entity = crate::entities::ui_showcase();
        let ui = entity.to_ui_meta();
        assert_eq!(ui.schema_version, "1");
        let widget = |name: &str| {
            ui.fields
                .iter()
                .find(|f| f.name == name)
                .unwrap()
                .widget
                .as_str()
        };
        assert_eq!(widget("price"), "currency");
        assert_eq!(widget("discount"), "percentage");
        assert_eq!(widget("birth_date"), "date");
        assert_eq!(widget("appointment_time"), "time");
        assert_eq!(widget("appointment_at"), "datetime");
        assert_eq!(widget("brand_color"), "color");
        assert_eq!(widget("customer_id"), "relation");
        assert_eq!(widget("rich_description"), "rich_text");
        assert_eq!(widget("image"), "image");
        assert_eq!(widget("attachment"), "file");
        assert_eq!(widget("lines"), "child_table");
        assert!(ui.tabs.contains(&"Details".into()));
        assert!(ui.fields.iter().any(|f| f.name == "line_total" && f.computed));
    }
}
