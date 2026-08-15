use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    List,
    Export,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Read => "read",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::List => "list",
            Self::Export => "export",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "create" => Some(Self::Create),
            "read" => Some(Self::Read),
            "update" => Some(Self::Update),
            "delete" => Some(Self::Delete),
            "list" => Some(Self::List),
            "export" => Some(Self::Export),
            "crud" => None,
            _ => None,
        }
    }

    pub fn crud() -> Vec<Self> {
        vec![
            Self::Create,
            Self::Read,
            Self::Update,
            Self::Delete,
            Self::List,
        ]
    }

    pub fn all() -> Vec<Self> {
        let mut v = Self::crud();
        v.push(Self::Export);
        v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionGrant {
    pub role: String,
    pub entity: String,
    pub actions: Vec<Action>,
}

impl PermissionGrant {
    pub fn new(role: impl Into<String>, entity: impl Into<String>, actions: Vec<Action>) -> Self {
        Self {
            role: role.into(),
            entity: entity.into(),
            actions,
        }
    }

    pub fn crud(role: impl Into<String>, entity: impl Into<String>) -> Self {
        Self::new(role, entity, Action::crud())
    }

    pub fn read(role: impl Into<String>, entity: impl Into<String>) -> Self {
        Self::new(role, entity, vec![Action::Read, Action::List])
    }
}

/// Built-in roles. Applications may add more.
pub const ROLE_ADMIN: &str = "Admin";
pub const ROLE_MANAGER: &str = "Manager";
pub const ROLE_STAFF: &str = "Staff";
pub const ROLE_CUSTOMER: &str = "Customer";

#[derive(Debug, Clone, Default)]
pub struct PermissionRegistry {
    /// role -> entity -> actions
    grants: HashMap<String, HashMap<String, HashSet<Action>>>,
}

impl PermissionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&mut self, grant: PermissionGrant) {
        let role_map = self.grants.entry(grant.role).or_default();
        let actions = role_map.entry(grant.entity).or_default();
        actions.extend(grant.actions);
    }

    pub fn grant_all(&mut self, role: &str, entity: &str) {
        self.grant(PermissionGrant::new(role, entity, Action::all()));
    }

    pub fn grants(&self) -> Vec<PermissionGrant> {
        let mut out = Vec::new();
        for (role, entities) in &self.grants {
            for (entity, actions) in entities {
                let mut acts: Vec<_> = actions.iter().copied().collect();
                acts.sort_by_key(|a| a.as_str());
                out.push(PermissionGrant {
                    role: role.clone(),
                    entity: entity.clone(),
                    actions: acts,
                });
            }
        }
        out.sort_by(|a, b| a.role.cmp(&b.role).then(a.entity.cmp(&b.entity)));
        out
    }

    pub fn allows(&self, roles: &[String], entity: &str, action: Action) -> bool {
        if roles.iter().any(|r| r.eq_ignore_ascii_case(ROLE_ADMIN)) {
            return true;
        }
        for role in roles {
            if let Some(entities) = self.grants.get(role) {
                if let Some(actions) = entities.get(entity) {
                    if actions.contains(&action) {
                        return true;
                    }
                }
                if let Some(actions) = entities.get("*") {
                    if actions.contains(&action) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn check(&self, ctx: &OpContext, entity: &str, action: Action) -> QefroResult<()> {
        if self.allows(&ctx.roles, entity, action) {
            Ok(())
        } else {
            Err(QefroError::forbidden(format!(
                "role(s) {:?} cannot {} {}",
                ctx.roles,
                action.as_str(),
                entity
            )))
        }
    }

    /// Seed Admin with full access to an entity. Field- and record-level
    /// checks can be layered on later without changing this signature.
    pub fn ensure_admin(&mut self, entity: &str) {
        self.grant_all(ROLE_ADMIN, entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn rbac_matrix() {
        let mut perms = PermissionRegistry::new();
        perms.grant(PermissionGrant::crud(ROLE_MANAGER, "Reservation"));
        perms.grant(PermissionGrant::new(
            ROLE_STAFF,
            "Reservation",
            vec![Action::Read, Action::Update, Action::List],
        ));
        perms.grant(PermissionGrant::read(ROLE_CUSTOMER, "Reservation"));
        perms.ensure_admin("Reservation");

        let manager = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Manager".into()]);
        let staff = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Staff".into()]);
        let customer = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Customer".into()]);
        let admin = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Admin".into()]);

        assert!(perms.check(&manager, "Reservation", Action::Create).is_ok());
        assert!(perms.check(&staff, "Reservation", Action::Create).is_err());
        assert!(perms.check(&staff, "Reservation", Action::Update).is_ok());
        assert!(perms.check(&customer, "Reservation", Action::Read).is_ok());
        assert!(perms
            .check(&customer, "Reservation", Action::Delete)
            .is_err());
        assert!(perms.check(&admin, "Reservation", Action::Delete).is_ok());
    }
}
