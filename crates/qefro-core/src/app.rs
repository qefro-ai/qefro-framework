use crate::document::{PrintFormat, ReportDef};
use crate::entity::EntityDef;
use crate::error::QefroResult;
use crate::hook::{EntityHook, HookRegistry};
use crate::platform::{NotificationDef, WebhookDef};
use crate::registry::EntityRegistry;
use crate::ui::DashboardDef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An installable application module: entities, plus extension points other
/// crates fill in (workflows, permissions, tools).
#[derive(Clone, Default)]
pub struct AppModule {
    pub name: String,
    pub version: String,
    pub label: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub api_version: String,
    pub framework_version: String,
    pub source: String,
    pub dependencies: BTreeMap<String, String>,
    pub navigation: Vec<NavItem>,
    pub entities: Vec<EntityDef>,
    pub hooks: HookRegistry,
    pub dashboards: Vec<DashboardDef>,
    pub reports: Vec<ReportDef>,
    pub print_formats: Vec<PrintFormat>,
    pub notifications: Vec<NotificationDef>,
    pub webhooks: Vec<WebhookDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NavItem {
    pub label: String,
    pub entity: String,
}

impl NavItem {
    pub fn new(label: impl Into<String>, entity: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            entity: entity.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default = "default_api_version")]
    pub api_version: String,
    #[serde(default)]
    pub framework_version: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub navigation: Vec<NavItem>,
    /// Legacy alias. Prefer `[dependencies]`.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_api_version() -> String {
    crate::version::APP_API_VERSION.to_string()
}

impl AppManifest {
    pub fn from_module(module: &AppModule) -> Self {
        let depends_on: Vec<String> = if module.dependencies.is_empty() {
            vec!["qefro-framework".into()]
        } else {
            module.dependencies.keys().cloned().collect()
        };
        Self {
            name: module.name.clone(),
            version: module.version.clone(),
            label: module.label.clone(),
            description: module.description.clone(),
            author: module.author.clone(),
            license: module.license.clone(),
            api_version: if module.api_version.is_empty() {
                default_api_version()
            } else {
                module.api_version.clone()
            },
            framework_version: module.framework_version.clone(),
            source: module.source.clone(),
            dependencies: module.dependencies.clone(),
            navigation: module.navigation.clone(),
            depends_on,
        }
    }
}

impl AppModule {
    pub fn new(name: impl Into<String>) -> AppModuleBuilder {
        AppModuleBuilder {
            module: AppModule {
                name: name.into(),
                version: "0.1.0".into(),
                label: String::new(),
                description: String::new(),
                author: String::new(),
                license: String::new(),
                api_version: default_api_version(),
                framework_version: crate::version::FRAMEWORK_COMPAT_REQ.into(),
                source: "catalog".into(),
                dependencies: BTreeMap::new(),
                navigation: Vec::new(),
                entities: Vec::new(),
                hooks: HookRegistry::new(),
                dashboards: Vec::new(),
                reports: Vec::new(),
                print_formats: Vec::new(),
                notifications: Vec::new(),
                webhooks: Vec::new(),
            },
        }
    }

    pub fn install_entities(&self, registry: &mut EntityRegistry) -> QefroResult<()> {
        for mut entity in self.entities.clone() {
            if entity.module.is_none() {
                entity.module = Some(self.name.clone());
            }
            registry.register(entity)?;
        }
        Ok(())
    }

    pub fn default_nav_slugs(&self) -> Vec<String> {
        self.navigation
            .iter()
            .filter_map(|item| {
                self.entities
                    .iter()
                    .find(|e| e.name == item.entity || e.slug == item.entity)
                    .map(|e| e.slug.clone())
            })
            .collect()
    }

    /// Standalone entities omitted from this module's default navigation.
    /// Empty when the module does not declare navigation, so other apps stay visible.
    pub fn default_hidden_slugs(&self) -> Vec<String> {
        if self.navigation.is_empty() {
            return Vec::new();
        }
        let nav_slugs: std::collections::HashSet<String> =
            self.default_nav_slugs().into_iter().collect();
        let nav_names: std::collections::HashSet<&str> = self
            .navigation
            .iter()
            .map(|item| item.entity.as_str())
            .collect();
        self.entities
            .iter()
            .filter(|entity| entity.standalone && entity.child_of.is_none())
            .filter(|entity| {
                !nav_slugs.contains(&entity.slug)
                    && !nav_names.contains(entity.name.as_str())
                    && !nav_names.contains(entity.slug.as_str())
            })
            .map(|entity| entity.slug.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityDef;

    #[test]
    fn hidden_slugs_are_standalone_entities_not_in_nav() {
        let module = AppModule::new("shop")
            .nav(NavItem::new("Orders", "Order"))
            .entity(EntityDef::new("Order").build())
            .entity(EntityDef::new("Product").build())
            .entity(
                EntityDef::new("OrderItem")
                    .child_of("Order", "items")
                    .build(),
            )
            .build();
        assert_eq!(module.default_nav_slugs(), vec!["orders"]);
        assert_eq!(module.default_hidden_slugs(), vec!["products"]);
    }

    #[test]
    fn no_navigation_hides_nothing() {
        let module = AppModule::new("shop")
            .entity(EntityDef::new("Order").build())
            .build();
        assert!(module.default_nav_slugs().is_empty());
        assert!(module.default_hidden_slugs().is_empty());
    }
}

pub struct AppModuleBuilder {
    module: AppModule,
}

impl AppModuleBuilder {
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.module.version = version.into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.module.label = label.into();
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.module.description = text.into();
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.module.author = author.into();
        self
    }

    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.module.license = license.into();
        self
    }

    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.module.api_version = version.into();
        self
    }

    pub fn framework_version(mut self, req: impl Into<String>) -> Self {
        self.module.framework_version = req.into();
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.module.source = source.into();
        self
    }

    pub fn dependency(mut self, name: impl Into<String>, req: impl Into<String>) -> Self {
        self.module.dependencies.insert(name.into(), req.into());
        self
    }

    pub fn nav(mut self, item: NavItem) -> Self {
        self.module.navigation.push(item);
        self
    }

    pub fn entity(mut self, entity: EntityDef) -> Self {
        self.module.entities.push(entity);
        self
    }

    pub fn hook(mut self, hook: Arc<dyn EntityHook>) -> Self {
        self.module.hooks.register(hook);
        self
    }

    pub fn dashboard(mut self, dashboard: DashboardDef) -> Self {
        self.module.dashboards.push(dashboard);
        self
    }

    pub fn report(mut self, report: ReportDef) -> Self {
        self.module.reports.push(report);
        self
    }

    pub fn print_format(mut self, format: PrintFormat) -> Self {
        self.module.print_formats.push(format);
        self
    }

    pub fn notification(mut self, def: NotificationDef) -> Self {
        self.module.notifications.push(def);
        self
    }

    pub fn webhook(mut self, def: WebhookDef) -> Self {
        self.module.webhooks.push(def);
        self
    }

    pub fn build(mut self) -> AppModule {
        if self.module.label.is_empty() {
            self.module.label = self.module.name.clone();
        }
        for entity in &mut self.module.entities {
            if entity.module.is_none() {
                entity.module = Some(self.module.name.clone());
            }
            entity.normalize();
        }
        self.module
    }
}
