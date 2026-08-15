# Studio workflows

Studio edits `WorkflowDef` and publishes an overlay onto the existing `WorkflowRegistry`. Transition execution stays in the workflow engine and `EntityService`. Studio does not reimplement `apply`.

## Editing

Authorized users (`studio.manage_workflows`) can add states, rename labels, add/remove transitions, and set allowed roles.

Before publish the server checks:

- initial state exists
- transitions reference defined states
- no duplicate `(from, name)` pairs
- every state is reachable from the initial state
- unknown role names are warnings

Unreachable states are rejected. Adding `Approved` without a transition into it will not publish.

## UI buttons

Workflow transitions already appear on the generic detail page from `_workflow.transitions`. After publish, a new transition (for example `approve` for Manager) shows up as a button without a custom React page. Custom `OperationHandler` implementations remain Rust and are labeled source-managed in the operations inspector.
