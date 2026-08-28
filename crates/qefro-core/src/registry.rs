use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::field::FieldDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
struct RegistryOverlay {
    by_name: HashMap<String, Arc<EntityDef>>,
    by_slug: HashMap<String, String>,
}

/// Process-wide registry of entity metadata. Lookups are keyed by name and slug.
///
/// Boot registrations are immutable. Studio publishes land in `overlay` so the
/// same registry stays authoritative without a second metadata system.
#[derive(Debug, Clone, Default)]
pub struct EntityRegistry {
    by_name: HashMap<String, Arc<EntityDef>>,
    by_slug: HashMap<String, String>,
    by_table: HashMap<String, String>,
    overlay: Arc<RwLock<RegistryOverlay>>,
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
        if let Some(def) = self.overlay_get(name) {
            return Ok(def);
        }
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
        let mut map = self.by_name.clone();
        if let Ok(overlay) = self.overlay.read() {
            for (name, def) in &overlay.by_name {
                map.insert(name.clone(), def.clone());
            }
        }
        let mut items: Vec<_> = map.into_values().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.list().into_iter().map(|e| e.name.clone()).collect();
        names.sort();
        names
    }

    fn overlay_get(&self, name: &str) -> Option<Arc<EntityDef>> {
        let overlay = self.overlay.read().ok()?;
        overlay.by_name.get(name).cloned().or_else(|| {
            overlay
                .by_slug
                .get(name)
                .and_then(|n| overlay.by_name.get(n).cloned())
        })
    }

    /// Replace or insert an entity in the live overlay. Boot source is unchanged.
    pub fn overlay_put(&self, mut def: EntityDef) -> QefroResult<()> {
        def.normalize();
        def.validate_idents()?;
        let mut overlay = self
            .overlay
            .write()
            .map_err(|e| QefroError::internal(format!("registry overlay: {e}")))?;
        overlay.by_slug.insert(def.slug.clone(), def.name.clone());
        overlay.by_name.insert(def.name.clone(), Arc::new(def));
        Ok(())
    }

    pub fn overlay_remove(&self, name: &str) {
        if let Ok(mut overlay) = self.overlay.write() {
            if let Some(def) = overlay.by_name.remove(name) {
                overlay.by_slug.remove(&def.slug);
            }
        }
    }

    pub fn is_overlay(&self, name: &str) -> bool {
        self.overlay_get(name).is_some()
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
            if let Some(child_of) = &entity.child_of {
                if self.try_get(&child_of.parent_entity).is_none() {
                    return Err(QefroError::bad_request(format!(
                        "Entity '{}' is child_of unknown parent '{}'",
                        entity.name, child_of.parent_entity
                    )));
                }
            }
            crate::formula::detect_cycles(&entity.fields)?;
            self.validate_formulas(&entity)?;
        }
        Ok(())
    }

    /// Replace a boot-registered entity. Used after application modules land
    /// so Person can gain inverse one-to-many fields for `person_id`.
    pub fn replace(&mut self, mut def: EntityDef) -> QefroResult<()> {
        def.normalize();
        def.validate_idents()?;
        if let Some(old) = self.by_name.remove(&def.name) {
            self.by_slug.remove(&old.slug);
            self.by_table.remove(&old.table);
        }
        self.by_slug.insert(def.slug.clone(), def.name.clone());
        self.by_table.insert(def.table.clone(), def.name.clone());
        self.by_name.insert(def.name.clone(), Arc::new(def));
        Ok(())
    }

    /// Attach Person ← business-entity inverses for every `person_id` field.
    pub fn wire_identity_inverses(&mut self) -> QefroResult<()> {
        let Ok(person) = self.get(crate::identity::PERSON_ENTITY) else {
            return Ok(());
        };
        let mut person = (*person).clone();
        let listed = self.list();
        let backrefs = crate::identity::person_backrefs(listed.iter().map(|e| e.as_ref()));
        if crate::identity::apply_person_backrefs(&mut person, backrefs) {
            self.replace(person)?;
        }
        Ok(())
    }

    fn validate_formulas(&self, entity: &EntityDef) -> QefroResult<()> {
        for field in &entity.fields {
            if !field.computed {
                continue;
            }
            let Some(formula) = &field.formula else {
                return Err(QefroError::bad_request(format!(
                    "computed field '{}.{}' is missing a formula",
                    entity.name, field.name
                )));
            };
            let expr = crate::formula::parse_formula(formula)?;
            for dep in crate::formula::formula_dependencies(&expr) {
                if let Some((table, child_field)) = dep.split_once('.') {
                    let child_table = entity.fields.iter().find(|f| f.name == table);
                    let Some(child_table) = child_table.filter(|f| f.is_child_table()) else {
                        return Err(QefroError::bad_request(format!(
                            "formula on '{}.{}' references unknown child table '{table}'",
                            entity.name, field.name
                        )));
                    };
                    let target = child_table
                        .relation
                        .as_ref()
                        .map(|r| r.target_entity.as_str())
                        .unwrap_or("");
                    let child = self.get(target)?;
                    if child.get_field(child_field).is_none() {
                        return Err(QefroError::bad_request(format!(
                            "formula on '{}.{}' references unknown field '{dep}'",
                            entity.name, field.name
                        )));
                    }
                } else if entity.get_field(&dep).is_none()
                    && !entity
                        .fields
                        .iter()
                        .any(|f| f.is_child_table() && f.name == dep)
                {
                    return Err(QefroError::bad_request(format!(
                        "formula on '{}.{}' references unknown field '{dep}'",
                        entity.name, field.name
                    )));
                }
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
        let updated = EntityDef::new("Customer")
            .field(FieldDef::string("name").required().label("Guest"))
            .build();
        reg.overlay_put(updated).unwrap();
        assert_eq!(
            reg.get("Customer")
                .unwrap()
                .get_field("name")
                .unwrap()
                .label,
            "Guest"
        );
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

    #[test]
    fn wire_identity_inverses_adds_person_backrefs() {
        let mut reg = EntityRegistry::new();
        reg.register(crate::identity::person_entity()).unwrap();
        reg.register(
            EntityDef::new("Customer")
                .table_name("customers")
                .slug_name("customers")
                .field(FieldDef::string("name").required())
                .field(FieldDef::string("email").required())
                .field(FieldDef::string("phone").nullable())
                .field(FieldDef::many_to_one("person_id", "Person").nullable())
                .build(),
        )
        .unwrap();
        reg.wire_identity_inverses().unwrap();
        let person = reg.get("Person").unwrap();
        let back = person.get_field("customers").expect("customers inverse");
        assert_eq!(
            back.relation.as_ref().unwrap().inverse_field.as_deref(),
            Some("person_id")
        );
        assert!(person.get_field("name").is_some());
        let customer = reg.get("Customer").unwrap();
        assert!(customer.get_field("name").is_some());
        assert!(customer.get_field("email").is_some());
        assert!(customer.get_field("phone").is_some());
        assert!(customer.get_field("person_id").unwrap().nullable);
    }
}
