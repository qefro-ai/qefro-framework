//! V0.9 business-platform metadata: actions, links, public forms,
//! notifications, and webhooks. Execution stays in EntityService.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Document action shown on the generic detail page. Invocation still goes
/// through `EntityService::execute`; this type is presentation + discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityActionDef {
    pub name: String,
    #[serde(default)]
    pub label: String,
    /// Operation to invoke. Defaults to `name`.
    #[serde(default)]
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<ConfirmationDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationDef {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub message: String,
}

impl EntityActionDef {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: humanize(&name),
            operation: name.clone(),
            icon: None,
            roles: Vec::new(),
            visibility: None,
            confirmation: None,
            name,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = operation.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn roles(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn confirm(mut self, message: impl Into<String>) -> Self {
        self.confirmation = Some(ConfirmationDef {
            required: true,
            message: message.into(),
        });
        self
    }
}

/// Related-records link. Prefer deriving from relations; use this when the
/// automatic inverse is not enough.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkDef {
    pub label: String,
    pub entity: String,
    /// Field on the related entity that points at this record.
    pub relation: String,
}

impl LinkDef {
    pub fn new(
        label: impl Into<String>,
        entity: impl Into<String>,
        relation: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            entity: entity.into(),
            relation: relation.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicFormDef {
    #[serde(default)]
    pub enabled: bool,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_message: Option<String>,
    /// Requests per minute per IP. None uses the platform default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<u32>,
}

impl PublicFormDef {
    pub fn new(slug: impl Into<String>) -> Self {
        Self {
            enabled: true,
            slug: slug.into(),
            title: None,
            description: None,
            fields: Vec::new(),
            success_message: None,
            rate_limit: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn fields(mut self, fields: &[&str]) -> Self {
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn success_message(mut self, message: impl Into<String>) -> Self {
        self.success_message = Some(message.into());
        self
    }

    pub fn rate_limit(mut self, n: u32) -> Self {
        self.rate_limit = Some(n);
        self
    }

    pub fn allows(&self, field: &str) -> bool {
        self.fields.iter().any(|f| f == field)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationDef {
    pub name: String,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub channels: Vec<String>,
    /// Role names, `owner`, or `creator`.
    #[serde(default)]
    pub recipients: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

impl NotificationDef {
    pub fn new(name: impl Into<String>, event: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            event: event.into(),
            channels: vec!["in_app".into()],
            recipients: vec!["Staff".into(), "Manager".into()],
            title: None,
            body: None,
            module: None,
        }
    }

    pub fn channels(mut self, channels: &[&str]) -> Self {
        self.channels = channels.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn recipients(mut self, recipients: &[&str]) -> Self {
        self.recipients = recipients.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookDef {
    pub name: String,
    pub event: String,
    pub target: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Environment variable holding the HMAC secret. Never returned to clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

fn default_true() -> bool {
    true
}

impl WebhookDef {
    pub fn new(
        name: impl Into<String>,
        event: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            event: event.into(),
            target: target.into(),
            enabled: true,
            secret_env: None,
            module: None,
        }
    }

    pub fn secret_env(mut self, name: impl Into<String>) -> Self {
        self.secret_env = Some(name.into());
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }
}

/// HMAC-SHA256 over `{timestamp}.{event_id}.{body}`.
pub fn webhook_signature(secret: &str, timestamp: i64, event_id: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .unwrap_or_else(|_| HmacSha256::new_from_slice(b"qefro").expect("hmac key"));
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(event_id.as_bytes());
    mac.update(b".");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

pub fn webhook_secret(def: &WebhookDef) -> String {
    if let Some(env) = &def.secret_env {
        if let Ok(v) = std::env::var(env) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    std::env::var("QEFRO_WEBHOOK_SECRET").unwrap_or_else(|_| "qefro-dev-webhook-secret".into())
}

fn humanize(name: &str) -> String {
    name.replace(['-', '_'], " ")
        .split_whitespace()
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
    fn signature_is_stable() {
        let a = webhook_signature("s", 1, "e", b"{}");
        let b = webhook_signature("s", 1, "e", b"{}");
        assert_eq!(a, b);
        assert!(a.starts_with("sha256="));
        assert_ne!(a, webhook_signature("other", 1, "e", b"{}"));
    }

    #[test]
    pub fn public_form_allowlist() {
        let form = PublicFormDef::new("book").fields(&["name", "phone"]);
        assert!(form.allows("name"));
        assert!(!form.allows("salary"));
    }
}
