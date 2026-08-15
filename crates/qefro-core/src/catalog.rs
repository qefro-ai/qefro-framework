//! Filesystem application catalog.
//!
//! Built-in apps (restaurant, crm) are still registered from Rust crates.
//! YAML apps under `apps/<name>/` can be discovered and installed without
//! modifying framework core.

use crate::app::AppManifest;
use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};
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
    pub depends_on: Vec<String>,
}

fn default_version() -> String {
    "0.1.0".into()
}

impl From<AppFileManifest> for AppManifest {
    fn from(value: AppFileManifest) -> Self {
        AppManifest {
            name: value.name,
            version: value.version,
            label: value.label,
            description: value.description,
            depends_on: value.depends_on,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledSet {
    #[serde(default)]
    pub installed: Vec<String>,
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
    let json = serde_json::to_string_pretty(set)
        .map_err(|e| QefroError::internal(e.to_string()))?;
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
    if let Ok(entries) = fs::read_dir(apps_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            let toml_path = path.join("app.toml");
            if !toml_path.exists() {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&toml_path) {
                if let Ok(file) = toml_from_manifest(&text) {
                    if out.iter().any(|a| a.manifest.name == file.name) {
                        continue;
                    }
                    let manifest: AppManifest = file.into();
                    out.push(DiscoveredApp {
                        manifest,
                        root: path,
                        builtin: false,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    out
}

pub fn parse_app_toml(text: &str) -> QefroResult<AppFileManifest> {
    toml_from_manifest(text).map_err(|_| QefroError::bad_request("invalid app.toml"))
}

fn toml_from_manifest(text: &str) -> Result<AppFileManifest, ()> {
    // Minimal TOML: name/version/label/description/depends_on without a toml crate.
    let mut name = String::new();
    let mut version = "0.1.0".into();
    let mut label = String::new();
    let mut description = String::new();
    let mut depends_on = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[dependencies]" || line.starts_with("depends_on") && line.contains('[') {
            in_deps = true;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').to_string();
            match k {
                "name" => name = v,
                "version" => version = v,
                "label" => label = v,
                "description" => description = v,
                "depends_on" => {
                    depends_on.extend(parse_string_list(&v));
                }
                _ => {}
            }
        } else if in_deps && line.starts_with('"') {
            depends_on.push(line.trim_matches(|c| c == '"' || c == ',' || c == ' ').to_string());
        }
    }
    if name.is_empty() {
        return Err(());
    }
    if label.is_empty() {
        label = name.clone();
    }
    Ok(AppFileManifest {
        name,
        version,
        label,
        description,
        depends_on,
    })
}

fn parse_string_list(raw: &str) -> Vec<String> {
    raw.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn load_yaml_entities(root: &Path) -> QefroResult<Vec<EntityDef>> {
    let dir = root.join("entities");
    let mut defs = Vec::new();
    if !dir.exists() {
        return Ok(defs);
    }
    for entry in fs::read_dir(&dir).map_err(|e| QefroError::internal(e.to_string()))? {
        let path = entry.map_err(|e| QefroError::internal(e.to_string()))?.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "yaml" | "yml" | "json") {
            defs.push(EntityDef::from_file(&path)?);
        }
    }
    Ok(defs)
}

pub fn install_app(name: &str) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    if !set.installed.iter().any(|n| n == name) {
        set.installed.push(name.to_string());
        set.installed.sort();
    }
    save_installed(&set)?;
    Ok(set)
}

pub fn remove_app(name: &str) -> QefroResult<InstalledSet> {
    let mut set = load_installed();
    set.installed.retain(|n| n != name);
    save_installed(&set)?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_app_toml() {
        let m = toml_from_manifest(
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
        assert_eq!(m.depends_on, vec!["qefro-framework"]);
    }
}
