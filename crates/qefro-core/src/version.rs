//! Semantic versions and compatibility requirements for apps and the framework.

use crate::error::{QefroError, QefroResult};
use semver::{Version, VersionReq};

/// Crate / runtime version. Apps declare `framework_version` against this.
pub const FRAMEWORK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// App package API. Bump when the `.qefro` / `app.toml` shape is incompatible.
pub const APP_API_VERSION: u32 = 1;

pub fn parse_version(raw: &str) -> QefroResult<Version> {
    let trimmed = raw.trim().trim_start_matches('v');
    Version::parse(trimmed).map_err(|_| {
        QefroError::bad_request(format!("invalid version '{raw}' (expected semver, e.g. 1.2.0)"))
    })
}

pub fn parse_req(raw: &str) -> QefroResult<VersionReq> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(VersionReq::STAR);
    }
    VersionReq::parse(trimmed).map_err(|_| {
        QefroError::bad_request(format!(
            "invalid version requirement '{raw}' (examples: >=0.7, >=1.0,<2.0)"
        ))
    })
}

pub fn matches_req(version: &str, req: &str) -> QefroResult<bool> {
    let v = parse_version(version)?;
    let r = parse_req(req)?;
    Ok(r.matches(&v))
}

pub fn compatible_with_framework(req: &str) -> QefroResult<()> {
    if req.trim().is_empty() {
        return Ok(());
    }
    if matches_req(FRAMEWORK_VERSION, req)? {
        return Ok(());
    }
    Err(QefroError::bad_request(format!(
        "app requires framework {req}; this runtime is {FRAMEWORK_VERSION}"
    )))
}

/// True when `next` is greater than or equal to `current`.
pub fn is_upgrade(current: &str, next: &str) -> QefroResult<bool> {
    Ok(parse_version(next)? >= parse_version(current)?)
}

pub fn is_framework_dep(name: &str) -> bool {
    matches!(name, "core" | "qefro-framework" | "qefro" | "framework")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_matches_caret_and_range() {
        assert!(matches_req("0.7.0", ">=0.7").unwrap());
        assert!(matches_req("0.7.1", ">=0.7,<0.9").unwrap());
        assert!(!matches_req("0.6.0", ">=0.7").unwrap());
        assert!(is_framework_dep("core"));
        assert!(!is_framework_dep("inventory"));
    }

    #[test]
    fn rejects_garbage_version() {
        assert!(parse_version("latest").is_err());
        assert!(parse_req("maybe-one").is_err());
    }
}
