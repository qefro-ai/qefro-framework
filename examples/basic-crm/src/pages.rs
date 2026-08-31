use qefro_core::{PageActionRef, PageDef, PageSection, PageTab};

pub fn sales_workspace() -> PageDef {
    PageDef::new("sales-workspace", "Sales Workspace")
        .module("crm")
        .template("sales_workspace")
        .layout("grid")
        .description(
            "Pipeline workspace composed from Opportunity, Task, CrmCustomer, and reports.",
        )
        .filter_fields(&["status"])
        .section(PageSection::widget_from("Revenue", "crm-ops", "Pipeline").size("md"))
        .section(PageSection::entity_view("Pipeline", "Opportunity", "kanban").size("xl"))
        .section(
            PageSection::entity_view("Open Tasks", "Task", "list")
                .query("status.neq=Completed&status.neq=Cancelled")
                .size("md"),
        )
        .section(PageSection::entity_view("Recent Customers", "CrmCustomer", "list").size("md"))
        .section(PageSection::report("Sales Report", "pipeline-by-status").size("xl"))
        .action(PageActionRef::new("Lead", "create").label("New Lead"))
        .action(PageActionRef::new("Opportunity", "create").label("New Opportunity"))
        .action(PageActionRef::new("Opportunity", "export").label("Export"))
}

pub fn customer_workspace() -> PageDef {
    PageDef::new("crm-customer-workspace", "Customer Workspace")
        .module("crm")
        .template("customer_workspace")
        .layout("split")
        .context("CrmCustomer", "id")
        .tab(PageTab::new("overview", "Overview"))
        .tab(PageTab::new("activity", "Activity"))
        .section(
            PageSection::entity_view("Customers", "CrmCustomer", "list")
                .pane("master")
                .tab("overview"),
        )
        .section(
            PageSection::activity("Activity", "CrmCustomer")
                .pane("detail")
                .tab("activity"),
        )
}
