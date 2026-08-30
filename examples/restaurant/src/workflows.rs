use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};

pub fn reservation() -> WorkflowDef {
    WorkflowDef::new("reservation", "Reservation", "Pending")
        .state(StateDef::new("Confirmed"))
        .state(StateDef::new("Seated"))
        .state(StateDef::new("Completed").terminal())
        .state(StateDef::new("Cancelled").terminal())
        .transition(
            TransitionDef::new("confirm", "Pending", "Confirmed")
                .roles(&["Manager", "Staff"])
                .label("Confirm"),
        )
        .transition(
            TransitionDef::new("seat", "Confirmed", "Seated")
                .roles(&["Manager", "Staff"])
                .label("Seat Customer"),
        )
        .transition(
            TransitionDef::new("complete", "Seated", "Completed")
                .roles(&["Manager", "Staff"])
                .label("Complete"),
        )
        .transition(
            TransitionDef::new("cancel", "Pending", "Cancelled")
                .label("Cancel")
                .confirm("Cancel this reservation?"),
        )
        .transition(
            TransitionDef::new("cancel_confirmed", "Confirmed", "Cancelled")
                .roles(&["Manager"])
                .label("Cancel"),
        )
}

pub fn order() -> WorkflowDef {
    WorkflowDef::new("order", "Order", "Draft")
        .state(StateDef::new("Scheduled"))
        .state(StateDef::new("Confirmed"))
        .state(StateDef::new("Preparing"))
        .state(StateDef::new("Ready"))
        .state(StateDef::new("Completed").terminal())
        .state(StateDef::new("Cancelled").terminal())
        .transition(
            TransitionDef::new("confirm", "Draft", "Confirmed")
                .roles(&["Manager", "Staff"])
                .label("Confirm"),
        )
        .transition(
            TransitionDef::new("schedule", "Draft", "Scheduled")
                .roles(&["Manager", "Staff"])
                .label("Schedule Pickup"),
        )
        .transition(
            TransitionDef::new("confirm", "Scheduled", "Confirmed")
                .roles(&["Manager", "Staff"])
                .label("Confirm"),
        )
        .transition(
            TransitionDef::new("prepare", "Confirmed", "Preparing")
                .roles(&["Manager", "Staff"])
                .label("Start Preparing"),
        )
        .transition(
            TransitionDef::new("ready", "Preparing", "Ready")
                .roles(&["Manager", "Staff"])
                .label("Mark Ready"),
        )
        .transition(
            TransitionDef::new("complete", "Ready", "Completed")
                .roles(&["Manager", "Staff"])
                .label("Complete"),
        )
        .transition(
            TransitionDef::new("cancel", "Draft", "Cancelled")
                .label("Cancel")
                .confirm("Cancel this order?"),
        )
        .transition(
            TransitionDef::new("cancel_scheduled", "Scheduled", "Cancelled").label("Cancel"),
        )
        .transition(
            TransitionDef::new("cancel_confirmed", "Confirmed", "Cancelled")
                .roles(&["Manager"])
                .label("Cancel"),
        )
        .transition(
            TransitionDef::new("cancel_preparing", "Preparing", "Cancelled")
                .roles(&["Manager"])
                .label("Cancel"),
        )
}
