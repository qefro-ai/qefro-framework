mod activity;
mod lead;
mod opportunity;

use qefro_api::{InstalledApp, NoopOperationHandler, OperationDef};
use serde_json::json;

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
            .description("Convert a contacted lead into a CRM customer and follow-up task")
            .permission("lead.convert")
            .roles(&["Manager"])
            .event("lead.converted")
            .transition("qualify")
            .input_schema(json!({
                "type": "object",
                "properties": {
                    "note": {
                        "type": "string",
                        "title": "Handoff note",
                        "description": "Optional note for the follow-up task"
                    }
                }
            })),
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
