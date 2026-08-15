mod activity;
mod lead;
mod opportunity;

use qefro_api::{InstalledApp, NoopOperationHandler, OperationDef};

pub fn register(app: InstalledApp) -> InstalledApp {
    app.operation(
        OperationDef::new("qualify", "Lead")
            .label("Qualify")
            .description("Mark a contacted lead as qualified")
            .permission("lead.qualify")
            .roles(&["Manager"])
            .transition("qualify")
            .event("lead.qualified"),
        NoopOperationHandler,
    )
    .operation(
        OperationDef::new("convert", "Lead")
            .label("Convert")
            .description("Convert a contacted lead into a CRM customer")
            .permission("lead.convert")
            .roles(&["Manager"])
            .event("lead.converted")
            .transition("qualify"),
        lead::ConvertLead,
    )
    .operation(
        OperationDef::new("win", "Opportunity")
            .label("Win")
            .permission("opportunity.win")
            .roles(&["Manager"])
            .transition("win")
            .event("opportunity.won"),
        NoopOperationHandler,
    )
    .operation(
        OperationDef::new("lose", "Opportunity")
            .label("Lose")
            .permission("opportunity.lose")
            .roles(&["Manager", "Staff"])
            .style("danger")
            .confirm()
            .event("opportunity.lost"),
        opportunity::LoseOpportunity,
    )
    .operation(
        OperationDef::new("complete", "Activity")
            .label("Complete")
            .permission("activity.complete")
            .roles(&["Manager", "Staff"])
            .event("activity.completed"),
        activity::CompleteActivity,
    )
}
