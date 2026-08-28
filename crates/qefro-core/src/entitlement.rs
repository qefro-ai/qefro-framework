use serde::{Deserialize, Serialize};

/// Subscription-ready entitlement. No payment provider in V0.4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub name: String,
    pub label: String,
    pub apps: Vec<String>,
    pub features: Vec<String>,
    pub max_users: Option<u32>,
}

impl Plan {
    pub fn starter() -> Self {
        Self {
            name: "starter".into(),
            label: "Starter".into(),
            apps: vec!["crm".into()],
            features: vec!["agent_actions".into()],
            max_users: Some(3),
        }
    }

    pub fn growth() -> Self {
        Self {
            name: "growth".into(),
            label: "Growth".into(),
            apps: vec!["crm".into(), "restaurant".into()],
            features: vec!["agent_actions".into(), "advanced_reports".into()],
            max_users: Some(20),
        }
    }

    pub fn enterprise() -> Self {
        Self {
            name: "enterprise".into(),
            label: "Enterprise".into(),
            apps: Vec::new(),
            features: Vec::new(),
            max_users: None,
        }
    }

    /// Empty `apps` means every globally installed application is allowed.
    pub fn allows_app(&self, app: &str) -> bool {
        self.apps.is_empty() || self.apps.iter().any(|a| a == app)
    }
}

#[derive(Debug, Clone)]
pub struct Entitlements {
    plans: Vec<Plan>,
}

impl Default for Entitlements {
    fn default() -> Self {
        Self::new()
    }
}

impl Entitlements {
    pub fn new() -> Self {
        Self {
            plans: vec![Plan::starter(), Plan::growth(), Plan::enterprise()],
        }
    }

    pub fn plan(&self, name: Option<&str>) -> Plan {
        let name = name.unwrap_or("enterprise");
        self.plans
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .unwrap_or_else(Plan::enterprise)
    }

    /// Tenant `enabled_apps` intersected with the plan. Empty tenant list means
    /// all installed apps that the plan allows.
    pub fn resolve_apps(
        &self,
        installed: &[String],
        enabled: &[String],
        plan: Option<&str>,
    ) -> Vec<String> {
        let plan = self.plan(plan);
        let candidates: Vec<String> = if enabled.is_empty() {
            installed.to_vec()
        } else {
            enabled.to_vec()
        };
        candidates
            .into_iter()
            .filter(|a| installed.iter().any(|i| i == a) && plan.allows_app(a))
            .collect()
    }

    pub fn can_enable(&self, app: &str, installed: &[String], plan: Option<&str>) -> bool {
        installed.iter().any(|i| i == app) && self.plan(plan).allows_app(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_cannot_enable_restaurant() {
        let e = Entitlements::new();
        let installed = vec!["restaurant".into(), "crm".into()];
        assert!(!e.can_enable("restaurant", &installed, Some("starter")));
        assert!(e.can_enable("crm", &installed, Some("starter")));
        let apps = e.resolve_apps(&installed, &["restaurant".into()], Some("starter"));
        assert!(apps.is_empty());
    }
}
