use qefro_core::{PageActionRef, PageDef, PageSection, PageTab};

pub fn restaurant_operations() -> PageDef {
    PageDef::new("restaurant-operations", "Restaurant Operations")
        .module("restaurant")
        .template("operations_dashboard")
        .layout("grid")
        .description("Floor workspace composed from existing Order, Reservation, DiningTable, and dashboard widgets.")
        .filter_fields(&["status"])
        .section(
            PageSection::widget_from("Today's Sales", "restaurant-ops", "Today's sales")
                .size("md"),
        )
        .section(PageSection::widget_from("Active Orders", "restaurant-ops", "Orders").size("md"))
        .section(
            PageSection::entity_view("Kitchen", "Order", "kanban")
                .query("status=Preparing")
                .size("xl"),
        )
        .section(PageSection::entity_view("Reservations", "Reservation", "list").size("md"))
        .section(PageSection::entity_view("Tables", "DiningTable", "list").size("md"))
        .section(
            PageSection::widget_from("Recent Activity", "restaurant-ops", "Recent order events")
                .size("xl"),
        )
        .action(PageActionRef::new("Order", "create").label("New Order"))
        .action(PageActionRef::new("Order", "export").label("Export"))
        .action(PageActionRef::new("Order", "refresh").label("Refresh"))
}

pub fn customer_workspace() -> PageDef {
    PageDef::new("customer-workspace", "Customer Workspace")
        .module("restaurant")
        .template("customer_workspace")
        .layout("split")
        .description("Customer master-detail using existing relations.")
        .context("Customer", "id")
        .tab(PageTab::new("overview", "Overview"))
        .tab(PageTab::new("orders", "Orders"))
        .tab(PageTab::new("activity", "Activity"))
        .section(
            PageSection::entity_view("Customers", "Customer", "list")
                .pane("master")
                .tab("overview"),
        )
        .section(
            PageSection::related("Orders", "Customer", "orders")
                .pane("detail")
                .tab("orders"),
        )
        .section(
            PageSection::activity("Activity", "Customer")
                .pane("detail")
                .tab("activity"),
        )
        .action(PageActionRef::new("Customer", "create").label("New Customer"))
}
