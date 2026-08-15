use qefro_workflow::{StateDef, TransitionDef, WorkflowDef};

pub fn lead() -> WorkflowDef {
    WorkflowDef::new("lead", "Lead", "New")
        .state(StateDef::new("Contacted"))
        .state(StateDef::new("Qualified").terminal())
        .state(StateDef::new("Unqualified").terminal())
        .transition(TransitionDef::new("contact", "New", "Contacted").roles(&["Manager", "Staff"]))
        .transition(TransitionDef::new("qualify", "Contacted", "Qualified").roles(&["Manager"]))
        .transition(
            TransitionDef::new("disqualify", "Contacted", "Unqualified")
                .roles(&["Manager", "Staff"]),
        )
}

pub fn opportunity() -> WorkflowDef {
    WorkflowDef::new("opportunity", "Opportunity", "Open")
        .state(StateDef::new("Qualified"))
        .state(StateDef::new("Won").terminal())
        .state(StateDef::new("Lost").terminal())
        .transition(TransitionDef::new("qualify", "Open", "Qualified").roles(&["Manager", "Staff"]))
        .transition(TransitionDef::new("win", "Qualified", "Won").roles(&["Manager"]))
        .transition(TransitionDef::new("lose", "Qualified", "Lost").roles(&["Manager", "Staff"]))
        .transition(TransitionDef::new("lose_open", "Open", "Lost").roles(&["Manager"]))
}
