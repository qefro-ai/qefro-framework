use crate::ui::DashboardDef;
use crate::entity::EntityDef;
use crate::error::QefroResult;
use crate::hook::{EntityHook, HookRegistry};
use crate::registry::EntityRegistry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// An installable application module: entities, plus extension points other
/// crates fill in (workflows, permissions, tools).
#[derive(Clone, Default)]
pub struct AppModule {
    pub name: String,
    pub version: String,
    pub label: String,
    pub description: String,
    pub entities: Vec<EntityDef>,
    pub hooks: HookRegistry,
    pub dashboards: Vec<DashboardDef>,
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
    pub depends_on: Vec<String>,
}

impl AppModule {
    pub fn new(name: impl Into<String>) -> AppModuleBuilder {
        AppModuleBuilder {
            module: AppModule {
                name: name.into(),
                version: "0.1.0".into(),
                label: String::new(),
                description: String::new(),
                entities: Vec::new(),
                hooks: HookRegistry::new(),
                dashboards: Vec::new(),
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
