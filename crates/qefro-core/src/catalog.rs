//! Filesystem application catalog.
//!
//! Built-in apps (restaurant, crm) are still registered from Rust crates.
//! YAML apps under `apps/<name>/` and installed packages under `.qefro/store/`
//! can be discovered without modifying framework core.

use crate::app::{AppManifest, NavItem};
use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const INSTALLED_FILE: &str = ".qefro/installed.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppFileManifest {
    pub name: String,
    #[serde(default = "default_version")]
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
    #[serde(default = "default_framework_req")]
    pub framework_version: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub navigation: Vec<NavItem>,
    /// Legacy list of names. Merged into `dependencies` on parse.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_version() -> String {
    "0.1.0".into()
}
fn default_api_version() -> String {
    crate::version::APP_API_VERSION.to_string()
}
fn default_framework_req() -> String {
    crate::version::FRAMEWORK_COMPAT_REQ.into()
}
fn default_source() -> String {
    "catalog".into()
}

impl AppFileManifest {
    pub fn normalize(&mut self) {
        if self.label.is_empty() {
            self.label = self.name.clone();
        }
        if self.source.is_empty() {
            self.source = default_source();
        }
        for name in &self.depends_on {
            if crate::version::is_framework_dep(name) {
                if self.framework_version.is_empty() {
                    self.framework_version = default_framework_req();
                }
                continue;
            }
            self.dependencies
                .entry(name.clone())
                .or_insert_with(|| "*".into());
        }
    }
}

impl From<AppFileManifest> for AppManifest {
    fn from(mut value: AppFileManifest) -> Self {
        value.normalize();
        AppManifest {
            name: value.name,
            version: value.version,
            label: value.label,
            description: value.description,
            author: value.author,
            license: value.license,
            api_version: value.api_version,
            framework_version: value.framework_version,
            source: value.source,
            dependencies: value.dependencies,
            navigation: value.navigation,
            depends_on: value.depends_on,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledSet {
    #[serde(default)]
    pub installed: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub records: BTreeMap<String, InstalledRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub version: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_status_installed")]
    pub status: String,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

fn default_status_installed() -> String {
    "installed".into()
}

impl InstalledSet {
    pub fn is_installed(&self, name: &str) -> bool {
        self.installed.iter().any(|n| n == name)
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled.iter().any(|n| n == name)
            || self
                .records
                .get(name)
                .map(|r| r.status == "disabled")
                .unwrap_or(false)
    }

    pub fn active(&self) -> Vec<String> {
        self.installed
            .iter()
            .filter(|n| !self.is_disabled(n))
            .cloned()
            .collect()
    }

    pub fn refs(&self) -> Vec<crate::validate::InstalledAppRef> {
        self.installed
            .iter()
            .map(|name| crate::validate::InstalledAppRef {
                name: name.clone(),
                version: self
                    .records
                    .get(name)
                    .map(|r| r.version.clone())
                    .unwrap_or_else(|| "0.0.0".into()),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    pub manifest: AppManifest,
    pub root: PathBuf,
    pub builtin: bool,
}

pub fn apps_dir() -> PathBuf {
    PathBuf::from("apps")
}

pub fn store_dir() -> PathBuf {
    PathBuf::from(".qefro/store")
}

pub fn installed_path() -> PathBuf {
    PathBuf::from(INSTALLED_FILE)
}

pub fn load_installed() -> InstalledSet {
    let path = installed_path();
    if !path.exists() {
        return InstalledSet::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_installed(set: &InstalledSet) -> QefroResult<()> {
    if let Some(parent) = installed_path().parent() {
        fs::create_dir_all(parent).map_err(|e| QefroError::internal(e.to_string()))?;
    }
    let json =
        serde_json::to_string_pretty(set).map_err(|e| QefroError::internal(e.to_string()))?;
    fs::write(installed_path(), json).map_err(|e| QefroError::internal(e.to_string()))?;
    Ok(())
}

pub fn discover_apps(builtins: &[AppManifest]) -> Vec<DiscoveredApp> {
    let mut out = Vec::new();
    for m in builtins {
        out.push(DiscoveredApp {
            manifest: m.clone(),
            root: apps_dir().join(&m.name),
            builtin: true,
        });
    }
    for dir in [apps_dir(), store_dir()] {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let toml_path = path.join("app.toml");
                if !toml_path.exists() {
                    continue;
                }
                if let Ok(text) = fs::read_to_string(&toml_path) {
                    if let Ok(file) = parse_app_toml(&text) {
                        if out.iter().any(|a| a.manifest.name == file.name) {
                            continue;
                        }
                        let builtin = file.source == "builtin";
                        out.push(DiscoveredApp {
                            manifest: file.into(),
                            root: path,
                            builtin,
                        });
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    out
}

pub fn parse_app_toml(text: &str) -> QefroResult<AppFileManifest> {
    let mut manifest: AppFileManifest = toml::from_str(text)
        .map_err(|e| QefroError::bad_request(format!("invalid app.toml: {e}")))?;
    manifest.normalize();
    if manifest.name.is_empty() {
        return Err(QefroError::bad_request("app.toml is missing name"));
    }
    Ok(manifest)
}

pub fn find_app_root(name: &str) -> Option<PathBuf> {
    if Path::new("app.toml").exists() {
        if let Ok(text) = fs::read_to_string("app.toml") {
            if let Ok(m) = parse_app_toml(&text) {
                if m.name == name {
                    return Some(PathBuf::from("."));
                }
            }
        }
    }
    app_root_candidates(name)
        .into_iter()
        .find(|p| p.join("app.toml").exists())
}

pub fn app_root_candidates(name: &str) -> Vec<PathBuf> {
    vec![
        apps_dir().join(name),
        PathBuf::from(name),
        store_dir().join(name),
    ]
}

pub fn load_yaml_entities(root: &Path) -> QefroResult<Vec<EntityDef>> {
    load_yaml_docs(&root.join("entities"))
}

pub fn list_data_files(dir: &Path) -> QefroResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return Ok(paths);
    }
    for entry in fs::read_dir(dir).map_err(|e| QefroError::internal(e.to_string()))? {
        let path = entry
            .map_err(|e| QefroError::internal(e.to_string()))?
            .path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "yaml" | "yml" | "json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn load_yaml_docs<T: DeserializeOwned>(dir: &Path) -> QefroResult<Vec<T>> {
    let mut defs = Vec::new();
    for path in list_data_files(dir)? {
        let text = fs::read_to_string(&path)
            .map_err(|e| QefroError::internal(format!("read {}: {e}", path.display())))?;
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let value = if ext == "json" {
            serde_json::from_str(&text)
                .map_err(|e| QefroError::bad_request(format!("{}: {e}", path.display())))?
        } else {
            serde_yaml::from_str(&text)
                .map_err(|e| QefroError::bad_request(format!("{}: {e}", path.display())))?
        };
        defs.push(value);
    }
    Ok(defs)
}

pub fn install_app(name: &str) -> QefroResult<InstalledSet> {
    mark_installed(name, "0.0.0", "catalog", None)
}

pub fn mark_installed(
    name: &str,
    version: &str,
    source: &str,
    sha256: Option<String>,
) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    if !set.installed.iter().any(|n| n == name) {
        set.installed.push(name.to_string());
        set.installed.sort();
    }
    set.disabled.retain(|n| n != name);
    set.records.insert(
        name.to_string(),
        InstalledRecord {
            version: version.to_string(),
            source: source.to_string(),
            status: "installed".into(),
            installed_at: Some(chrono::Utc::now().to_rfc3339()),
            sha256,
        },
    );
    save_installed(&set)?;
    Ok(set)
}

pub fn disable_app(name: &str) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    if !set.installed.iter().any(|n| n == name) {
        return Err(QefroError::not_found(format!(
            "app '{name}' is not installed"
        )));
    }
    if !set.disabled.iter().any(|n| n == name) {
        set.disabled.push(name.to_string());
        set.disabled.sort();
    }
    if let Some(rec) = set.records.get_mut(name) {
        rec.status = "disabled".into();
    }
    save_installed(&set)?;
    Ok(set)
}

pub fn enable_app(name: &str) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    if !set.installed.iter().any(|n| n == name) {
        return Err(QefroError::not_found(format!(
            "app '{name}' is not installed"
        )));
    }
    set.disabled.retain(|n| n != name);
    if let Some(rec) = set.records.get_mut(name) {
        rec.status = "installed".into();
    }
    save_installed(&set)?;
    Ok(set)
}

pub fn remove_app(name: &str) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    set.installed.retain(|n| n != name);
    set.disabled.retain(|n| n != name);
    set.records.remove(name);
    save_installed(&set)?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_app_toml() {
        let m = parse_app_toml(
            r#"
name = "restaurant"
version = "0.2.0"
label = "Restaurant"
description = "Tables and orders"
depends_on = ["qefro-framework"]
"#,
        )
        .unwrap();
        assert_eq!(m.name, "restaurant");
        assert!(m.depends_on.contains(&"qefro-framework".into()));
    }

    #[test]
    fn parses_dependencies_table() {
        let m = parse_app_toml(
            r#"
name = "restaurant"
version = "1.0.0"
label = "Restaurant"
framework_version = ">=0.7"

[dependencies]
inventory = ">=1.0,<2.0"

[[navigation]]
label = "Orders"
entity = "Order"
"#,
        )
        .unwrap();
        assert_eq!(m.dependencies.get("inventory").unwrap(), ">=1.0,<2.0");
        assert_eq!(m.navigation[0].entity, "Order");
    }
}
