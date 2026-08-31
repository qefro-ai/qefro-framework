//! `.qefro` application packages.
//!
//! A package is a ZIP archive of application definitions plus
//! `qefro-package.json` (manifest, file list, SHA-256). Paths are validated
//! against traversal. This is not a PKI: checksums detect accidental
//! corruption and prepare for future signing.

use crate::error::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const PACKAGE_META: &str = "qefro-package.json";
pub const PACKAGE_FORMAT: u32 = 1;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_FILES: usize = 256;

const PACK_DIRS: &[&str] = &[
    "entities",
    "workflows",
    "permissions",
    "reports",
    "dashboards",
    "pages",
    "print_formats",
    "communications",
    "seeds",
    "hooks",
    "migrations",
    "assets",
    "tools",
];

const PACK_FILES: &[&str] = &["app.toml", "README.md", "runtime.toml"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMeta {
    pub format: u32,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub created_at: String,
    pub files: Vec<String>,
    /// Runtime that built the package.
    #[serde(default)]
    pub framework_version: String,
    #[serde(default)]
    pub metadata_schema: u32,
    #[serde(default)]
    pub ui_schema: String,
}

pub fn assert_safe_relative(path: &Path) -> QefroResult<()> {
    if path.is_absolute() {
        return Err(QefroError::bad_request(format!(
            "absolute path not allowed: {}",
            path.display()
        )));
    }
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name.contains('\0') {
                    return Err(QefroError::bad_request("invalid path"));
                }
            }
            Component::CurDir => {}
            _ => {
                return Err(QefroError::bad_request(format!(
                    "unsafe package path: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

pub fn collect_package_files(root: &Path) -> QefroResult<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for name in PACK_FILES {
        let path = root.join(name);
        if path.is_file() {
            files.push(((*name).to_string(), read_capped(&path)?));
        }
    }
    for dir in PACK_DIRS {
        let dir_path = root.join(dir);
        if dir_path.is_dir() {
            collect_dir(root, &dir_path, &mut files)?;
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files.dedup_by(|a, b| a.0 == b.0);
    if files.len() > MAX_FILES {
        return Err(QefroError::bad_request(format!(
            "package has too many files ({})",
            files.len()
        )));
    }
    let total: u64 = files.iter().map(|(_, b)| b.len() as u64).sum();
    if total > MAX_TOTAL_BYTES {
        return Err(QefroError::bad_request("package exceeds size limit"));
    }
    Ok(files)
}

fn collect_dir(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) -> QefroResult<()> {
    for entry in fs::read_dir(dir).map_err(|e| QefroError::internal(e.to_string()))? {
        let entry = entry.map_err(|e| QefroError::internal(e.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_dir(root, &path, files)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| QefroError::internal("path prefix"))?;
        assert_safe_relative(rel)?;
        let key = rel.to_string_lossy().replace('\\', "/");
        files.push((key, read_capped(&path)?));
    }
    Ok(())
}

fn read_capped(path: &Path) -> QefroResult<Vec<u8>> {
    let meta = fs::metadata(path).map_err(|e| QefroError::internal(e.to_string()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(QefroError::bad_request(format!(
            "file too large: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|e| QefroError::internal(e.to_string()))
}

pub fn content_sha256(files: &[(String, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (name, bytes) in files {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(bytes);
    }
    hex::encode(hasher.finalize())
}

pub fn write_package(
    root: &Path,
    dest: &Path,
    name: &str,
    version: &str,
) -> QefroResult<PackageMeta> {
    let files = collect_package_files(root)?;
    if !files.iter().any(|(n, _)| n == "app.toml") {
        return Err(QefroError::bad_request("package is missing app.toml"));
    }
    let sha256 = content_sha256(&files);
    let meta = PackageMeta {
        format: PACKAGE_FORMAT,
        name: name.to_string(),
        version: version.to_string(),
        sha256: sha256.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        files: files.iter().map(|(n, _)| n.clone()).collect(),
        framework_version: crate::version::FRAMEWORK_VERSION.into(),
        metadata_schema: crate::version::METADATA_SCHEMA_VERSION,
        ui_schema: crate::ui::UI_SCHEMA_VERSION.into(),
    };
    let meta_json =
        serde_json::to_vec_pretty(&meta).map_err(|e| QefroError::internal(e.to_string()))?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| QefroError::internal(e.to_string()))?;
    }
    let file = File::create(dest).map_err(|e| QefroError::internal(e.to_string()))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(
            zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0).unwrap_or_else(|_| {
                zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap()
            }),
        );
    zip.start_file(PACKAGE_META, options)
        .map_err(|e| QefroError::internal(e.to_string()))?;
    zip.write_all(&meta_json)
        .map_err(|e| QefroError::internal(e.to_string()))?;
    for (name, bytes) in &files {
        zip.start_file(name, options)
            .map_err(|e| QefroError::internal(e.to_string()))?;
        zip.write_all(bytes)
            .map_err(|e| QefroError::internal(e.to_string()))?;
    }
    zip.finish()
        .map_err(|e| QefroError::internal(e.to_string()))?;
    Ok(meta)
}

pub fn inspect_package(path: &Path) -> QefroResult<(PackageMeta, Vec<(String, Vec<u8>)>)> {
    let file = File::open(path)
        .map_err(|e| QefroError::bad_request(format!("cannot open {}: {e}", path.display())))?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| QefroError::bad_request(format!("invalid .qefro package: {e}")))?;
    if zip.len() > MAX_FILES + 1 {
        return Err(QefroError::bad_request("package has too many files"));
    }
    let mut seen = HashSetLite::new();
    let mut files = Vec::new();
    let mut meta: Option<PackageMeta> = None;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| QefroError::bad_request(format!("invalid package entry: {e}")))?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        let rel = Path::new(&name);
        assert_safe_relative(rel)?;
        if !seen.insert(name.clone()) {
            return Err(QefroError::bad_request(format!(
                "duplicate package file '{name}'"
            )));
        }
        if entry.size() > MAX_FILE_BYTES {
            return Err(QefroError::bad_request(format!("file too large: {name}")));
        }
        let compressed = entry.compressed_size();
        if compressed > 0
            && entry.size() > compressed.saturating_mul(100)
            && entry.size() > 1_000_000
        {
            return Err(QefroError::bad_request(format!(
                "refusing zip-bomb entry: {name}"
            )));
        }
        if name != PACKAGE_META && !allowed_package_member(&name) {
            return Err(QefroError::bad_request(format!(
                "unexpected package file '{name}'"
            )));
        }
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| QefroError::bad_request(format!("cannot read {name}: {e}")))?;
        total += buf.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(QefroError::bad_request("package exceeds size limit"));
        }
        if name == PACKAGE_META {
            meta =
                Some(serde_json::from_slice(&buf).map_err(|e| {
                    QefroError::bad_request(format!("invalid {PACKAGE_META}: {e}"))
                })?);
        } else {
            files.push((name, buf));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let meta =
        meta.ok_or_else(|| QefroError::bad_request("package is missing qefro-package.json"))?;
    if meta.format != PACKAGE_FORMAT {
        return Err(QefroError::bad_request(format!(
            "unsupported package format {}",
            meta.format
        )));
    }
    let actual = content_sha256(&files);
    if actual != meta.sha256 {
        return Err(QefroError::bad_request(
            "package checksum mismatch (archive may be corrupted or tampered with)",
        ));
    }
    Ok((meta, files))
}

pub fn extract_package(path: &Path, dest: &Path) -> QefroResult<PackageMeta> {
    let (meta, files) = inspect_package(path)?;
    if dest.exists() {
        fs::create_dir_all(dest).map_err(|e| QefroError::internal(e.to_string()))?;
    } else {
        fs::create_dir_all(dest).map_err(|e| QefroError::internal(e.to_string()))?;
    }
    let dest_canon = fs::canonicalize(dest).map_err(|e| QefroError::internal(e.to_string()))?;
    for (name, bytes) in files {
        let rel = Path::new(&name);
        assert_safe_relative(rel)?;
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| QefroError::internal(e.to_string()))?;
        }
        let parent_canon = target
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .unwrap_or_else(|| dest_canon.clone());
        if !parent_canon.starts_with(&dest_canon) {
            return Err(QefroError::bad_request(format!(
                "refusing to write outside install directory: {name}"
            )));
        }
        fs::write(&target, bytes).map_err(|e| QefroError::internal(e.to_string()))?;
    }
    Ok(meta)
}

fn allowed_package_member(name: &str) -> bool {
    if PACK_FILES.iter().any(|f| *f == name) {
        return true;
    }
    PACK_DIRS
        .iter()
        .any(|d| name == *d || name.starts_with(&format!("{d}/")))
}

struct HashSetLite {
    inner: std::collections::HashSet<String>,
}

impl HashSetLite {
    fn new() -> Self {
        Self {
            inner: std::collections::HashSet::new(),
        }
    }
    fn insert(&mut self, v: String) -> bool {
        self.inner.insert(v)
    }
}

pub fn default_package_name(name: &str, version: &str) -> PathBuf {
    PathBuf::from(format!("{name}-{version}.qefro"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qefro-pkg-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_unexpected_package_path() {
        assert!(!allowed_package_member("../../etc/passwd"));
        assert!(!allowed_package_member("src/main.rs"));
        assert!(allowed_package_member("entities/customer.yaml"));
        assert!(allowed_package_member("app.toml"));
    }

    #[test]
    fn roundtrip_package() {
        let root = temp_dir("src");
        fs::write(
            root.join("app.toml"),
            "name = \"myshop\"\nversion = \"1.0.0\"\nlabel = \"My Shop\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("entities")).unwrap();
        fs::write(
            root.join("entities/customer.yaml"),
            "name: Customer\nfields:\n  - name: name\n    type: string\n",
        )
        .unwrap();
        let dest = temp_dir("out").join("myshop-1.0.0.qefro");
        let meta = write_package(&root, &dest, "myshop", "1.0.0").unwrap();
        assert_eq!(meta.name, "myshop");
        let extract = temp_dir("extract");
        extract_package(&dest, &extract).unwrap();
        assert!(extract.join("app.toml").exists());
        assert!(extract.join("entities/customer.yaml").exists());
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(dest.parent().unwrap()).ok();
        fs::remove_dir_all(extract).ok();
    }

    #[test]
    fn rejects_zip_with_parent_entry() {
        let dir = temp_dir("evil");
        let zip_path = dir.join("evil.qefro");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("../../outside-file", options).unwrap();
        zip.write_all(b"nope").unwrap();
        zip.finish().unwrap();
        let err = inspect_package(&zip_path).unwrap_err();
        assert!(err.to_string().contains("unsafe") || err.to_string().contains("qefro-package"));
        fs::remove_dir_all(dir).ok();
    }
}
