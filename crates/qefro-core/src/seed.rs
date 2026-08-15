use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Seed records shipped with an application package.
///
/// Kinds:
/// - `system` — non-tenant platform rows (skipped for tenant-owned entities)
/// - `install` — run when a tenant first enables the app
/// - `tenant` — explicit `qefro app seed --tenant`
/// - `development` — only when `QEFRO_ENV=development`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedBatch {
    #[serde(default = "default_kind")]
    pub kind: String,
    pub entity: String,
    #[serde(default)]
    pub unique_by: Vec<String>,
    #[serde(default)]
    pub records: Vec<Value>,
}

fn default_kind() -> String {
    "tenant".into()
}

impl SeedBatch {
    pub fn kind_ok(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "system" | "install" | "tenant" | "development"
        )
    }

    pub fn unique_key(&self, record: &Value) -> Option<String> {
        if self.unique_by.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for field in &self.unique_by {
            let v = record.get(field)?;
            parts.push(v.to_string());
        }
        Some(parts.join("\0"))
    }
}

pub fn parse_seed_file(text: &str) -> Result<Vec<SeedBatch>, String> {
    let value: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    if value.is_array() {
        serde_json::from_value(value).map_err(|e| e.to_string())
    } else if value.get("records").is_some() || value.get("entity").is_some() {
        let batch: SeedBatch = serde_json::from_value(value).map_err(|e| e.to_string())?;
        Ok(vec![batch])
    } else if let Some(batches) = value.get("batches") {
        serde_json::from_value(batches.clone()).map_err(|e| e.to_string())
    } else {
        Err("seed file must be a batch, an array of batches, or { batches: [...] }".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_batch() {
        let batches = parse_seed_file(
            r#"
kind: tenant
entity: Product
unique_by: [sku]
records:
  - name: Coffee
    sku: COF-1
"#,
        )
        .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].entity, "Product");
        assert_eq!(batches[0].records.len(), 1);
    }
}
