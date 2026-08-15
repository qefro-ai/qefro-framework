use qefro_core::{OpContext, QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

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
pub const ROLE_HR: &str = "HR";
pub const ROLE_PUBLIC: &str = "Public";

/// Role access to fields at or below `level`. Level 0 is always allowed for
/// callers who already passed entity RBAC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldLevelGrant {
    pub role: String,
    pub entity: String,
    pub level: u8,
    #[serde(default = "default_true")]
    pub read: bool,
    #[serde(default = "default_true")]
    pub write: bool,
}

fn default_true() -> bool {
    true
}

impl FieldLevelGrant {
    pub fn new(role: impl Into<String>, entity: impl Into<String>, level: u8) -> Self {
        Self {
            role: role.into(),
            entity: entity.into(),
            level,
            read: true,
            write: true,
        }
    }

    pub fn read_only(mut self) -> Self {
        self.write = false;
        self
    }
}

#[derive(Debug, Default)]
struct PermissionOverlay {
    replaced: HashSet<String>,
    grants: HashMap<String, HashMap<String, HashSet<Action>>>,
    field_levels: Vec<FieldLevelGrant>,
}

#[derive(Debug, Clone, Default)]
pub struct PermissionRegistry {
    /// role -> entity -> actions
    grants: HashMap<String, HashMap<String, HashSet<Action>>>,
    field_levels: Vec<FieldLevelGrant>,
    overlay: Arc<RwLock<PermissionOverlay>>,
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

    pub fn grant_field_level(&mut self, grant: FieldLevelGrant) {
        self.field_levels.push(grant);
    }

    pub fn field_levels(&self) -> Vec<FieldLevelGrant> {
        let overlay = self.overlay.read().ok();
        let mut out = self.field_levels.clone();
        if let Some(overlay) = overlay.as_ref() {
            if !overlay.field_levels.is_empty() {
                let replaced: HashSet<_> = overlay
                    .field_levels
                    .iter()
                    .map(|g| g.entity.clone())
                    .collect();
                out.retain(|g| !replaced.contains(&g.entity));
                out.extend(overlay.field_levels.clone());
            }
        }
        out
    }

    pub fn overlay_field_levels(&self, entity: &str, grants: Vec<FieldLevelGrant>) {
        let Ok(mut overlay) = self.overlay.write() else {
            return;
        };
        overlay.field_levels.retain(|g| g.entity != entity);
        overlay
            .field_levels
            .extend(grants.into_iter().filter(|g| g.entity == entity));
    }

    /// Level 0 is visible to anyone who passed entity RBAC. Higher levels need
    /// a grant whose `level` is >= the field's permission level.
    pub fn can_read_field(&self, ctx: &OpContext, entity: &str, level: u8) -> bool {
        self.can_field(ctx, entity, level, false)
    }

    pub fn can_write_field(&self, ctx: &OpContext, entity: &str, level: u8) -> bool {
        self.can_field(ctx, entity, level, true)
    }

    fn can_field(&self, ctx: &OpContext, entity: &str, level: u8, write: bool) -> bool {
        if ctx.is_admin() {
            return true;
        }
        if level == 0 {
            return true;
        }
        self.field_levels().iter().any(|g| {
            g.entity == entity
                && ctx.has_role(&g.role)
                && g.level >= level
                && if write { g.write } else { g.read }
        })
    }

    pub fn grants(&self) -> Vec<PermissionGrant> {
        let overlay = self.overlay.read().ok();
        let replaced = overlay
            .as_ref()
            .map(|o| o.replaced.clone())
            .unwrap_or_default();
        let mut merged: HashMap<String, HashMap<String, HashSet<Action>>> = self.grants.clone();
        for entity in &replaced {
            for role_map in merged.values_mut() {
                role_map.remove(entity);
            }
        }
        if let Some(overlay) = overlay.as_ref() {
            for (role, entities) in &overlay.grants {
                let role_map = merged.entry(role.clone()).or_default();
                for (entity, actions) in entities {
                    role_map.insert(entity.clone(), actions.clone());
                }
            }
        }
        let mut out = Vec::new();
        for (role, entities) in merged {
            for (entity, actions) in entities {
                let mut acts: Vec<_> = actions.into_iter().collect();
                acts.sort_by_key(|a| a.as_str());
                out.push(PermissionGrant {
                    role: role.clone(),
                    entity,
                    actions: acts,
                });
            }
        }
        out.sort_by(|a, b| a.role.cmp(&b.role).then(a.entity.cmp(&b.entity)));
        out
    }

    /// Replace grants for one entity in the live overlay. Boot grants stay put.
    pub fn overlay_entity(&self, entity: &str, grants: Vec<PermissionGrant>) {
        let Ok(mut overlay) = self.overlay.write() else {
            return;
        };
        overlay.replaced.insert(entity.to_string());
        for role_map in overlay.grants.values_mut() {
            role_map.remove(entity);
        }
        for grant in grants {
            if grant.entity != entity {
                continue;
            }
            overlay
                .grants
                .entry(grant.role)
                .or_default()
                .entry(grant.entity)
                .or_default()
                .extend(grant.actions);
        }
    }

    pub fn allows(&self, roles: &[String], entity: &str, action: Action) -> bool {
        if roles.iter().any(|r| r.eq_ignore_ascii_case(ROLE_ADMIN)) {
            return true;
        }
        let overlay = self.overlay.read().ok();
        let replaced = overlay
            .as_ref()
            .map(|o| o.replaced.contains(entity))
            .unwrap_or(false);
        for role in roles {
            if replaced {
                if let Some(overlay) = overlay.as_ref() {
                    if actions_allow(&overlay.grants, role, entity, action) {
                        return true;
                    }
                }
            } else if actions_allow(&self.grants, role, entity, action) {
                return true;
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

fn actions_allow(
    grants: &HashMap<String, HashMap<String, HashSet<Action>>>,
    role: &str,
    entity: &str,
    action: Action,
) -> bool {
    let Some(entities) = grants.get(role) else {
        return false;
    };
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
    false
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

    #[test]
    fn field_levels_hide_sensitive_fields() {
        let mut perms = PermissionRegistry::new();
        perms.grant(PermissionGrant::crud(ROLE_STAFF, "Employee"));
        perms.grant(PermissionGrant::crud(ROLE_HR, "Employee"));
        perms.grant_field_level(FieldLevelGrant::new(ROLE_HR, "Employee", 2));
        let staff = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Staff".into()]);
        let hr = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["HR".into()]);
        assert!(perms.can_read_field(&staff, "Employee", 0));
        assert!(!perms.can_read_field(&staff, "Employee", 2));
        assert!(!perms.can_write_field(&staff, "Employee", 2));
        assert!(perms.can_read_field(&hr, "Employee", 2));
        assert!(perms.can_write_field(&hr, "Employee", 2));
    }

    #[test]
    fn overlay_replaces_entity_grants() {
        let mut perms = PermissionRegistry::new();
        perms.grant(PermissionGrant::crud(ROLE_STAFF, "Payment"));
        let staff = OpContext::new(Uuid::nil(), Uuid::nil(), vec!["Staff".into()]);
        assert!(perms.check(&staff, "Payment", Action::Create).is_ok());
        perms.overlay_entity(
            "Payment",
            vec![PermissionGrant::read(ROLE_STAFF, "Payment")],
        );
        assert!(perms.check(&staff, "Payment", Action::Create).is_err());
        assert!(perms.check(&staff, "Payment", Action::Read).is_ok());
    }
}
