use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::field::FieldDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Process-wide registry of entity metadata. Lookups are keyed by name and slug.
#[derive(Debug, Clone, Default)]
pub struct EntityRegistry {
    by_name: HashMap<String, Arc<EntityDef>>,
    by_slug: HashMap<String, String>,
    by_table: HashMap<String, String>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, mut def: EntityDef) -> QefroResult<()> {
        def.normalize();
        def.validate_idents()?;
        if self.by_name.contains_key(&def.name) {
            return Err(QefroError::conflict(format!(
                "entity '{}' is already registered",
                def.name
            )));
        }
        if self.by_slug.contains_key(&def.slug) {
            return Err(QefroError::conflict(format!(
                "entity slug '{}' is already registered",
                def.slug
            )));
        }
        self.by_slug.insert(def.slug.clone(), def.name.clone());
        self.by_table.insert(def.table.clone(), def.name.clone());
        self.by_name.insert(def.name.clone(), Arc::new(def));
        Ok(())
    }

    pub fn get(&self, name: &str) -> QefroResult<Arc<EntityDef>> {
        self.by_name
            .get(name)
            .cloned()
            .or_else(|| {
                self.by_slug
                    .get(name)
                    .and_then(|n| self.by_name.get(n).cloned())
            })
            .ok_or_else(|| QefroError::not_found(format!("entity '{name}' not found")))
    }

    pub fn try_get(&self, name: &str) -> Option<Arc<EntityDef>> {
        self.get(name).ok()
    }

    pub fn list(&self) -> Vec<Arc<EntityDef>> {
        let mut items: Vec<_> = self.by_name.values().cloned().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.by_name.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn load_dir(&mut self, dir: &Path) -> QefroResult<usize> {
        let mut count = 0;
        if !dir.exists() {
            return Ok(0);
        }
        for entry in std::fs::read_dir(dir)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", dir.display())))?
        {
            let entry = entry.map_err(|e| QefroError::internal(e.to_string()))?;
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(ext, "yaml" | "yml" | "json") {
                self.register(EntityDef::from_file(&path)?)?;
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn validate_relations(&self) -> QefroResult<()> {
        let names = self.names();
        for entity in self.list() {
            for field in &entity.fields {
                let Some(rel) = &field.relation else { continue };
                if self.try_get(&rel.target_entity).is_some() {
                    continue;
                }
                let suggestion = crate::ident::suggest_similar(
                    &rel.target_entity,
                    names.iter().map(|s| s.as_str()),
                );
                let hint = suggestion
                    .map(|s| format!(" Did you mean '{s}'?"))
                    .unwrap_or_default();
                return Err(QefroError::bad_request(format!(
                    "Entity '{}' references unknown entity '{}'.{hint}",
                    entity.name, rel.target_entity
                )));
            }
        }
        Ok(())
    }

    pub fn field<'a>(&self, entity: &'a EntityDef, name: &str) -> QefroResult<&'a FieldDef> {
        entity.get_field(name).ok_or_else(|| {
            QefroError::bad_request(format!("unknown field '{name}' on {}", entity.name))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    pub entities: Vec<EntityDef>,
}

impl From<&EntityRegistry> for RegistrySnapshot {
    fn from(reg: &EntityRegistry) -> Self {
        Self {
            entities: reg.list().iter().map(|e| (**e).clone()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldDef;

    #[test]
    fn register_and_lookup() {
        let mut reg = EntityRegistry::new();
        reg.register(
            EntityDef::new("Customer")
                .field(FieldDef::string("name").required())
                .build(),
        )
        .unwrap();
        assert_eq!(reg.get("Customer").unwrap().name, "Customer");
        assert_eq!(reg.get("customers").unwrap().name, "Customer");
        assert!(reg.get("Order").is_err());
            assert!(reg.register(EntityDef::new("Customer").build()).is_err());
    }

    #[test]
    fn unknown_relation_suggests() {
        let mut reg = EntityRegistry::new();
        reg.register(
            EntityDef::new("Reservation")
                .field(FieldDef::many_to_one("table_id", "Table"))
                .build(),
        )
        .unwrap();
        reg.register(EntityDef::new("DiningTable").build()).unwrap();
        let err = reg.validate_relations().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown entity 'Table'"));
        assert!(msg.contains("DiningTable"));
    }
}
