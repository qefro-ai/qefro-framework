mod dashboard;
mod entities;
mod operations;
mod permissions;
mod workflows;

use qefro_api::InstalledApp;
use qefro_core::{
    AppModule, AutomationAction, AutomationDef, AutomationTrigger, Condition, NavItem,
    NotificationDef, ReportDef, WebhookDef,
};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;

pub fn module() -> AppModule {
    AppModule::new("restaurant")
        .version("1.0.0")
        .label("Restaurant")
        .description("Tables, reservations, menus, orders, and payments")
        .nav(NavItem::new("Orders", "Order"))
        .nav(NavItem::new("Reservations", "Reservation"))
        .nav(
            NavItem::new("Kitchen", "Order")
                .query("status=Preparing")
                .view("kanban"),
        )
        .nav(NavItem::new("Tables", "DiningTable"))
        .nav(NavItem::new("Menu", "MenuItem"))
        .nav(NavItem::new("Customers", "Customer"))
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
        .report(
            ReportDef::new("orders-by-status", "Order")
                .label("Orders By Status")
                .module("restaurant")
                .fields(&["status", "grand_total"])
                .group_by(&["status"])
                .sum("grand_total")
                .count("id")
                .chart("bar"),
        )
        .notification(
            NotificationDef::new("reservation_confirmed", "reservation.confirmed")
                .channels(&["in_app", "email"])
                .recipients(&["Staff", "Manager", "owner"])
                .title("Reservation confirmed")
                .module("restaurant"),
        )
        .notification(
            NotificationDef::new("order_confirmed", "order.confirmed")
                .channels(&["in_app"])
                .recipients(&["Admin", "Staff", "Manager", "owner"])
                .title("Order confirmed")
                .module("restaurant"),
        )
        .notification(
            NotificationDef::new("order_ready", "")
                .channels(&["in_app"])
                .recipients(&["Admin", "Staff", "Manager", "owner"])
                .title("Order is ready")
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
        .webhook(
            WebhookDef::new(
                "order-ready",
                "",
                std::env::var("QEFRO_RESTAURANT_ORDER_WEBHOOK_URL")
                    .unwrap_or_else(|_| "test://order-ready".into()),
            )
            .module("restaurant"),
        )
        .automation(
            AutomationDef::new(
                "order_ready_notification",
                AutomationTrigger::event("workflow.transitioned"),
            )
            .description("Notify restaurant staff when an order becomes Ready")
            .conditions(Condition::all(vec![
                Condition::field_equals("entity", "Order"),
                Condition::field_equals("to_state", "Ready"),
            ]))
            .action(AutomationAction::Notify {
                notify: qefro_core::NotifyAction {
                    notification: Some("order_ready".into()),
                    role: Some("Staff".into()),
                    ..Default::default()
                },
            })
            .action(AutomationAction::SendWebhook {
                send_webhook: qefro_core::WebhookAction {
                    webhook: Some("order-ready".into()),
                    name: None,
                },
            }),
        )
        .automation(
            AutomationDef::new(
                "order_confirmed_activity",
                AutomationTrigger::event("workflow.transitioned"),
            )
            .conditions(Condition::all(vec![
                Condition::field_equals("entity", "Order"),
                Condition::field_equals("to_state", "Confirmed"),
            ]))
            .action(AutomationAction::create_activity("Kitchen: order confirmed")),
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
        assert!(ui
            .fields
            .iter()
            .any(|f| f.name == "line_total" && f.computed));
    }

    #[test]
    fn ops_navigation_hides_setup_entities() {
        let module = crate::module();
        assert_eq!(
            module.default_nav_slugs(),
            vec![
                "orders",
                "reservations",
                "tables",
                "menu-items",
                "customers"
            ]
        );
        let hidden = module.default_hidden_slugs();
        for slug in [
            "restaurants",
            "restaurant-settings",
            "branches",
            "menu-categories",
            "payments",
            "ui-showcases",
        ] {
            assert!(
                hidden.iter().any(|s| s == slug),
                "missing {slug} in {hidden:?}"
            );
        }
        assert!(!hidden.iter().any(|s| s == "reservations"));
        assert!(!hidden.iter().any(|s| s == "order-items"));
    }

    #[test]
    fn customer_keeps_contact_columns_and_nullable_person() {
        let customer = crate::entities::customer();
        assert!(customer.get_field("name").unwrap().required);
        assert!(customer.get_field("email").unwrap().required);
        assert!(customer.get_field("phone").is_some());
        let person = customer.get_field("person_id").unwrap();
        assert!(!person.required);
        assert!(person.nullable);
        assert_eq!(person.ui.section.as_deref(), Some("Identity"));
        assert!(person
            .ui
            .help
            .as_ref()
            .is_some_and(|h| h.contains("Walk-ins")));
        assert!(person.ui.list);
        assert!(customer.get_field("party_type").is_some());
        assert!(customer.get_field("organization_id").is_some());
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

    #[test]
    fn kitchen_order_notifications_are_declared() {
        let module = crate::module();
        let names: Vec<_> = module
            .notifications
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"order_confirmed"), "{names:?}");
        assert!(names.contains(&"order_ready"), "{names:?}");
        let autos: Vec<_> = module.automations.iter().map(|a| a.name.as_str()).collect();
        assert!(autos.contains(&"order_ready_notification"), "{autos:?}");
    }
}
