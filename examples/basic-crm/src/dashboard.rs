use qefro_core::{DashboardCard, DashboardDef};

pub fn ops() -> DashboardDef {
    DashboardDef::new("crm-ops", "CRM operations")
        .module("crm")
        .card(DashboardCard::kpi("Customers", "CrmCustomer").size("sm"))
        .card(
            DashboardCard::count("Open tasks", "Task")
                .filter("status.neq", "Completed")
                .filter("status.neq", "Cancelled"),
        )
        .card(DashboardCard::count("New leads", "Lead").filter("status", "New"))
        .card(DashboardCard::count("Open opportunities", "Opportunity").filter("status", "Open"))
        .card(
            DashboardCard::sum("Pipeline", "Opportunity", "amount")
                .filter("status", "Open")
                .roles(&["Admin", "Manager"]),
        )
        .card(DashboardCard::workflow("Opportunities by status", "Opportunity").size("lg"))
        .card(DashboardCard::status_breakdown(
            "Leads by status",
            "Lead",
            "status",
        ))
        .card(
            DashboardCard::chart("Pipeline by status", "Opportunity", "bar", "status")
                .metric_name("sum")
                .measure_field("amount")
                .size("xl"),
        )
        .card(DashboardCard::activity(
            "Recent CRM activity",
            "CrmCustomer",
            8,
        ))
        .card(DashboardCard::recent("Recent activities", "Activity", 8))
        .card(
            DashboardCard::report_card("Pipeline report", "Opportunity", "pipeline-by-status")
                .size("lg"),
        )
}
