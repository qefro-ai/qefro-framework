use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Role assigned to background job execution. Not Admin. Not a user principal.
pub const ROLE_WORKER: &str = "Worker";
/// Anonymous public-form principal. Never Admin.
pub const ROLE_PUBLIC: &str = "Public";

/// Server-side operation context. Tenant identity is taken from the
/// authenticated session, never from an untrusted client field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub roles: Vec<String>,
    pub request_id: Uuid,
    #[serde(default)]
    pub session_id: Option<Uuid>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    /// Empty means all globally installed apps (V0.3 default).
    #[serde(default)]
    pub enabled_apps: Vec<String>,
    #[serde(default)]
    pub features: std::collections::HashMap<String, bool>,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub plan: Option<String>,
    /// Display name of the acting user. Never agent chain-of-thought.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_name: Option<String>,
    /// `user` (default), `agent`, `worker`, or `system`.
    #[serde(default)]
    pub source: String,
}

impl OpContext {
    pub fn new(tenant_id: Uuid, user_id: Uuid, roles: Vec<String>) -> Self {
        Self {
            tenant_id,
            user_id,
            roles,
            request_id: Uuid::new_v4(),
            session_id: None,
            ip: None,
            user_agent: None,
            enabled_apps: Vec::new(),
            features: Default::default(),
            timezone: "UTC".into(),
            locale: "en-US".into(),
            currency: "USD".into(),
            plan: None,
            actor_name: None,
            source: String::new(),
        }
    }

    pub fn worker(tenant_id: Uuid, user_id: Uuid) -> Self {
        let mut ctx = Self::new(tenant_id, user_id, vec![ROLE_WORKER.into()]);
        ctx.request_id = Uuid::new_v4();
        ctx
    }

    /// Restricted public-form context. Not Admin. Cannot switch tenant.
    pub fn public(tenant_id: Uuid) -> Self {
        Self::new(tenant_id, Uuid::nil(), vec![ROLE_PUBLIC.into()])
    }

    pub fn is_public(&self) -> bool {
        self.roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case(ROLE_PUBLIC))
    }

    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case("Admin"))
    }

    pub fn is_worker(&self) -> bool {
        self.roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case(ROLE_WORKER))
            || self.source.eq_ignore_ascii_case("worker")
    }

    pub fn is_agent(&self) -> bool {
        self.source.eq_ignore_ascii_case("agent")
    }

    /// Business-facing actor label for Activity. Audit still stores user_id.
    pub fn activity_actor_name(&self) -> String {
        if self.is_agent() {
            return "Qefro Agent".into();
        }
        if self.is_worker() || self.source.eq_ignore_ascii_case("system") {
            return "System".into();
        }
        self.actor_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "User".into())
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
    }

    /// Application modules the tenant may use. Empty `enabled_apps` allows all.
    pub fn allows_app(&self, module: Option<&str>) -> bool {
        let Some(module) = module else {
            return true;
        };
        if self.enabled_apps.is_empty() {
            return true;
        }
        self.enabled_apps.iter().any(|a| a == module)
    }

    /// Missing flags are unrestricted. Explicit `false` denies.
    pub fn feature_allowed(&self, name: &str) -> bool {
        self.features.get(name).copied().unwrap_or(true)
    }

    pub fn apply_tenant_config(&mut self, config: &crate::TenantConfig) {
        self.enabled_apps = config.enabled_apps.clone();
        self.features = config.features.flags.clone();
        self.timezone = config.business.timezone.clone();
        self.locale = config.business.locale.clone();
        self.currency = config.business.currency.clone();
        self.plan = config.plan.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_detection() {
        let ctx = OpContext::new(Uuid::new_v4(), Uuid::new_v4(), vec!["Manager".into()]);
        assert!(!ctx.is_admin());
        assert!(ctx.has_role("manager"));
        assert!(!ctx.is_worker());
    }

    #[test]
    fn worker_is_not_admin() {
        let ctx = OpContext::worker(Uuid::new_v4(), Uuid::new_v4());
        assert!(ctx.is_worker());
        assert!(!ctx.is_admin());
    }

    #[test]
    fn app_and_feature_gates() {
        let mut ctx = OpContext::new(Uuid::new_v4(), Uuid::new_v4(), vec!["Staff".into()]);
        assert!(ctx.allows_app(Some("crm")));
        ctx.enabled_apps = vec!["restaurant".into()];
        assert!(ctx.allows_app(Some("restaurant")));
        assert!(!ctx.allows_app(Some("crm")));
        ctx.features.insert("agent_actions".into(), false);
        assert!(!ctx.feature_allowed("agent_actions"));
        assert!(ctx.feature_allowed("unknown"));
    }

    #[test]
    fn activity_actor_labels_agents_without_reasoning() {
        let mut ctx = OpContext::new(Uuid::new_v4(), Uuid::new_v4(), vec!["Staff".into()]);
        ctx.actor_name = Some("Ahmed".into());
        assert_eq!(ctx.activity_actor_name(), "Ahmed");
        ctx.source = "agent".into();
        assert_eq!(ctx.activity_actor_name(), "Qefro Agent");
    }
}
