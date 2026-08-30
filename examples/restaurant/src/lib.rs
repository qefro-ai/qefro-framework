mod dashboard;
mod entities;
mod operations;
mod permissions;
mod workflows;

use qefro_api::InstalledApp;
use qefro_core::{
    AppModule, AutomationAction, AutomationDef, AutomationTrigger, Condition, NavItem,
    NotificationDef, ReportDef, TenantBranding, WebhookDef,
};
use qefro_permissions::PermissionGrant;
use qefro_workflow::WorkflowDef;

/// Warm hospitality palette. Applied when the tenant has not set branding yet.
const MARK: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='8' fill='%239a3412'/%3E%3Ccircle cx='16' cy='19' r='7.25' fill='none' stroke='%23f4f1ea' stroke-width='1.8'/%3E%3Cpath fill='%23f4f1ea' d='M11.2 6.4h1.9v10.2h-1.9zm7.4 0h1.45v10.2H18.6zm1.9 0H21.9v10.2h-1.4z'/%3E%3C/svg%3E";

pub fn branding() -> TenantBranding {
    TenantBranding {
        logo: Some(MARK.into()),
        favicon: Some(MARK.into()),
        primary_color: Some("#9a3412".into()),
        secondary_color: Some("#f4f1ea".into()),
        accent_color: Some("#c2410c".into()),
        company_name: None,
        app_name: Some("Qefro Kitchen".into()),
    }
}

pub fn module() -> AppModule {
    AppModule::new("restaurant")
        .version("1.0.0")
        .label("Restaurant")
        .description("Tables, reservations, menus, dine-in and takeaway orders, and payments")
        .branding(branding())
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
    fn module_contributes_hospitality_branding_defaults() {
        let branding = crate::module().branding;
        assert_eq!(branding.primary_color.as_deref(), Some("#9a3412"));
        assert_eq!(branding.accent_color.as_deref(), Some("#c2410c"));
        assert_eq!(branding.secondary_color.as_deref(), Some("#f4f1ea"));
        assert_eq!(branding.app_name.as_deref(), Some("Qefro Kitchen"));
        assert!(branding
            .logo
            .as_ref()
            .is_some_and(|s| s.starts_with("data:image/svg+xml")));
        assert_eq!(branding.logo, branding.favicon);
        assert!(branding.company_name.is_none());
    }

    #[test]
    fn ops_dashboard_is_an_operations_home() {
        let dash = crate::dashboard::ops();
        assert_eq!(dash.name, "restaurant-ops");
        assert_eq!(dash.label, "Floor operations");
        let titles: Vec<_> = dash.cards.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"Reservations today"));
        assert!(titles.contains(&"Tables free"));
        assert!(titles.contains(&"Sales today"));
        assert!(titles.contains(&"Orders by status"));
        assert!(titles.contains(&"Recent orders"));
        assert_eq!(dash.cards.iter().filter(|c| c.kind == "metric").count(), 9);
        assert!(titles.contains(&"Upcoming pickups"));
        assert!(titles.contains(&"Ready for pickup"));
    }

    #[test]
    fn order_models_dine_in_and_takeaway() {
        let order = crate::entities::order();
        let ui = order.to_ui_meta();
        assert_eq!(ui.schema_version, "1");
        let order_type = order.get_field("order_type").unwrap();
        assert_eq!(order_type.default, Some(serde_json::json!("Dine-in")));
        assert!(order_type.required);
        assert_eq!(order_type.ui.widget, "radio");
        assert!(order_type.ui.filter);
        let pickup = order.get_field("pickup_at").unwrap();
        assert!(!pickup.required);
        assert_eq!(
            pickup
                .ui
                .visible_when
                .as_ref()
                .map(|w| (w.field.as_str(), &w.equals)),
            Some(("order_type", &serde_json::json!("Takeaway")))
        );
        let table = order.get_field("table_id").unwrap();
        assert_eq!(
            table.ui.visible_when.as_ref().map(|w| w.field.as_str()),
            Some("order_type")
        );
        assert_eq!(
            table.ui.visible_when.as_ref().map(|w| &w.equals),
            Some(&serde_json::json!("Dine-in"))
        );
        let status = order.get_field("status").unwrap();
        match &status.field_type {
            qefro_core::FieldType::Enum { values } => {
                assert!(values.contains(&"Scheduled".into()));
                assert_eq!(values[0], "Draft");
                assert_eq!(values[1], "Scheduled");
            }
            other => panic!("expected enum, got {other:?}"),
        }
        let views = order.views.as_ref().unwrap();
        let columns: Vec<_> = views
            .list
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .map(|c| c.field.as_str())
            .collect();
        assert!(columns.contains(&"order_type"));
        assert!(columns.contains(&"pickup_at"));
        assert_eq!(
            views.kanban.as_ref().unwrap().group_by.as_deref(),
            Some("status")
        );
        let wf = crate::workflows::order();
        assert!(wf.states.iter().any(|s| s.name == "Scheduled"));
        assert!(wf
            .transitions
            .iter()
            .any(|t| t.name == "schedule" && t.to == "Scheduled"));
        assert!(wf
            .transitions
            .iter()
            .any(|t| t.name == "confirm" && t.from == "Scheduled" && t.to == "Confirmed"));
        assert!(wf
            .transitions
            .iter()
            .any(|t| t.name == "cancel_scheduled" && t.from == "Scheduled"));
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
