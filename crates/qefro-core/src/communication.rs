//! Metadata-driven communication: templates, channels, and recipient rules.
//!
//! Delivery lives in EntityService / JobQueue. This module is presentation of
//! the existing business model — not a second event or automation engine.

use crate::error::{QefroError, QefroResult};
use crate::registry::EntityRegistry;
use crate::template::{reject_unsafe_template, template_paths, validate_template_paths};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CHANNEL_IN_APP: &str = "in_app";
pub const CHANNEL_EMAIL: &str = "email";
pub const CHANNEL_SMS: &str = "sms";
pub const CHANNEL_WHATSAPP: &str = "whatsapp";

pub const PURPOSE_TRANSACTIONAL: &str = "transactional";
pub const PURPOSE_MARKETING: &str = "marketing";

pub const COMM_PENDING: &str = "pending";
pub const COMM_QUEUED: &str = "queued";
pub const COMM_SENDING: &str = "sending";
pub const COMM_SENT: &str = "sent";
pub const COMM_DELIVERED: &str = "delivered";
pub const COMM_FAILED: &str = "failed";
pub const COMM_DEAD_LETTER: &str = "dead_letter";
pub const COMM_SKIPPED: &str = "skipped";

pub const CHANNELS: &[&str] = &[CHANNEL_IN_APP, CHANNEL_EMAIL, CHANNEL_SMS, CHANNEL_WHATSAPP];

pub const PREFERENCE_NONE: &str = "none";

/// Declarative message template bound to an existing entity and domain event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunicationDef {
    pub name: String,
    #[serde(default)]
    pub event: String,
    pub entity: String,
    #[serde(default = "default_channels")]
    pub channels: Vec<String>,
    #[serde(default = "default_purpose")]
    pub purpose: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: String,
    /// Relation or field path to the recipient record, e.g. `customer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient_path: Option<String>,
    /// Optional field on the recipient (`communication_channel`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_channel_field: Option<String>,
    /// Optional boolean on the recipient. Honored for marketing only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opt_out_field: Option<String>,
    /// Attach the entity's generated PDF when one exists. Does not generate a document here.
    #[serde(default)]
    pub attach_document: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

fn default_channels() -> Vec<String> {
    vec![CHANNEL_IN_APP.into()]
}

fn default_purpose() -> String {
    PURPOSE_TRANSACTIONAL.into()
}

impl CommunicationDef {
    pub fn new(
        name: impl Into<String>,
        event: impl Into<String>,
        entity: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            event: event.into(),
            entity: entity.into(),
            channels: default_channels(),
            purpose: default_purpose(),
            subject: None,
            body: String::new(),
            recipient_path: None,
            preferred_channel_field: None,
            opt_out_field: None,
            attach_document: false,
            module: None,
        }
    }

    pub fn channels(mut self, channels: &[&str]) -> Self {
        self.channels = channels.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = purpose.into();
        self
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn recipient_path(mut self, path: impl Into<String>) -> Self {
        self.recipient_path = Some(path.into());
        self
    }

    pub fn preferred_channel_field(mut self, field: impl Into<String>) -> Self {
        self.preferred_channel_field = Some(field.into());
        self
    }

    pub fn opt_out_field(mut self, field: impl Into<String>) -> Self {
        self.opt_out_field = Some(field.into());
        self
    }

    pub fn attach_document(mut self) -> Self {
        self.attach_document = true;
        self
    }

    pub fn module(mut self, name: impl Into<String>) -> Self {
        self.module = Some(name.into());
        self
    }

    pub fn is_marketing(&self) -> bool {
        self.purpose.eq_ignore_ascii_case(PURPOSE_MARKETING)
    }

    pub fn matches_event(&self, event_name: &str) -> bool {
        !self.event.is_empty() && (self.event == event_name || self.event == "*")
    }
}

/// Ordered channel list after preference and opt-out. Empty means skip send.
pub fn select_channels(def: &CommunicationDef, recipient: &Value) -> Vec<String> {
    if def.is_marketing() {
        if let Some(field) = &def.opt_out_field {
            if truthy(recipient.get(field)) {
                return Vec::new();
            }
        }
    }
    let allowed: Vec<String> = def
        .channels
        .iter()
        .map(|c| c.trim().to_ascii_lowercase())
        .filter(|c| CHANNELS.contains(&c.as_str()))
        .collect();
    if allowed.is_empty() {
        return Vec::new();
    }
    let preferred = def
        .preferred_channel_field
        .as_ref()
        .and_then(|f| recipient.get(f))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    match preferred.as_deref() {
        Some(PREFERENCE_NONE) => Vec::new(),
        Some(pref) if allowed.iter().any(|c| c == pref) => {
            let mut out = vec![pref.to_string()];
            for ch in &allowed {
                if ch != pref {
                    out.push(ch.clone());
                }
            }
            out
        }
        _ => allowed,
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(true)) => true,
        Some(Value::String(s)) => matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    }
}

/// Address used by a channel. Never includes secrets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecipientAddress {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_id: Option<String>,
    pub name: Option<String>,
}

impl RecipientAddress {
    pub fn from_record(record: &Value) -> Self {
        let email = first_string(record, &["email", "person.email", "user.email"]);
        let phone = first_string(record, &["phone", "person.phone", "mobile"]);
        let user_id = first_string(record, &["user_id", "person.user_id"]);
        let name = first_string(record, &["name", "person.name", "label"]);
        Self {
            email,
            phone,
            user_id,
            name,
        }
    }

    pub fn address_for(&self, channel: &str) -> Option<String> {
        match channel {
            CHANNEL_EMAIL => self.email.clone(),
            CHANNEL_SMS | CHANNEL_WHATSAPP => self.phone.clone(),
            CHANNEL_IN_APP => self.user_id.clone(),
            _ => None,
        }
    }
}

fn first_string(record: &Value, paths: &[&str]) -> Option<String> {
    for path in paths {
        let mut cur = record;
        let mut found = true;
        for seg in path.split('.') {
            match cur {
                Value::Object(map) => {
                    if let Some(next) = map.get(seg) {
                        cur = next;
                    } else {
                        found = false;
                        break;
                    }
                }
                _ => {
                    found = false;
                    break;
                }
            }
        }
        if found {
            if let Some(s) = cur.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

pub fn validate_communication(def: &CommunicationDef, registry: &EntityRegistry) -> Vec<String> {
    let mut errors = Vec::new();
    if def.name.trim().is_empty() {
        errors.push("communication is missing a name".into());
    }
    if registry.try_get(&def.entity).is_none() {
        errors.push(format!(
            "communication '{}' references unknown entity '{}'",
            def.name, def.entity
        ));
        return errors;
    }
    if def.channels.is_empty() {
        errors.push(format!("communication '{}' has no channels", def.name));
    }
    for ch in &def.channels {
        if !CHANNELS.contains(&ch.as_str()) {
            errors.push(format!(
                "communication '{}' has unknown channel '{ch}'",
                def.name
            ));
        }
    }
    if !def.purpose.eq_ignore_ascii_case(PURPOSE_TRANSACTIONAL)
        && !def.purpose.eq_ignore_ascii_case(PURPOSE_MARKETING)
    {
        errors.push(format!(
            "communication '{}' has invalid purpose '{}'",
            def.name, def.purpose
        ));
    }
    if let Err(e) = reject_unsafe_template(&def.body) {
        errors.push(format!("communication '{}': {e}", def.name));
    }
    if let Some(subject) = &def.subject {
        if let Err(e) = reject_unsafe_template(subject) {
            errors.push(format!("communication '{}' subject: {e}", def.name));
        }
    }
    let mut src = def.body.clone();
    if let Some(subject) = &def.subject {
        src.push(' ');
        src.push_str(subject);
    }
    for err in validate_template_paths(&src, &def.entity, registry) {
        errors.push(format!("communication '{}': {err}", def.name));
    }
    if let Some(path) = &def.recipient_path {
        let probe = format!("{{{{ {path} }}}}");
        for err in validate_template_paths(&probe, &def.entity, registry) {
            errors.push(format!("communication '{}' recipient: {err}", def.name));
        }
    }
    let _ = template_paths(&src);
    errors
}

pub fn reject_unsafe_communication_payload(payload: &Value) -> QefroResult<()> {
    let blob = payload.to_string();
    reject_unsafe_template(&blob)?;
    let lower = blob.to_ascii_lowercase();
    if lower.contains("javascript:")
        || lower.contains("<script")
        || lower.contains("select ")
        || lower.contains("insert ")
        || lower.contains("drop ")
    {
        return Err(QefroError::bad_request(
            "communication templates reject JavaScript, SQL, and executable markup",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityDef;
    use crate::field::FieldDef;
    use serde_json::json;

    fn registry() -> EntityRegistry {
        let mut r = EntityRegistry::new();
        r.register(
            EntityDef::new("Customer")
                .field(FieldDef::string("name"))
                .field(FieldDef::string("email").nullable())
                .field(FieldDef::string("phone").nullable())
                .field(
                    FieldDef::enum_values(
                        "communication_channel",
                        vec!["in_app", "email", "sms", "whatsapp", "none"],
                    )
                    .nullable(),
                )
                .field(FieldDef::boolean("marketing_opt_out").nullable())
                .build(),
        )
        .unwrap();
        r.register(
            EntityDef::new("Order")
                .field(FieldDef::string("doc_no").nullable())
                .field(FieldDef::many_to_one("customer_id", "Customer"))
                .field(FieldDef::currency("total").nullable())
                .build(),
        )
        .unwrap();
        r
    }

    #[test]
    fn fallback_puts_preferred_channel_first() {
        let def = CommunicationDef::new("order_confirmed", "order.confirmed", "Order")
            .channels(&[CHANNEL_WHATSAPP, CHANNEL_EMAIL, CHANNEL_IN_APP])
            .preferred_channel_field("communication_channel");
        let recipient = json!({ "communication_channel": "email", "email": "a@x.com" });
        assert_eq!(
            select_channels(&def, &recipient),
            vec!["email", "whatsapp", "in_app"]
        );
    }

    #[test]
    fn none_preference_skips_send() {
        let def = CommunicationDef::new("n", "e", "Order")
            .channels(&[CHANNEL_EMAIL])
            .preferred_channel_field("communication_channel");
        let recipient = json!({ "communication_channel": "none" });
        assert!(select_channels(&def, &recipient).is_empty());
    }

    #[test]
    fn marketing_opt_out_skips_send() {
        let def = CommunicationDef::new("promo", "customer.created", "Customer")
            .purpose(PURPOSE_MARKETING)
            .channels(&[CHANNEL_EMAIL])
            .opt_out_field("marketing_opt_out");
        let recipient = json!({ "marketing_opt_out": true, "email": "a@x.com" });
        assert!(select_channels(&def, &recipient).is_empty());
        let transactional = def.clone().purpose(PURPOSE_TRANSACTIONAL);
        assert_eq!(select_channels(&transactional, &recipient), vec!["email"]);
    }

    #[test]
    fn validate_unknown_entity_and_field() {
        let registry = registry();
        let bad = CommunicationDef::new("x", "e", "Missing").body("{{ name }}");
        let errs = validate_communication(&bad, &registry);
        assert!(errs.iter().any(|e| e.contains("unknown entity")));
        let bad_field =
            CommunicationDef::new("y", "order.confirmed", "Order").body("Hello {{ missing }}");
        let errs = validate_communication(&bad_field, &registry);
        assert!(errs.iter().any(|e| e.contains("unknown field")));
    }

    #[test]
    fn recipient_address_from_nested_person() {
        let rec = json!({
            "name": "Walk-in",
            "email": "",
            "person": { "name": "Ahmed", "email": "ahmed@x.com", "phone": "+1" }
        });
        let addr = RecipientAddress::from_record(&rec);
        assert_eq!(addr.email.as_deref(), Some("ahmed@x.com"));
        assert_eq!(addr.phone.as_deref(), Some("+1"));
    }

    #[test]
    fn rejects_javascript_in_body() {
        let registry = registry();
        let def = CommunicationDef::new("evil", "e", "Order").body("<script>alert(1)</script>");
        let errs = validate_communication(&def, &registry);
        assert!(!errs.is_empty());
    }
}
