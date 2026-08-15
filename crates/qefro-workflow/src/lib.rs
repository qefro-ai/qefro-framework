use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitionDef {
    pub name: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub allowed_roles: Vec<String>,
    #[serde(default)]
    pub label: String,
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
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowRegistry {
    by_name: HashMap<String, WorkflowDef>,
    by_entity: HashMap<String, String>,
}

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: WorkflowDef) {
        self.by_entity.insert(def.entity.clone(), def.name.clone());
        self.by_name.insert(def.name.clone(), def);
    }

    pub fn get(&self, name: &str) -> Option<&WorkflowDef> {
        self.by_name.get(name)
    }

    pub fn for_entity(&self, entity: &str) -> Option<&WorkflowDef> {
        self.by_entity.get(entity).and_then(|n| self.by_name.get(n))
    }

    pub fn list(&self) -> Vec<&WorkflowDef> {
        self.by_name.values().collect()
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
}
