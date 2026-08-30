use qefro_core::{DashboardCard, DashboardDef};

pub fn ops() -> DashboardDef {
    DashboardDef::new("restaurant-ops", "Floor operations")
        .module("restaurant")
        .card(
            DashboardCard::kpi("Today's reservations", "Reservation")
                .filter("reservation_date", "today")
                .size("sm"),
        )
        .card(
            DashboardCard::kpi("Orders", "Order")
                .filter("status", "Confirmed")
                .size("sm"),
        )
        .card(DashboardCard::count("Available tables", "DiningTable").filter("status", "available"))
        .card(DashboardCard::count("Occupied tables", "DiningTable").filter("status", "occupied"))
        .card(DashboardCard::count("Draft orders", "Order").filter("status", "Draft"))
        .card(DashboardCard::count("Upcoming pickups", "Order").filter("status", "Scheduled"))
        .card(DashboardCard::count("Orders preparing", "Order").filter("status", "Preparing"))
        .card(
            DashboardCard::kpi("Ready orders", "Order")
                .filter("status", "Ready")
                .size("sm"),
        )
        .card(
            DashboardCard::count("Ready for pickup", "Order")
                .filter("status", "Ready")
                .filter("order_type", "Takeaway"),
        )
        .card(
            DashboardCard::sum("Today's sales", "Payment", "amount")
                .filter("status", "captured")
                .roles(&["Admin", "Manager"]),
        )
        .card(
            DashboardCard::chart("Sales trend", "Order", "area", "order_date")
                .metric_name("sum")
                .measure_field("grand_total")
                .size("xl"),
        )
        .card(DashboardCard::workflow("Kitchen status", "Order").size("md"))
        .card(DashboardCard::status_breakdown("Table status", "DiningTable", "status").size("md"))
        .card(DashboardCard::status_breakdown(
            "Reservations by status",
            "Reservation",
            "status",
        ))
        .card(DashboardCard::status_breakdown(
            "Orders by status",
            "Order",
            "status",
        ))
        .card(DashboardCard::activity("Recent order events", "Order", 8).size("lg"))
        .card(DashboardCard::recent(
            "Upcoming reservations",
            "Reservation",
            8,
        ))
        .card(DashboardCard::recent("Recent orders", "Order", 8))
        .card(DashboardCard::audit("Changes today").roles(&["Admin"]))
}
