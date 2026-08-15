use qefro_core::{DashboardCard, DashboardDef};

pub fn ops() -> DashboardDef {
    DashboardDef::new("crm-ops", "CRM operations").module("crm")
        .card(DashboardCard::count("Customers", "CrmCustomer"))
        .card(DashboardCard::count("New leads", "Lead").filter("status", "New"))
        .card(DashboardCard::count("Open opportunities", "Opportunity").filter("status", "Open"))
        .card(DashboardCard::sum("Pipeline", "Opportunity", "amount").filter("status", "Open"))
        .card(DashboardCard::status_breakdown("Leads by status", "Lead", "status"))
        .card(DashboardCard::recent("Recent activities", "Activity", 8))
}
