//! Outbound HTTP policy (SSRF). String checks are not enough on their own:
//! callers that perform DNS must also [`assert_public_ip`] after resolution
//! and must not follow redirects to a new host.
//!
//! `test://` URLs are allowed so webhook tests do not hit the network.

use crate::{QefroError, QefroResult};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "localhost.localdomain",
    "metadata.google.internal",
    "metadata.goog",
    "kubernetes",
    "kubernetes.default",
    "kubernetes.default.svc",
];

/// True when `host` is a name or literal that must not be fetched.
pub fn is_blocked_host(host: &str) -> bool {
    let host = host.trim().trim_matches(|c| c == '[' || c == ']');
    let lower = host.to_ascii_lowercase();
    if BLOCKED_HOSTS.contains(&lower.as_str()) {
        return true;
    }
    if lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".corp")
        || lower.ends_with(".lan")
    {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return !is_public_ip(ip);
    }
    false
}

pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

pub fn assert_public_ip(ip: IpAddr) -> QefroResult<()> {
    if is_public_ip(ip) {
        Ok(())
    } else {
        Err(QefroError::bad_request("outbound URL is not allowed"))
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    if ip.is_unspecified() || ip.is_loopback() || ip.is_broadcast() || ip.is_link_local() {
        return false;
    }
    if ip.is_private() || ip.is_multicast() {
        return false;
    }
    // IETF CGNAT 100.64.0.0/10, benchmark 198.18.0.0/15, docs 192.0.2.0/24 etc.
    if o[0] == 100 && (o[1] & 0xc0) == 64 {
        return false;
    }
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) {
        return false;
    }
    if o[0] == 192 && o[1] == 0 && (o[2] == 0 || o[2] == 2) {
        return false;
    }
    if o[0] == 198 && o[1] == 51 && o[2] == 100 {
        return false;
    }
    if o[0] == 203 && o[1] == 0 && o[2] == 113 {
        return false;
    }
    // Cloud metadata commonly 169.254.169.254 (already link-local) and 10/8.
    true
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
        return false;
    }
    let segs = ip.segments();
    // unique local fc00::/7
    if segs[0] & 0xfe00 == 0xfc00 {
        return false;
    }
    // IPv4-mapped
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    // link-local fe80::/10
    if segs[0] & 0xffc0 == 0xfe80 {
        return false;
    }
    true
}

/// Reject non-http(s) schemes, credentials in the URL, and blocked hosts.
/// Does not perform DNS. Pair with post-resolution [`assert_public_ip`].
pub fn validate_http_url(raw: &str) -> QefroResult<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(QefroError::bad_request("outbound URL is required"));
    }
    if raw.starts_with("test://") {
        return Ok(());
    }
    let lower = raw.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(QefroError::bad_request(
            "outbound URL must be http or https",
        ));
    }
    let rest = raw.split_once("://").map(|(_, r)| r).unwrap_or("");
    if rest.contains('@') {
        return Err(QefroError::bad_request(
            "outbound URL must not include credentials",
        ));
    }
    let hostport = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if hostport.starts_with('[') {
        hostport
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    if host.is_empty() || is_blocked_host(host) {
        return Err(QefroError::bad_request("outbound URL is not allowed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_localhost_and_private() {
        assert!(validate_http_url("http://127.0.0.1/hook").is_err());
        assert!(validate_http_url("http://localhost/hook").is_err());
        assert!(validate_http_url("http://10.0.0.5/hook").is_err());
        assert!(validate_http_url("http://192.168.1.1/hook").is_err());
        assert!(validate_http_url("http://169.254.169.254/latest").is_err());
        assert!(validate_http_url("http://metadata.google.internal/").is_err());
        assert!(validate_http_url("ftp://example.com/").is_err());
        assert!(validate_http_url("https://user:pass@example.com/").is_err());
        assert!(validate_http_url("test://order-ready").is_ok());
        assert!(validate_http_url("https://hooks.example.com/a").is_ok());
    }

    #[test]
    fn cgnat_and_unspecified_are_private() {
        assert!(!is_public_ip("100.64.1.1".parse().unwrap()));
        assert!(!is_public_ip("0.0.0.0".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
    }
}
