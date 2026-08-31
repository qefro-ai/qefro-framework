use qefro_core::{Condition, OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateDef {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub terminal: bool,
}

impl StateDef {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: name.clone(),
            name,
            terminal: false,
        }
    }

    pub fn terminal(mut self) -> Self {
        self.terminal = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionDef {
    pub name: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub allowed_roles: Vec<String>,
    #[serde(default)]
    pub label: String,
    /// When true the generic UI asks before invoking the transition endpoint.
    #[serde(default)]
    pub confirmation: bool,
    #[serde(default)]
    pub confirmation_message: String,
    /// Safe metadata condition over the current record. Evaluated server-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<Condition>,
}

impl TransitionDef {
    pub fn new(name: impl Into<String>, from: impl Into<String>, to: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: name.clone(),
            name,
            from: from.into(),
            to: to.into(),
            allowed_roles: Vec::new(),
            confirmation: false,
            confirmation_message: String::new(),
            guard: None,
        }
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.allowed_roles = roles.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn confirm(mut self, message: impl Into<String>) -> Self {
        self.confirmation = true;
        self.confirmation_message = message.into();
        self
    }

    /// Require listed fields to be non-empty before the transition.
    pub fn requires(mut self, fields: &[&str]) -> Self {
        let parts: Vec<Condition> = fields
            .iter()
            .map(|name| Condition {
                field: Some((*name).to_string()),
                is_not_empty: Some(true),
                ..Default::default()
            })
            .collect();
        self.guard = Some(if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            Condition::all(parts)
        });
        self
    }

    pub fn guard(mut self, condition: Condition) -> Self {
        self.guard = Some(condition);
        self
    }

    pub fn guard_allows(&self, record: &serde_json::Value) -> QefroResult<()> {
        if let Some(guard) = &self.guard {
            if !guard.matches(record) {
                let label = if self.label.is_empty() {
                    self.name.as_str()
                } else {
                    self.label.as_str()
                };
                return Err(QefroError::workflow(format!(
                    "cannot {label}: a required field is missing"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    pub name: String,
    pub entity: String,
    #[serde(default = "status_field")]
    pub field: String,
    pub initial: String,
    pub states: Vec<StateDef>,
    pub transitions: Vec<TransitionDef>,
}

fn status_field() -> String {
    "status".into()
}

impl WorkflowDef {
    pub fn new(
        name: impl Into<String>,
        entity: impl Into<String>,
        initial: impl Into<String>,
    ) -> Self {
        let initial = initial.into();
        Self {
            name: name.into(),
            entity: entity.into(),
            field: "status".into(),
            initial: initial.clone(),
            states: vec![StateDef::new(initial)],
            transitions: Vec::new(),
        }
    }

    pub fn state(mut self, state: StateDef) -> Self {
        if !self.states.iter().any(|s| s.name == state.name) {
            self.states.push(state);
        }
        self
    }

    pub fn transition(mut self, t: TransitionDef) -> Self {
        if !self.states.iter().any(|s| s.name == t.from) {
            self.states.push(StateDef::new(&t.from));
        }
        if !self.states.iter().any(|s| s.name == t.to) {
            self.states.push(StateDef::new(&t.to));
        }
        self.transitions.push(t);
        self
    }

    pub fn find_transition(&self, from: &str, name: &str) -> Option<&TransitionDef> {
        self.transitions
            .iter()
            .find(|t| t.from == from && (t.name == name || t.to == name))
    }

    pub fn allowed_from(&self, from: &str, ctx: &OpContext) -> Vec<&TransitionDef> {
        self.transitions
            .iter()
            .filter(|t| t.from == from && self.role_allowed(t, ctx))
            .collect()
    }

    pub fn role_allowed(&self, t: &TransitionDef, ctx: &OpContext) -> bool {
        if ctx.is_admin() {
            return true;
        }
        if t.allowed_roles.is_empty() {
            return true;
        }
        t.allowed_roles.iter().any(|r| ctx.has_role(r))
    }

    /// Structural checks used by Studio before publish. Does not execute transitions.
    pub fn validate(&self) -> QefroResult<Vec<String>> {
        let mut warnings = Vec::new();
        let names: HashSet<&str> = self.states.iter().map(|s| s.name.as_str()).collect();
        if !names.contains(self.initial.as_str()) {
            return Err(QefroError::bad_request(format!(
                "initial state '{}' is not defined",
                self.initial
            )));
        }
        let mut seen = HashSet::new();
        for t in &self.transitions {
            if !names.contains(t.from.as_str()) {
                return Err(QefroError::bad_request(format!(
                    "transition '{}' starts from unknown state '{}'",
                    t.name, t.from
                )));
            }
            if !names.contains(t.to.as_str()) {
                return Err(QefroError::bad_request(format!(
                    "transition '{}' targets unknown state '{}'",
                    t.name, t.to
                )));
            }
            let key = (t.from.clone(), t.name.clone());
            if !seen.insert(key) {
                return Err(QefroError::bad_request(format!(
                    "duplicate transition '{}' from '{}'",
                    t.name, t.from
                )));
            }
            for role in &t.allowed_roles {
                if !qefro_core::studio::known_role(role) {
                    warnings.push(format!(
                        "transition '{}' references unknown role '{role}'",
                        t.name
                    ));
                }
            }
        }
        let mut reachable = HashSet::new();
        let mut stack = vec![self.initial.clone()];
        while let Some(state) = stack.pop() {
            if !reachable.insert(state.clone()) {
                continue;
            }
            for t in self.transitions.iter().filter(|t| t.from == state) {
                stack.push(t.to.clone());
            }
        }
        for state in &self.states {
            if !reachable.contains(&state.name) {
                return Err(QefroError::bad_request(format!(
                    "state '{}' is unreachable from '{}'",
                    state.name, self.initial
                )));
            }
        }
        Ok(warnings)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowRegistry {
    by_name: HashMap<String, WorkflowDef>,
    by_entity: HashMap<String, String>,
    overlay: Arc<RwLock<HashMap<String, WorkflowDef>>>,
}

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: WorkflowDef) {
        self.by_entity.insert(def.entity.clone(), def.name.clone());
        self.by_name.insert(def.name.clone(), def);
    }

    pub fn overlay_put(&self, def: WorkflowDef) {
        if let Ok(mut overlay) = self.overlay.write() {
            overlay.insert(def.entity.clone(), def);
        }
    }

    pub fn get(&self, name: &str) -> Option<WorkflowDef> {
        if let Ok(overlay) = self.overlay.read() {
            if let Some(def) = overlay.values().find(|d| d.name == name) {
                return Some(def.clone());
            }
        }
        self.by_name.get(name).cloned()
    }

    pub fn for_entity(&self, entity: &str) -> Option<WorkflowDef> {
        if let Ok(overlay) = self.overlay.read() {
            if let Some(def) = overlay.get(entity) {
                return Some(def.clone());
            }
        }
        self.by_entity
            .get(entity)
            .and_then(|n| self.by_name.get(n))
            .cloned()
    }

    pub fn list(&self) -> Vec<WorkflowDef> {
        let mut map = self.by_name.clone();
        if let Ok(overlay) = self.overlay.read() {
            for def in overlay.values() {
                if let Some(old) = self.by_entity.get(&def.entity) {
                    if old != &def.name {
                        map.remove(old);
                    }
                }
                map.insert(def.name.clone(), def.clone());
            }
        }
        let mut items: Vec<_> = map.into_values().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    }

    /// Apply a named transition. Does not persist — callers write the new
    /// status through the entity service.
    pub fn apply(
        &self,
        entity: &str,
        current: &str,
        transition: &str,
        ctx: &OpContext,
    ) -> QefroResult<String> {
        let wf = self
            .for_entity(entity)
            .ok_or_else(|| QefroError::not_found(format!("no workflow for {entity}")))?;
        let t = wf.find_transition(current, transition).ok_or_else(|| {
            QefroError::workflow(format!(
                "transition '{transition}' is not valid from '{current}'"
            ))
        })?;
        if !wf.role_allowed(t, ctx) {
            return Err(QefroError::forbidden(format!(
                "role(s) {:?} cannot apply '{transition}'",
                ctx.roles
            )));
        }
        Ok(t.to.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn reservation_wf() -> WorkflowDef {
        WorkflowDef::new("reservation", "Reservation", "Pending")
            .state(StateDef::new("Confirmed"))
            .state(StateDef::new("Seated"))
            .state(StateDef::new("Completed").terminal())
            .state(StateDef::new("Cancelled").terminal())
            .transition(
                TransitionDef::new("confirm", "Pending", "Confirmed").roles(&["Manager", "Staff"]),
            )
            .transition(
                TransitionDef::new("seat", "Confirmed", "Seated").roles(&["Staff", "Manager"]),
            )
            .transition(
                TransitionDef::new("complete", "Seated", "Completed").roles(&["Staff", "Manager"]),
            )
            .transition(TransitionDef::new("cancel", "Pending", "Cancelled"))
            .transition(
                TransitionDef::new("cancel_confirmed", "Confirmed", "Cancelled")
                    .roles(&["Manager"]),
            )
    }

    #[test]
    fn happy_path_and_forbidden() {
        let mut reg = WorkflowRegistry::new();
        reg.register(reservation_wf());
        let staff = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Staff".into()]);
        let manager = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Manager".into()]);
        let customer = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Customer".into()]);

        assert_eq!(
            reg.apply("Reservation", "Pending", "confirm", &staff)
                .unwrap(),
            "Confirmed"
        );
        assert!(reg
            .apply("Reservation", "Pending", "complete", &staff)
            .is_err());
        assert!(reg
            .apply("Reservation", "Confirmed", "cancel_confirmed", &staff)
            .is_err());
        assert_eq!(
            reg.apply("Reservation", "Confirmed", "cancel_confirmed", &manager)
                .unwrap(),
            "Cancelled"
        );
        assert!(reg
            .apply("Reservation", "Pending", "confirm", &customer)
            .is_err());
    }

    #[test]
    fn validate_rejects_unreachable_and_duplicates() {
        let ok = reservation_wf();
        assert!(ok.validate().unwrap().is_empty());
        let bad = WorkflowDef::new("order", "Order", "Draft")
            .state(StateDef::new("Approved"))
            .transition(TransitionDef::new("confirm", "Draft", "Confirmed"));
        assert!(bad.validate().is_err());
        let dup = WorkflowDef::new("order", "Order", "Draft")
            .state(StateDef::new("Confirmed"))
            .transition(TransitionDef::new("confirm", "Draft", "Confirmed"))
            .transition(TransitionDef::new("confirm", "Draft", "Confirmed"));
        assert!(dup.validate().is_err());
    }

    #[test]
    fn guard_blocks_empty_required_field() {
        let t = TransitionDef::new("confirm", "Draft", "Confirmed").requires(&["customer_id"]);
        assert!(t
            .guard_allows(&serde_json::json!({ "customer_id": "" }))
            .is_err());
        assert!(t
            .guard_allows(
                &serde_json::json!({ "customer_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" })
            )
            .is_ok());
    }

    #[test]
    fn overlay_replaces_live_workflow() {
        let mut reg = WorkflowRegistry::new();
        reg.register(reservation_wf());
        let mut next = reservation_wf();
        next = next.state(StateDef::new("Waitlisted"));
        next.transitions
            .push(TransitionDef::new("waitlist", "Pending", "Waitlisted").roles(&["Manager"]));
        next.validate().unwrap();
        reg.overlay_put(next);
        let manager = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Manager".into()]);
        assert_eq!(
            reg.apply("Reservation", "Pending", "waitlist", &manager)
                .unwrap(),
            "Waitlisted"
        );
    }
}

/// Framework Task workflow. Status is workflow-managed; clients must transition.
pub fn task_workflow() -> WorkflowDef {
    use qefro_core::{STATUS_CANCELLED, STATUS_COMPLETED, STATUS_IN_PROGRESS, STATUS_OPEN, TASK_ENTITY, TASK_WORKFLOW};

    WorkflowDef::new(TASK_WORKFLOW, TASK_ENTITY, STATUS_OPEN)
        .state(StateDef::new(STATUS_IN_PROGRESS))
        .state(StateDef::new(STATUS_COMPLETED).terminal())
        .state(StateDef::new(STATUS_CANCELLED).terminal())
        .transition(
            TransitionDef::new("start", STATUS_OPEN, STATUS_IN_PROGRESS).label("Start"),
        )
        .transition(
            TransitionDef::new("completed", STATUS_OPEN, STATUS_COMPLETED).label("Complete"),
        )
        .transition(
            TransitionDef::new("completed", STATUS_IN_PROGRESS, STATUS_COMPLETED).label("Complete"),
        )
        .transition(
            TransitionDef::new("cancelled", STATUS_OPEN, STATUS_CANCELLED)
                .label("Cancel")
                .confirm("Cancel this task?"),
        )
        .transition(
            TransitionDef::new("cancelled", STATUS_IN_PROGRESS, STATUS_CANCELLED)
                .label("Cancel")
                .confirm("Cancel this task?"),
        )
}

#[cfg(test)]
mod task_workflow_tests {
    #[test]
    fn task_workflow_is_structurally_valid() {
        crate::task_workflow().validate().unwrap();
    }
}
