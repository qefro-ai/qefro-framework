use qefro_core::{DashboardCard, DashboardDef};

pub fn ops() -> DashboardDef {
    DashboardDef::new("restaurant-ops", "Restaurant operations")
        .module("restaurant")
        .card(
            DashboardCard::count("Today's reservations", "Reservation")
                .filter("reservation_date", "today"),
        )
        .card(DashboardCard::count("Available tables", "DiningTable").filter("status", "available"))
        .card(DashboardCard::count("Occupied tables", "DiningTable").filter("status", "occupied"))
        .card(DashboardCard::count("Draft orders", "Order").filter("status", "Draft"))
        .card(DashboardCard::count("Orders preparing", "Order").filter("status", "Preparing"))
        .card(DashboardCard::count("Orders ready", "Order").filter("status", "Ready"))
        .card(DashboardCard::sum("Today's sales", "Payment", "amount").filter("status", "captured"))
        .card(DashboardCard::status_breakdown(
            "Reservations by status",
            "Reservation",
            "status",
        ))
        .card(DashboardCard::recent(
            "Recent reservations",
            "Reservation",
            8,
        ))
}
