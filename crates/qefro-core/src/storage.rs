use crate::{QefroError, QefroResult};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Tenant-isolated blob storage. Local disk now; S3-compatible later.
pub trait BlobStore: Send + Sync {
    fn put(&self, tenant_id: Uuid, key: &str, bytes: &[u8]) -> QefroResult<String>;
    fn get(&self, tenant_id: Uuid, key: &str) -> QefroResult<Vec<u8>>;
    fn delete(&self, tenant_id: Uuid, key: &str) -> QefroResult<()>;
}

pub struct LocalBlobStore {
    root: PathBuf,
}

impl LocalBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn tenant_path(&self, tenant_id: Uuid, key: &str) -> QefroResult<PathBuf> {
        if key.contains("..") || key.starts_with('/') || Path::new(key).is_absolute() {
            return Err(QefroError::bad_request("invalid storage key"));
        }
        Ok(self.root.join(tenant_id.to_string()).join(key))
    }
}

impl BlobStore for LocalBlobStore {
    fn put(&self, tenant_id: Uuid, key: &str, bytes: &[u8]) -> QefroResult<String> {
        let path = self.tenant_path(tenant_id, key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| QefroError::internal(format!("storage mkdir: {e}")))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| QefroError::internal(format!("storage write: {e}")))?;
        Ok(format!("tenant/{tenant_id}/{key}"))
    }

    fn get(&self, tenant_id: Uuid, key: &str) -> QefroResult<Vec<u8>> {
        let path = self.tenant_path(tenant_id, key)?;
        std::fs::read(&path).map_err(|_| QefroError::not_found("object not found"))
    }

    fn delete(&self, tenant_id: Uuid, key: &str) -> QefroResult<()> {
        let path = self.tenant_path(tenant_id, key)?;
        let _ = std::fs::remove_file(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_tenants_and_rejects_path_escape() {
        let dir = std::env::temp_dir().join(format!("qefro-blob-{}", Uuid::new_v4()));
        let store = LocalBlobStore::new(&dir);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        store.put(a, "logo.png", b"aaa").unwrap();
        store.put(b, "logo.png", b"bbb").unwrap();
        assert_eq!(store.get(a, "logo.png").unwrap(), b"aaa");
        assert_eq!(store.get(b, "logo.png").unwrap(), b"bbb");
        assert!(store.put(a, "../x", b"no").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
