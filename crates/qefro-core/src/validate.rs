use crate::bundle::AppBundle;
use crate::error::{QefroError, QefroResult};
use crate::field::RelationKind;
use crate::registry::EntityRegistry;
use crate::version::{self, is_framework_dep};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn fail(self) -> QefroResult<()> {
        if self.ok() {
            Ok(())
        } else {
            Err(QefroError::bad_request(self.errors.join("\n")))
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

#[derive(Debug, Clone)]
pub struct InstalledAppRef {
    pub name: String,
    pub version: String,
}

pub fn validate_bundle(bundle: &AppBundle, installed: &[InstalledAppRef]) -> ValidationReport {
    let mut report = ValidationReport::default();
    validate_manifest(&bundle.manifest.name, &bundle.manifest.version, &mut report);
    if bundle.manifest.label.is_empty() {
        report.error("manifest is missing label");
    }
    if bundle.manifest.api_version.trim() != version::APP_API_VERSION.to_string()
        && !bundle.manifest.api_version.is_empty()
    {
        match bundle.manifest.api_version.parse::<u32>() {
            Ok(n) if n > version::APP_API_VERSION => report.error(format!(
                "unsupported api_version {} (runtime supports {})",
                bundle.manifest.api_version,
                version::APP_API_VERSION
            )),
            Ok(_) => {}
            Err(_) => report.error(format!(
                "invalid api_version '{}'",
                bundle.manifest.api_version
            )),
        }
    }
    if let Err(e) = version::compatible_with_framework(&bundle.manifest.framework_version) {
        report.error(e.to_string());
    }

    validate_name(&bundle.manifest.name, &mut report);

    let mut registry = EntityRegistry::new();
    let mut seen_entities = HashSet::new();
    for entity in &bundle.entities {
        if !seen_entities.insert(entity.name.clone()) {
            report.error(format!("duplicate entity '{}'", entity.name));
            continue;
        }
        if let Err(e) = registry.register(entity.clone()) {
            report.error(e.to_string());
        }
    }
    if let Err(e) = registry.validate_relations() {
        report.error(e.to_string());
    }

    for entity in &bundle.entities {
        if let Some(wf) = &entity.workflow {
            let found = bundle
                .workflows
                .iter()
                .any(|w| w.get("name").and_then(|v| v.as_str()) == Some(wf.as_str()));
            if !found {
                report.error(format!(
                    "entity '{}' references missing workflow '{}'",
                    entity.name, wf
                ));
            }
        }
        for table in &entity.child_tables {
            if registry.try_get(&table.child_entity).is_none() {
                report.error(format!(
                    "entity '{}' child table '{}' references missing entity '{}'",
                    entity.name, table.name, table.child_entity
                ));
            }
        }
        for field in &entity.fields {
            if let Some(rel) = &field.relation {
                if matches!(
                    rel.kind,
                    RelationKind::ChildTable
                        | RelationKind::ManyToOne
                        | RelationKind::OneToMany
                        | RelationKind::ManyToMany
                ) && registry.try_get(&rel.target_entity).is_none()
                {
                    report.error(format!(
                        "entity '{}' field '{}' references missing entity '{}'",
                        entity.name, field.name, rel.target_entity
                    ));
                }
            }
        }
    }

    let mut seen_wf = HashSet::new();
    for wf in &bundle.workflows {
        let Some(name) = wf.get("name").and_then(|v| v.as_str()) else {
            report.error("workflow is missing name");
            continue;
        };
        if !seen_wf.insert(name.to_string()) {
            report.error(format!("duplicate workflow '{name}'"));
        }
        let entity = wf.get("entity").and_then(|v| v.as_str()).unwrap_or("");
        if entity.is_empty() {
            report.error(format!("workflow '{name}' is missing entity"));
        } else if registry.try_get(entity).is_none() {
            report.error(format!(
                "workflow '{name}' references missing entity '{entity}'"
            ));
        }
    }

    for grant in &bundle.permissions {
        let entity = grant.get("entity").and_then(|v| v.as_str()).unwrap_or("");
        if entity.is_empty() {
            report.error("permission grant is missing entity");
        } else if registry.try_get(entity).is_none() {
            report.error(format!("permission references missing entity '{entity}'"));
        }
        if grant
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .is_empty()
        {
            report.error("permission grant is missing role");
        }
    }

    for report_def in &bundle.reports {
        if registry.try_get(&report_def.entity).is_none() {
            report.error(format!(
                "report '{}' references missing entity '{}'",
                report_def.name, report_def.entity
            ));
        }
    }
    for dash in &bundle.dashboards {
        for card in &dash.cards {
            if registry.try_get(&card.entity).is_none() && !card.entity.starts_with('_') {
                report.error(format!(
                    "dashboard '{}' card '{}' references missing entity '{}'",
                    dash.name, card.title, card.entity
                ));
            }
        }
    }
    let entity_slugs: Vec<String> = bundle.entities.iter().map(|e| e.slug.clone()).collect();
    let mut page_slugs = HashSet::new();
    let mut page_names = HashSet::new();
    for page in &bundle.pages {
        if !page_names.insert(page.name.clone()) {
            report.error(format!("duplicate page '{}'", page.name));
        }
        let slug = if page.slug.is_empty() {
            crate::ident::slugify(&page.name)
        } else {
            page.slug.clone()
        };
        if !page_slugs.insert(slug.clone()) {
            report.error(format!("duplicate page route '/pages/{slug}'"));
        }
        for err in crate::page::validate_page(
            page,
            &registry,
            &bundle.reports,
            &bundle.dashboards,
            &entity_slugs,
        ) {
            report.error(err);
        }
    }
    for fmt in &bundle.print_formats {
        for err in crate::document::validate_print_format(fmt, &registry) {
            report.error(err);
        }
    }
    for entity in &bundle.entities {
        for fmt in &entity.print_formats {
            for err in crate::document::validate_print_format(fmt, &registry) {
                report.error(err);
            }
        }
    }
    for def in &bundle.communications {
        for err in crate::communication::validate_communication(def, &registry) {
            report.error(err);
        }
    }
    for item in &bundle.manifest.navigation {
        if let Some(page_name) = &item.page {
            if !bundle
                .pages
                .iter()
                .any(|p| p.name == *page_name || p.slug == *page_name)
            {
                report.error(format!(
                    "navigation '{}' references missing page '{}'",
                    item.label, page_name
                ));
            }
            continue;
        }
        if item.entity.is_empty() {
            report.error(format!(
                "navigation '{}' is missing entity or page",
                item.label
            ));
            continue;
        }
        if registry.try_get(&item.entity).is_none() {
            report.error(format!(
                "navigation '{}' references missing entity '{}'",
                item.label, item.entity
            ));
        }
    }
    for seed in &bundle.seeds {
        if !seed.kind_ok() {
            report.error(format!("invalid seed kind '{}'", seed.kind));
        }
        if registry.try_get(&seed.entity).is_none() {
            report.error(format!("seed references missing entity '{}'", seed.entity));
        }
    }
    for hook in &bundle.hooks {
        if !hook.event_ok() {
            report.error(format!("invalid lifecycle hook '{}'", hook.on));
        }
    }
    for mig in &bundle.migrations {
        if version::parse_version(&mig.version).is_err() {
            report.error(format!(
                "migration '{}' has invalid version '{}'",
                mig.id, mig.version
            ));
        }
        if mig.looks_destructive() && mig.sql.trim().is_empty() {
            report.warn(format!("migration '{}' is marked destructive", mig.id));
        }
    }

    validate_dependencies(&bundle.manifest.dependencies, installed, &mut report);
    detect_cycles(
        bundle.manifest.name.as_str(),
        &bundle.manifest.dependencies,
        installed,
        &mut report,
    );

    if bundle.entities.is_empty() && bundle.manifest.source != "builtin" {
        report.warn("app has no entities");
    }

    report
}

pub fn validate_manifest(name: &str, version: &str, report: &mut ValidationReport) {
    if name.trim().is_empty() {
        report.error("manifest is missing name");
    }
    if version::parse_version(version).is_err() {
        report.error(format!("invalid version '{version}'"));
    }
}

pub fn validate_name(name: &str, report: &mut ValidationReport) {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        report.error(format!(
            "invalid app name '{name}' (use letters, numbers, '-' or '_')"
        ));
    }
}

fn validate_dependencies(
    deps: &BTreeMap<String, String>,
    installed: &[InstalledAppRef],
    report: &mut ValidationReport,
) {
    for (name, req) in deps {
        if is_framework_dep(name) {
            if let Err(e) = version::compatible_with_framework(req) {
                report.error(e.to_string());
            }
            continue;
        }
        let Some(found) = installed.iter().find(|a| a.name == *name) else {
            report.error(format!("missing dependency '{name}' ({req})"));
            continue;
        };
        match version::matches_req(&found.version, req) {
            Ok(true) => {}
            Ok(false) => report.error(format!(
                "dependency '{name}' {req} is not satisfied by installed {}",
                found.version
            )),
            Err(e) => report.error(e.to_string()),
        }
    }
}

fn detect_cycles(
    name: &str,
    deps: &BTreeMap<String, String>,
    installed: &[InstalledAppRef],
    report: &mut ValidationReport,
) {
    let mut visiting = HashSet::new();
    let mut stack = vec![name.to_string()];
    visiting.insert(name.to_string());
    fn walk(
        current: &str,
        deps: &BTreeMap<String, String>,
        installed: &[InstalledAppRef],
        visiting: &mut HashSet<String>,
        stack: &mut Vec<String>,
        report: &mut ValidationReport,
    ) {
        for dep in deps.keys() {
            if is_framework_dep(dep) {
                continue;
            }
            if visiting.contains(dep.as_str()) {
                report.error(format!(
                    "circular dependency: {} → {dep}",
                    stack.join(" → ")
                ));
                continue;
            }
            visiting.insert(dep.clone());
            stack.push(dep.clone());
            // Only the current app's deps are known here; installed apps do not
            // carry their graphs in this check. Direct A↔B via the current
            // bundle naming itself is still caught.
            if dep == current {
                report.error(format!("circular dependency: {dep} depends on itself"));
            }
            let _ = installed;
            stack.pop();
            visiting.remove(dep);
        }
    }
    walk(name, deps, installed, &mut visiting, &mut stack, report);
}

/// Compare two bundles of the same app. Fields present in `from` but missing
/// in `to` are potentially destructive (columns are not dropped automatically).
pub fn destructive_field_removals(from: &AppBundle, to: &AppBundle) -> Vec<String> {
    let mut out = Vec::new();
    for old in &from.entities {
        let Some(new) = to.entities.iter().find(|e| e.name == old.name) else {
            out.push(format!(
                "entity '{}' would disappear from metadata",
                old.name
            ));
            continue;
        };
        for field in old.stored_fields() {
            if new.get_field(&field.name).is_none() {
                out.push(format!("{}.{}", old.name, field.name));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::AppFileManifest;
    use crate::entity::EntityDef;
    use crate::field::FieldDef;
    use std::path::PathBuf;

    fn empty_bundle(name: &str) -> AppBundle {
        AppBundle {
            root: PathBuf::from("."),
            manifest: AppFileManifest {
                name: name.into(),
                version: "1.0.0".into(),
                label: name.into(),
                description: String::new(),
                author: String::new(),
                license: "MIT".into(),
                api_version: "1".into(),
                framework_version: crate::version::FRAMEWORK_COMPAT_REQ.into(),
                source: "catalog".into(),
                dependencies: BTreeMap::new(),
                navigation: Vec::new(),
                depends_on: Vec::new(),
                branding: Default::default(),
            },
            entities: vec![EntityDef::new("Customer")
                .field(FieldDef::string("name").required())
                .build()],
            workflows: Vec::new(),
            permissions: Vec::new(),
            reports: Vec::new(),
            dashboards: Vec::new(),
            pages: Vec::new(),
            print_formats: Vec::new(),
            communications: Vec::new(),
            seeds: Vec::new(),
            hooks: Vec::new(),
            migrations: Vec::new(),
            assets: Vec::new(),
        }
    }

    #[test]
    fn valid_minimal_app() {
        let report = validate_bundle(&empty_bundle("myshop"), &[]);
        assert!(report.ok(), "{:?}", report.errors);
    }

    #[test]
    fn missing_dependency_fails() {
        let mut bundle = empty_bundle("restaurant");
        bundle
            .manifest
            .dependencies
            .insert("inventory".into(), ">=1.0".into());
        let report = validate_bundle(&bundle, &[]);
        assert!(!report.ok());
        assert!(report.errors.iter().any(|e| e.contains("inventory")));
    }

    #[test]
    fn satisfied_dependency_ok() {
        let mut bundle = empty_bundle("restaurant");
        bundle
            .manifest
            .dependencies
            .insert("inventory".into(), ">=1.0,<2.0".into());
        let installed = vec![InstalledAppRef {
            name: "inventory".into(),
            version: "1.0.0".into(),
        }];
        let report = validate_bundle(&bundle, &installed);
        assert!(report.ok(), "{:?}", report.errors);
    }

    #[test]
    fn incompatible_dependency_version_fails() {
        let mut bundle = empty_bundle("restaurant");
        bundle
            .manifest
            .dependencies
            .insert("inventory".into(), ">=2.0".into());
        let installed = vec![InstalledAppRef {
            name: "inventory".into(),
            version: "1.0.0".into(),
        }];
        let report = validate_bundle(&bundle, &installed);
        assert!(!report.ok());
    }

    #[test]
    fn workflow_missing_entity_fails() {
        let mut bundle = empty_bundle("myshop");
        bundle.workflows.push(serde_json::json!({
            "name": "order",
            "entity": "Order",
            "initial": "Draft",
            "states": [],
            "transitions": []
        }));
        let report = validate_bundle(&bundle, &[]);
        assert!(report.errors.iter().any(|e| e.contains("Order")));
    }

    #[test]
    fn yaml_and_rust_customer_share_ui_shape() {
        let rust = EntityDef::new("Customer")
            .field(FieldDef::string("name").required().searchable())
            .build();
        let yaml = EntityDef::from_yaml(
            "name: Customer\nfields:\n  - name: name\n    type: string\n    required: true\n    searchable: true\n",
        )
        .unwrap();
        assert_eq!(rust.name, yaml.name);
        assert_eq!(rust.slug, yaml.slug);
        let ru = rust.to_ui_meta();
        let yu = yaml.to_ui_meta();
        assert_eq!(
            ru.fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
            yu.fields.iter().map(|f| &f.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn removed_field_is_reported_not_silently_dropped() {
        let from = empty_bundle("shop");
        let mut to = empty_bundle("shop");
        to.entities[0].fields.clear();
        let dropped = destructive_field_removals(&from, &to);
        assert!(dropped.iter().any(|d| d.contains("name")));
    }

    #[test]
    fn unknown_page_entity_fails() {
        let mut bundle = empty_bundle("shop");
        bundle
            .pages
            .push(crate::page::PageDef::new("ops", "Ops").section(
                crate::page::PageSection::entity_view("Ghost", "Missing", "list"),
            ));
        let report = validate_bundle(&bundle, &[]);
        assert!(!report.ok());
        assert!(report.errors.iter().any(|e| e.contains("unknown entity")));
    }

    #[test]
    fn valid_page_composition() {
        let mut bundle = empty_bundle("shop");
        bundle.pages.push(
            crate::page::PageDef::new("customer-workspace", "Customers")
                .layout("stack")
                .section(crate::page::PageSection::entity_view(
                    "Customers",
                    "Customer",
                    "list",
                )),
        );
        bundle
            .manifest
            .navigation
            .push(crate::app::NavItem::page_link(
                "Workspace",
                "customer-workspace",
            ));
        let report = validate_bundle(&bundle, &[]);
        assert!(report.ok(), "{:?}", report.errors);
    }
}
