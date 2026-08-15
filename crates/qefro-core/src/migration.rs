use crate::error::{QefroError, QefroResult};
use serde::{Deserialize, Serialize};

/// Explicit application schema/version migration.
///
/// Additive entity fields still land through metadata `apply_schema`.
/// These files record the version step and may include extra SQL. Destructive
/// SQL is never applied unless `destructive = true` and the operator confirms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMigration {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub destructive: bool,
    #[serde(default)]
    pub sql: String,
}

impl AppMigration {
    pub fn looks_destructive(&self) -> bool {
        self.destructive || sql_is_destructive(&self.sql)
    }
}

pub fn sql_is_destructive(sql: &str) -> bool {
    let upper = sql.to_ascii_uppercase();
    let compact = upper.replace('\n', " ");
    compact.contains("DROP TABLE")
        || compact.contains("DROP COLUMN")
        || compact.contains("DROP INDEX")
        || compact.contains("TRUNCATE")
        || compact.contains("DELETE FROM")
        || compact.contains("ALTER TABLE") && compact.contains(" DROP ")
}

pub fn parse_migration_file(text: &str, fallback_id: &str, fallback_version: &str) -> QefroResult<AppMigration> {
    let mut m: AppMigration = serde_yaml::from_str(text)
        .map_err(|e| QefroError::bad_request(format!("invalid migration yaml: {e}")))?;
    if m.id.trim().is_empty() {
        m.id = fallback_id.to_string();
    }
    if m.version.trim().is_empty() {
        m.version = fallback_version.to_string();
    }
    if sql_is_destructive(&m.sql) {
        m.destructive = true;
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_drop_column() {
        assert!(sql_is_destructive("ALTER TABLE orders DROP COLUMN notes"));
        assert!(!sql_is_destructive("ALTER TABLE orders ADD COLUMN source TEXT"));
    }
}
