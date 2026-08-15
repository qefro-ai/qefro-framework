use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case("Admin"))
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
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
    }
}
