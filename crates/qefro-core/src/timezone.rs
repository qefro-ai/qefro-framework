//! Explicit tenant timezone conversion.
//!
//! API and database values use UTC (RFC3339 / TIMESTAMPTZ). The UI displays
//! tenant-local time. Never assume the browser or server local zone.

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

pub fn parse_tz(name: &str) -> Tz {
    if name.eq_ignore_ascii_case("tenant") || name.eq_ignore_ascii_case("utc") || name.is_empty() {
        return chrono_tz::UTC;
    }
    name.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

/// Convert a stored UTC instant into a tenant-local RFC3339 string (offset included).
pub fn utc_to_local(utc: DateTime<Utc>, tz_name: &str) -> String {
    let tz = parse_tz(tz_name);
    utc.with_timezone(&tz).to_rfc3339()
}

/// Interpret a naive local datetime in the tenant zone and return UTC.
pub fn local_to_utc(local: NaiveDateTime, tz_name: &str) -> DateTime<Utc> {
    let tz = parse_tz(tz_name);
    tz.from_local_datetime(&local)
        .single()
        .or_else(|| tz.from_local_datetime(&local).earliest())
        .unwrap_or_else(|| tz.from_utc_datetime(&local))
        .with_timezone(&Utc)
}

/// Parse a client datetime. RFC3339 is stored as-is (converted to UTC).
/// Naive `YYYY-MM-DDTHH:MM` is interpreted in `tz_name`.
pub fn canonicalize_datetime(raw: &str, tz_name: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    const FMTS: &[&str] = &[
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
    ];
    for fmt in FMTS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(raw, fmt) {
            return Some(local_to_utc(naive, tz_name));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn kolkata_offset_is_explicit() {
        let utc = Utc.with_ymd_and_hms(2026, 8, 15, 14, 30, 0).unwrap();
        let local = utc_to_local(utc, "Asia/Kolkata");
        assert!(local.contains("+05:30"), "{local}");
        assert!(local.contains("20:00"), "{local}");
    }

    #[test]
    fn naive_local_converts_to_utc() {
        let naive = NaiveDateTime::parse_from_str("2026-08-15T20:00", "%Y-%m-%dT%H:%M").unwrap();
        let utc = local_to_utc(naive, "Asia/Kolkata");
        assert_eq!(utc.to_rfc3339(), "2026-08-15T14:30:00+00:00");
    }

    #[test]
    fn rfc3339_is_already_canonical() {
        let dt = canonicalize_datetime("2026-08-15T14:30:00Z", "Asia/Kolkata").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-08-15T14:30:00+00:00");
    }

    #[test]
    fn naive_uses_tenant_zone_not_server_local() {
        let dt = canonicalize_datetime("2026-08-15T20:00", "Asia/Kolkata").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-08-15T14:30:00+00:00");
    }
}
