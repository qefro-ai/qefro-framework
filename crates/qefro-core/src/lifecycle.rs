use serde::{Deserialize, Serialize};

/// Declarative application lifecycle hook. Apps must not execute shell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleHookDef {
    /// `install`, `upgrade`, `uninstall`, `tenant_enable`, `tenant_disable`
    pub on: String,
    #[serde(default)]
    pub seed_kinds: Vec<String>,
}

impl LifecycleHookDef {
    pub fn event_ok(&self) -> bool {
        matches!(
            self.on.as_str(),
            "install" | "upgrade" | "uninstall" | "tenant_enable" | "tenant_disable"
        )
    }
}

pub fn lifecycle_event_name(on: &str) -> &'static str {
    match on {
        "install" => "app.installed",
        "upgrade" => "app.updated",
        "uninstall" => "app.uninstalled",
        "tenant_enable" => "app.enabled",
        "tenant_disable" => "app.disabled",
        _ => "app.lifecycle",
    }
}
