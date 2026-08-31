//! Declarative automation metadata. Execution lives in EntityService / JobQueue.

use crate::condition::Condition;
use crate::context::{is_privileged_role, ROLE_PUBLIC};
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
    /// Sequential graph. When empty, `actions` run as a linear list (compat).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<AutomationStep>,
    /// Maximum nested automation chain length. Default 8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
    /// Studio/publish version. In-flight executions snapshot the def they started with.
    #[serde(
        default = "default_version",
        skip_serializing_if = "is_default_version"
    )]
    pub version: u32,
    /// JobQueue attempts for `automation.run`. Default 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
}

fn default_true() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

fn is_default_version(v: &u32) -> bool {
    *v == 1
}

/// One node in an automation. Not a second workflow engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutomationStep {
    Wait {
        wait: WaitSpec,
    },
    Branch {
        condition: Condition,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        then: Vec<AutomationStep>,
        #[serde(
            default,
            skip_serializing_if = "Vec::is_empty",
            alias = "else",
            rename = "else"
        )]
        otherwise: Vec<AutomationStep>,
    },
    End {
        end: bool,
    },
    Action(AutomationAction),
}

/// Delay until a duration elapses or a record datetime field is reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WaitSpec {
    Duration(String),
    UntilField {
        #[serde(alias = "until")]
        until_field: String,
    },
}

impl WaitSpec {
    pub fn duration(raw: impl Into<String>) -> Self {
        Self::Duration(raw.into())
    }

    pub fn until_field(name: impl Into<String>) -> Self {
        Self::UntilField {
            until_field: name.into(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Duration(d) => format!("wait {d}"),
            Self::UntilField { until_field } => format!("wait until {until_field}"),
        }
    }
}

impl AutomationStep {
    pub fn wait(duration: impl Into<String>) -> Self {
        Self::Wait {
            wait: WaitSpec::Duration(duration.into()),
        }
    }

    pub fn wait_until(field: impl Into<String>) -> Self {
        Self::Wait {
            wait: WaitSpec::UntilField {
                until_field: field.into(),
            },
        }
    }

    pub fn branch(
        condition: Condition,
        then: Vec<AutomationStep>,
        otherwise: Vec<AutomationStep>,
    ) -> Self {
        Self::Branch {
            condition,
            then,
            otherwise,
        }
    }

    pub fn action(action: AutomationAction) -> Self {
        Self::Action(action)
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wait { .. } => "wait",
            Self::Branch { .. } => "condition",
            Self::End { .. } => "end",
            Self::Action(a) => match a.kind() {
                "update_entity" => "update_entity",
                "create_entity" => "create_entity",
                "transition" => "transition",
                "notify" => "notify",
                "send_communication" => "send_communication",
                "create_activity" => "create_activity",
                "create_comment" => "create_comment",
                "assign" => "assign",
                "send_webhook" => "send_webhook",
                "print_document" => "print_document",
                other if other == "end" => "end",
                _ => "action",
            },
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Wait { wait } => wait.label(),
            Self::Branch { condition, .. } => format!(
                "if {}",
                condition
                    .field
                    .clone()
                    .unwrap_or_else(|| "condition".into())
            ),
            Self::End { .. } => "end".into(),
            Self::Action(a) => a.kind().to_string(),
        }
    }
}

/// Parse `30m` / `1h` / `3d` / `15s`. Used by wait steps on JobQueue `run_at`.
pub fn parse_wait_duration(raw: &str) -> Result<chrono::Duration, String> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s == "0" || s == "0s" || s == "0m" {
        return Ok(chrono::Duration::zero());
    }
    let Some(idx) = s.find(|c: char| c.is_ascii_alphabetic()) else {
        return Err(format!("wait '{raw}' needs a unit (s, m, h, d)"));
    };
    let (n, unit) = s.split_at(idx);
    let amount: i64 = n
        .parse()
        .map_err(|_| format!("wait '{raw}' is not a number"))?;
    if amount < 0 {
        return Err("wait duration cannot be negative".into());
    }
    let d = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => chrono::Duration::seconds(amount),
        "m" | "min" | "mins" | "minute" | "minutes" => chrono::Duration::minutes(amount),
        "h" | "hr" | "hrs" | "hour" | "hours" => chrono::Duration::hours(amount),
        "d" | "day" | "days" => chrono::Duration::days(amount),
        other => return Err(format!("unknown wait unit '{other}'")),
    };
    Ok(d)
}

pub const DEFAULT_AUTOMATION_DEPTH: u32 = 8;

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
    PrintDocument {
        print_document: PrintDocumentAction,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PrintDocumentAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
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
            Self::PrintDocument { .. } => "print_document",
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

    pub fn create_task(title: impl Into<String>) -> Self {
        Self::CreateEntity {
            create_entity: CreateEntityAction {
                entity: "Task".into(),
                fields: json!({
                    "title": title.into(),
                    "entity_type": "{{entity}}",
                    "entity_id": "{{record_id}}",
                }),
            },
        }
    }

    pub fn print_document(format: impl Into<String>) -> Self {
        Self::PrintDocument {
            print_document: PrintDocumentAction {
                format: Some(format.into()),
                ..Default::default()
            },
        }
    }

    pub fn referenced_entity(&self) -> Option<&str> {
        match self {
            Self::CreateEntity { create_entity } => Some(create_entity.entity.as_str()),
            Self::UpdateEntity { update_entity } => update_entity.entity.as_deref(),
            Self::Transition { transition } => transition.entity.as_deref(),
            Self::Assign { assign } => assign.entity.as_deref(),
            Self::PrintDocument { print_document } => print_document.entity.as_deref(),
            _ => None,
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
            steps: Vec::new(),
            max_depth: None,
            version: 1,
            max_attempts: None,
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

    pub fn step(mut self, step: AutomationStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn max_depth(mut self, depth: u32) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn depth_limit(&self) -> u32 {
        self.max_depth.unwrap_or(DEFAULT_AUTOMATION_DEPTH)
    }

    pub fn attempt_limit(&self) -> u32 {
        self.max_attempts.unwrap_or(5).clamp(1, 20)
    }

    /// Steps to execute. `actions` is the linear shorthand when `steps` is empty.
    pub fn effective_steps(&self) -> Vec<AutomationStep> {
        if !self.steps.is_empty() {
            return self.steps.clone();
        }
        self.actions
            .iter()
            .cloned()
            .map(AutomationStep::Action)
            .collect()
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
            "steps": self.effective_steps().iter().map(step_to_studio).collect::<Vec<_>>(),
            "as_roles": self.as_roles,
            "timezone": self.timezone,
            "max_depth": self.depth_limit(),
            "max_attempts": self.attempt_limit(),
            "version": self.version,
            "status": if self.enabled { "published" } else { "disabled" },
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

fn step_to_studio(step: &AutomationStep) -> Value {
    match step {
        AutomationStep::Wait { wait } => json!({
            "kind": "wait",
            "wait": wait,
            "label": wait.label(),
        }),
        AutomationStep::Branch {
            condition,
            then,
            otherwise,
        } => json!({
            "kind": "condition",
            "condition": condition,
            "then": then.iter().map(step_to_studio).collect::<Vec<_>>(),
            "else": otherwise.iter().map(step_to_studio).collect::<Vec<_>>(),
        }),
        AutomationStep::End { .. } => json!({ "kind": "end" }),
        AutomationStep::Action(action) => json!({
            "kind": action.kind(),
            "action": action,
            "label": action.kind(),
        }),
    }
}

/// Studio / CLI validation. No arbitrary code.
pub fn validate_automation(
    def: &AutomationDef,
    registry: Option<&crate::registry::EntityRegistry>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if def.name.trim().is_empty() {
        errors.push("automation is missing a name".into());
    }
    for role in &def.as_roles {
        if is_privileged_automation_role(role) {
            errors.push(format!(
                "automation '{}': as_roles cannot include privileged role '{role}'",
                def.name
            ));
        }
    }
    if def.trigger.is_scheduled() {
        if def.trigger.schedule.as_deref().unwrap_or("").is_empty() {
            errors.push(format!(
                "automation '{}': scheduled trigger needs a cron expression",
                def.name
            ));
        } else if let Some(cron) = &def.trigger.schedule {
            if crate::schedule::parse_cron(cron).is_err() {
                errors.push(format!(
                    "automation '{}': invalid cron '{}'",
                    def.name, cron
                ));
            }
        }
    } else if def.trigger.event_name().unwrap_or("").is_empty() {
        errors.push(format!("automation '{}': missing trigger event", def.name));
    }
    let steps = def.effective_steps();
    if steps.is_empty() {
        errors.push(format!(
            "automation '{}': has no steps or actions",
            def.name
        ));
    }
    validate_steps(&def.name, &steps, registry, &mut errors, 0);
    errors
}

/// Admin / System / Public must never be used as automation execution roles.
pub fn is_privileged_automation_role(role: &str) -> bool {
    is_privileged_role(role) || role.eq_ignore_ascii_case(ROLE_PUBLIC)
}

/// Drop privileged roles. Empty result means the caller should use Worker.
pub fn sanitize_automation_roles(roles: Vec<String>) -> Vec<String> {
    roles
        .into_iter()
        .filter(|role| !is_privileged_automation_role(role))
        .collect()
}

/// Studio payloads must not carry credentials. Secrets stay in server config.
pub fn reject_unsafe_automation_payload(payload: &Value) -> crate::error::QefroResult<()> {
    fn walk(value: &Value, errors: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    let key = k.to_ascii_lowercase();
                    if key.contains("password")
                        || key.contains("api_key")
                        || key.contains("apikey")
                        || key.contains("secret")
                        || key.contains("token")
                        || key.contains("credential")
                        || key.contains("jwt")
                    {
                        errors.push(format!("automation metadata must not contain '{k}'"));
                    }
                    if key == "as_roles" {
                        let roles: Vec<&str> = match v {
                            Value::Array(items) => {
                                items.iter().filter_map(|item| item.as_str()).collect()
                            }
                            Value::String(s) => s.split(',').map(str::trim).collect(),
                            _ => Vec::new(),
                        };
                        for role in roles {
                            if is_privileged_automation_role(role) {
                                errors.push(format!(
                                    "automation as_roles cannot include privileged role '{role}'"
                                ));
                            }
                        }
                    }
                    walk(v, errors);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, errors);
                }
            }
            _ => {}
        }
    }
    let mut errors = Vec::new();
    walk(payload, &mut errors);
    if let Some(err) = errors.into_iter().next() {
        return Err(crate::error::QefroError::bad_request(err));
    }
    Ok(())
}

fn validate_steps(
    name: &str,
    steps: &[AutomationStep],
    registry: Option<&crate::registry::EntityRegistry>,
    errors: &mut Vec<String>,
    depth: usize,
) {
    if depth > 16 {
        errors.push(format!("automation '{name}': step nesting is too deep"));
        return;
    }
    let mut ended = false;
    for step in steps {
        if ended {
            errors.push(format!("automation '{name}': unreachable step after end"));
            break;
        }
        match step {
            AutomationStep::Wait { wait } => match wait {
                WaitSpec::Duration(raw) => {
                    if let Err(e) = parse_wait_duration(raw) {
                        errors.push(format!("automation '{name}': {e}"));
                    }
                }
                WaitSpec::UntilField { until_field } => {
                    if until_field.trim().is_empty() {
                        errors.push(format!("automation '{name}': wait until_field is empty"));
                    }
                }
            },
            AutomationStep::Branch {
                condition,
                then,
                otherwise,
            } => {
                if condition.field.is_none() && condition.all.is_none() && condition.any.is_none() {
                    errors.push(format!("automation '{name}': condition is empty"));
                }
                validate_steps(name, then, registry, errors, depth + 1);
                validate_steps(name, otherwise, registry, errors, depth + 1);
            }
            AutomationStep::End { .. } => {
                ended = true;
            }
            AutomationStep::Action(action) => {
                match action.kind() {
                    "notify" | "send_communication" | "send_webhook" | "create_activity"
                    | "create_comment" | "update_entity" | "create_entity" | "transition"
                    | "assign" | "print_document" => {}
                    other => errors.push(format!("automation '{name}': unknown action '{other}'")),
                }
                if let Some(entity) = action.referenced_entity() {
                    if let Some(registry) = registry {
                        if registry.try_get(entity).is_none() {
                            errors.push(format!(
                                "automation '{name}': action references unknown entity '{entity}'"
                            ));
                        }
                    }
                }
                if let AutomationAction::SendCommunication { send_communication } = action {
                    if send_communication
                        .template
                        .as_deref()
                        .unwrap_or("")
                        .is_empty()
                    {
                        errors.push(format!(
                            "automation '{name}': send_communication is missing a template"
                        ));
                    }
                }
                if let AutomationAction::Transition { transition } = action {
                    if transition.name.trim().is_empty() {
                        errors.push(format!("automation '{name}': transition is missing a name"));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::condition::Condition;
    use serde_json::json;

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

    #[test]
    fn steps_wait_and_branch_round_trip() {
        let yaml = r#"
name: order_confirmed_followup
trigger:
  event: order.confirmed
steps:
  - send_communication:
      template: order_confirmed
  - wait: 30m
  - condition:
      field: status
      equals: Preparing
    then:
      - notify:
          role: Manager
"#;
        let def: AutomationDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.effective_steps().len(), 3);
        assert_eq!(def.effective_steps()[1].kind(), "wait");
        assert_eq!(
            parse_wait_duration("30m").unwrap(),
            chrono::Duration::minutes(30)
        );
        assert!(validate_automation(&def, None).is_empty());
    }

    #[test]
    fn validate_rejects_missing_trigger_and_bad_wait() {
        let def = AutomationDef::new("broken", AutomationTrigger::event(""))
            .step(AutomationStep::wait("nope"));
        let errs = validate_automation(&def, None);
        assert!(errs.iter().any(|e| e.contains("trigger")));
        assert!(errs.iter().any(|e| e.contains("wait")));
    }

    #[test]
    fn validate_rejects_unreachable_after_end() {
        let def = AutomationDef::new("ended", AutomationTrigger::event("entity.created"))
            .step(AutomationStep::End { end: true })
            .step(AutomationStep::wait("1m"));
        let errs = validate_automation(&def, None);
        assert!(errs.iter().any(|e| e.contains("unreachable")));
    }

    #[test]
    fn reject_secrets_in_payload() {
        let err = reject_unsafe_automation_payload(&json!({"api_key": "x"}));
        assert!(err.is_err());
        assert!(reject_unsafe_automation_payload(&json!({"name": "ok"})).is_ok());
        let admin = reject_unsafe_automation_payload(&json!({"as_roles": ["Admin"]}));
        assert!(admin.is_err(), "{admin:?}");
    }

    #[test]
    fn validate_rejects_admin_as_roles() {
        let def = AutomationDef::new("priv", AutomationTrigger::event("entity.created"))
            .as_roles(&["Admin"])
            .action(AutomationAction::create_activity("x"));
        let errs = validate_automation(&def, None);
        assert!(errs.iter().any(|e| e.contains("privileged")), "{errs:?}");
        assert!(
            sanitize_automation_roles(vec!["Admin".into(), "Staff".into()])
                .iter()
                .eq(["Staff"])
        );
    }
}
