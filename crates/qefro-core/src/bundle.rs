//! Load a Qefro application directory (catalog, store, or extracted package).

use crate::app::{AppManifest, AppModule};
use crate::catalog::{load_yaml_docs, parse_app_toml, AppFileManifest};
use crate::document::{PrintFormat, ReportDef};
use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::lifecycle::LifecycleHookDef;
use crate::migration::{parse_migration_file, AppMigration};
use crate::seed::{parse_seed_file, SeedBatch};
use crate::ui::DashboardDef;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Filesystem application package before it is turned into `InstalledApp`.
#[derive(Debug, Clone)]
pub struct AppBundle {
    pub root: PathBuf,
    pub manifest: AppFileManifest,
    pub entities: Vec<EntityDef>,
    pub workflows: Vec<Value>,
    pub permissions: Vec<Value>,
    pub reports: Vec<ReportDef>,
    pub dashboards: Vec<DashboardDef>,
    pub print_formats: Vec<PrintFormat>,
    pub seeds: Vec<SeedBatch>,
    pub hooks: Vec<LifecycleHookDef>,
    pub migrations: Vec<AppMigration>,
    pub assets: Vec<PathBuf>,
}

impl AppBundle {
    pub fn load(root: &Path) -> QefroResult<Self> {
        let toml_path = root.join("app.toml");
        let text = fs::read_to_string(&toml_path).map_err(|e| {
            QefroError::bad_request(format!("cannot read {}: {e}", toml_path.display()))
        })?;
        let manifest = parse_app_toml(&text)?;
        let mut entities = crate::catalog::load_yaml_entities(root)?;
        for entity in &mut entities {
            if entity.module.is_none() {
                entity.module = Some(manifest.name.clone());
            }
            entity.normalize();
        }
        let mut reports: Vec<ReportDef> = load_yaml_docs(&root.join("reports"))?;
        for report in &mut reports {
            if report.module.is_none() {
                report.module = Some(manifest.name.clone());
            }
        }
        let mut dashboards: Vec<DashboardDef> = load_yaml_docs(&root.join("dashboards"))?;
        for dash in &mut dashboards {
            if dash.module.is_none() {
                dash.module = Some(manifest.name.clone());
            }
        }
        let print_formats: Vec<PrintFormat> = load_yaml_docs(&root.join("print_formats"))?;
        let workflows = load_raw_docs(&root.join("workflows"))?;
        let permissions = load_permission_docs(&root.join("permissions"))?;
        let seeds = load_seeds(&root.join("seeds"))?;
        let hooks = load_yaml_docs(&root.join("hooks"))?;
        let migrations = load_migrations(&root.join("migrations"), &manifest.version)?;
        let assets = list_assets(&root.join("assets"))?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
            entities,
            workflows,
            permissions,
            reports,
            dashboards,
            print_formats,
            seeds,
            hooks,
            migrations,
            assets,
        })
    }

    pub fn app_manifest(&self) -> AppManifest {
        self.manifest.clone().into()
    }

    pub fn into_module(self) -> AppModule {
        let mut builder = AppModule::new(&self.manifest.name)
            .version(&self.manifest.version)
            .label(&self.manifest.label)
            .description(&self.manifest.description)
            .author(&self.manifest.author)
            .license(&self.manifest.license)
            .api_version(&self.manifest.api_version)
            .framework_version(&self.manifest.framework_version)
            .source(&self.manifest.source);
        for (name, req) in &self.manifest.dependencies {
            builder = builder.dependency(name, req);
        }
        for item in self.manifest.navigation {
            builder = builder.nav(item);
        }
        for entity in self.entities {
            builder = builder.entity(entity);
        }
        for dash in self.dashboards {
            builder = builder.dashboard(dash);
        }
        for report in self.reports {
            builder = builder.report(report);
        }
        for fmt in self.print_formats {
            builder = builder.print_format(fmt);
        }
        builder.build()
    }
}

fn load_raw_docs(dir: &Path) -> QefroResult<Vec<Value>> {
    load_yaml_docs(dir)
}

fn load_permission_docs(dir: &Path) -> QefroResult<Vec<Value>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for path in crate::catalog::list_data_files(dir)? {
        let text = fs::read_to_string(&path)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", path.display())))?;
        let value: Value = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&text)
                .map_err(|e| QefroError::bad_request(format!("{}: {e}", path.display())))?
        } else {
            serde_yaml::from_str(&text)
                .map_err(|e| QefroError::bad_request(format!("{}: {e}", path.display())))?
        };
        if let Some(arr) = value.as_array() {
            out.extend(arr.iter().cloned());
        } else {
            out.push(value);
        }
    }
    Ok(out)
}

fn load_seeds(dir: &Path) -> QefroResult<Vec<SeedBatch>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for path in crate::catalog::list_data_files(dir)? {
        let text = fs::read_to_string(&path)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", path.display())))?;
        let mut batches = parse_seed_file(&text)
            .map_err(|e| QefroError::bad_request(format!("{}: {e}", path.display())))?;
        out.append(&mut batches);
    }
    Ok(out)
}

fn load_migrations(dir: &Path, app_version: &str) -> QefroResult<Vec<AppMigration>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for path in crate::catalog::list_data_files(dir)? {
        let text = fs::read_to_string(&path)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", path.display())))?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("migration");
        let version = stem
            .split('_')
            .next()
            .filter(|s| crate::version::parse_version(s).is_ok())
            .unwrap_or(app_version);
        out.push(parse_migration_file(&text, stem, version)?);
    }
    out.sort_by(|a, b| a.version.cmp(&b.version).then(a.id.cmp(&b.id)));
    Ok(out)
}

fn list_assets(dir: &Path) -> QefroResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    walk_files(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> QefroResult<()> {
    for entry in fs::read_dir(dir).map_err(|e| QefroError::internal(e.to_string()))? {
        let entry = entry.map_err(|e| QefroError::internal(e.to_string()))?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        if path.is_dir() {
            walk_files(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            crate::package::assert_safe_relative(rel)?;
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate_bundle;

    #[test]
    fn v1_benchmark_apps_load() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["inventory", "helpdesk"] {
            let path = root.join("apps").join(name);
            let bundle = AppBundle::load(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            let report = validate_bundle(&bundle, &[]);
            assert!(report.ok(), "{name}: {}", report.errors.join("; "));
        }
    }
}
