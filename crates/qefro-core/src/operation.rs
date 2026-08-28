use crate::ident::snake_case;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Metadata for a business operation. Handlers live in `qefro-db`; this type
/// is safe for HTTP, CLI, UI, and agent tool schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDef {
    pub name: String,
    pub entity: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// Documented permission key, e.g. `reservation.confirm`.
    #[serde(default)]
    pub permission: String,
    /// If non-empty, the caller must have one of these roles (Admin always allowed).
    #[serde(default)]
    pub roles: Vec<String>,
    /// Named workflow transition applied after the handler (and validated before).
    #[serde(default)]
    pub workflow_transition: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "default_true")]
    pub audit: bool,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub job: Option<String>,
    /// `action` or a CRUD verb (`create`, `get`, `find`, `update`, `delete`).
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub tool_name: String,
    /// When true, a Worker principal may execute this operation. Default false.
    #[serde(default)]
    pub worker_safe: bool,
}

fn default_style() -> String {
    "primary".into()
}
fn default_true() -> bool {
    true
}
fn default_kind() -> String {
    "action".into()
}

impl OperationDef {
    pub fn new(name: impl Into<String>, entity: impl Into<String>) -> Self {
        let name = name.into();
        let entity = entity.into();
        let permission = format!("{}.{}", snake_case(&entity), snake_case(&name));
        let tool_name = format!("{}_{}", snake_case(&name), snake_case(&entity));
        Self {
            label: humanize(&name),
            description: String::new(),
            permission,
            roles: Vec::new(),
            workflow_transition: None,
            input_schema: json!({ "type": "object", "properties": {} }),
            requires_confirmation: false,
            confirmation_message: None,
            icon: None,
            style: "primary".into(),
            audit: true,
            event: None,
            job: None,
            kind: "action".into(),
            tool_name,
            worker_safe: false,
            name,
            entity,
        }
    }

    pub fn crud(kind: &str, entity: &str) -> Self {
        let mut op = Self::new(kind, entity);
        op.kind = kind.to_string();
        op.tool_name = match kind {
            "find" => format!(
                "find_{}",
                crate::ident::to_plural_slug(entity).replace('-', "_")
            ),
            _ => format!("{}_{}", kind, snake_case(entity)),
        };
        op.permission = format!("{}.{}", snake_case(entity), kind);
        op.label = humanize(kind);
        op
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = text.into();
        self
    }

    pub fn permission(mut self, perm: impl Into<String>) -> Self {
        self.permission = perm.into();
        self
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn transition(mut self, name: impl Into<String>) -> Self {
        self.workflow_transition = Some(name.into());
        self
    }

    pub fn event(mut self, name: impl Into<String>) -> Self {
        self.event = Some(name.into());
        self
    }

    pub fn job(mut self, name: impl Into<String>) -> Self {
        self.job = Some(name.into());
        self
    }

    pub fn style(mut self, style: impl Into<String>) -> Self {
        self.style = style.into();
        self
    }

    pub fn confirm(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    pub fn confirmation_message(mut self, message: impl Into<String>) -> Self {
        self.requires_confirmation = true;
        self.confirmation_message = Some(message.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn tool(mut self, name: impl Into<String>) -> Self {
        self.tool_name = name.into();
        self
    }

    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn worker_safe(mut self) -> Self {
        self.worker_safe = true;
        self
    }

    pub fn role_allowed(&self, ctx: &crate::OpContext) -> bool {
        if ctx.is_admin() {
            return true;
        }
        if self.roles.is_empty() {
            return true;
        }
        self.roles.iter().any(|r| ctx.has_role(r))
    }

    pub fn to_client_json(&self) -> Value {
        json!({
            "name": self.name,
            "label": self.label,
            "entity": self.entity,
            "description": self.description,
            "permission": self.permission,
            "requires_confirmation": self.requires_confirmation,
            "confirmation_message": self.confirmation_message,
            "icon": self.icon,
            "style": self.style,
            "kind": self.kind,
            "tool_name": self.tool_name,
            "input_schema": self.input_schema,
            "workflow_transition": self.workflow_transition,
            "event": self.event,
        })
    }
}

/// Developer-facing constructor: `operation("confirm", "Reservation").label("Confirm")`.
pub fn operation(name: impl Into<String>, entity: impl Into<String>) -> OperationDef {
    OperationDef::new(name, entity)
}

fn humanize(name: &str) -> String {
    snake_case(name)
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_permission_and_tool() {
        let op = OperationDef::new("confirm", "Reservation")
            .label("Confirm")
            .transition("confirm")
            .event("reservation.confirmed");
        assert_eq!(op.permission, "reservation.confirm");
        assert_eq!(op.tool_name, "confirm_reservation");
        assert_eq!(op.event.as_deref(), Some("reservation.confirmed"));
    }
}
