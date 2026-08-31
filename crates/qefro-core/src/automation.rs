//! Declarative automation metadata. Execution lives in EntityService / JobQueue.

use crate::condition::Condition;
use crate::ident::snake_case;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationDef {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    pub trigger: AutomationTrigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<AutomationAction>,
    /// Module that owns this rule. Used for Studio grouping, not authorization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Explicit roles used as OpContext when the event has no actor (scheduled)
    /// or when the author opts in. Audited. Never implied Admin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub as_roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationTrigger {
    /// `event` or `scheduled`.
    #[serde(default = "default_event_type")]
    #[serde(rename = "type")]
    pub kind: String,
    /// Domain event name (`entity.created`, `workflow.transitioned`, …)
    /// or a specific name such as `order.ready`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Five-field cron expression when `kind` is `scheduled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
}

fn default_event_type() -> String {
    "event".into()
}

impl AutomationTrigger {
    pub fn event(name: impl Into<String>) -> Self {
        Self {
            kind: "event".into(),
            event: Some(name.into()),
            schedule: None,
        }
    }

    pub fn scheduled(cron: impl Into<String>) -> Self {
        Self {
            kind: "scheduled".into(),
            event: None,
            schedule: Some(cron.into()),
        }
    }

    pub fn is_scheduled(&self) -> bool {
        self.kind.eq_ignore_ascii_case("scheduled") || self.schedule.is_some()
    }

    pub fn event_name(&self) -> Option<&str> {
        self.event.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutomationAction {
    Named {
        #[serde(rename = "action")]
        kind: String,
        #[serde(flatten)]
        params: Value,
    },
    UpdateEntity {
        update_entity: UpdateEntityAction,
    },
    CreateEntity {
        create_entity: CreateEntityAction,
    },
    Transition {
        transition: TransitionAction,
    },
    Notify {
        notify: NotifyAction,
    },
    SendCommunication {
        send_communication: CommunicationAction,
    },
    CreateActivity {
        create_activity: ActivityAction,
    },
    CreateComment {
        create_comment: CommentAction,
    },
    Assign {
        assign: AssignAction,
    },
    SendWebhook {
        send_webhook: WebhookAction,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UpdateEntityAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(default)]
    pub fields: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CreateEntityAction {
    pub entity: String,
    #[serde(default)]
    pub fields: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TransitionAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NotifyAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipients: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CommunicationAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ActivityAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CommentAction {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssignAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default = "assigned_to_field")]
    pub field: String,
}

fn assigned_to_field() -> String {
    "assigned_to".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WebhookAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl AutomationAction {
    pub fn kind(&self) -> &str {
        match self {
            Self::UpdateEntity { .. } => "update_entity",
            Self::CreateEntity { .. } => "create_entity",
            Self::Transition { .. } => "transition",
            Self::Notify { .. } => "notify",
            Self::SendCommunication { .. } => "send_communication",
            Self::CreateActivity { .. } => "create_activity",
            Self::CreateComment { .. } => "create_comment",
            Self::Assign { .. } => "assign",
            Self::SendWebhook { .. } => "send_webhook",
            Self::Named { kind, .. } => kind.as_str(),
        }
    }

    pub fn notify(role: impl Into<String>) -> Self {
        Self::Notify {
            notify: NotifyAction {
                role: Some(role.into()),
                ..Default::default()
            },
        }
    }

    pub fn send_communication(template: impl Into<String>) -> Self {
        Self::SendCommunication {
            send_communication: CommunicationAction {
                template: Some(template.into()),
                ..Default::default()
            },
        }
    }

    pub fn create_activity(message: impl Into<String>) -> Self {
        Self::CreateActivity {
            create_activity: ActivityAction {
                message: Some(message.into()),
                ..Default::default()
            },
        }
    }
}

impl AutomationDef {
    pub fn new(name: impl Into<String>, trigger: AutomationTrigger) -> Self {
        let name = name.into();
        Self {
            name: snake_case(&name),
            enabled: true,
            description: String::new(),
            trigger,
            conditions: None,
            actions: Vec::new(),
            module: None,
            as_roles: Vec::new(),
            timezone: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = text.into();
        self
    }

    pub fn conditions(mut self, condition: Condition) -> Self {
        self.conditions = Some(condition);
        self
    }

    pub fn action(mut self, action: AutomationAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }

    pub fn as_roles(mut self, roles: &[&str]) -> Self {
        self.as_roles = roles.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.timezone = Some(tz.into());
        self
    }

    pub fn id_key(&self) -> String {
        match &self.module {
            Some(m) if !m.is_empty() => format!("{}:{}", m, self.name),
            _ => self.name.clone(),
        }
    }

    pub fn to_studio_json(&self) -> Value {
        json!({
            "name": self.name,
            "id": self.id_key(),
            "enabled": self.enabled,
            "description": self.description,
            "module": self.module,
            "trigger": self.trigger,
            "conditions": self.conditions,
            "actions": self.actions.iter().map(|a| json!({
                "kind": a.kind(),
                "action": a,
            })).collect::<Vec<_>>(),
            "as_roles": self.as_roles,
            "timezone": self.timezone,
        })
    }

    pub fn matches_event(&self, event_name: &str) -> bool {
        if !self.enabled || self.trigger.is_scheduled() {
            return false;
        }
        match self.trigger.event_name() {
            None | Some("*") => true,
            Some(name) => name == event_name || name.eq_ignore_ascii_case(event_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::condition::Condition;

    #[test]
    fn yaml_round_trip_order_ready() {
        let yaml = r#"
name: order_ready_notification
trigger:
  event: workflow.transitioned
conditions:
  all:
    - field: entity
      equals: Order
    - field: to_state
      equals: ready
actions:
  - notify:
      role: Staff
"#;
        let def: AutomationDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "order_ready_notification");
        assert!(def.matches_event("workflow.transitioned"));
        assert_eq!(def.actions[0].kind(), "notify");
        let view = serde_json::json!({
            "entity": "Order",
            "to_state": "Ready",
        });
        assert!(def.conditions.as_ref().unwrap().matches(&view));
        let _ = Condition::field_equals("entity", "Order");
    }
}
