//! Five-field cron expressions for scheduled automation.
//!
//! Fallback timezone is UTC when tenant/application config has none.

use crate::error::{QefroError, QefroResult};
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq)]
pub struct CronExpr {
    minutes: Vec<u8>,
    hours: Vec<u8>,
    days: Vec<u8>,
    months: Vec<u8>,
    weekdays: Vec<u8>,
    pub raw: String,
}

pub fn parse_cron(expr: &str) -> QefroResult<CronExpr> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(QefroError::bad_request(
            "schedule must be a 5-field cron expression (min hour day month weekday)",
        ));
    }
    Ok(CronExpr {
        minutes: parse_field(parts[0], 0, 59)?,
        hours: parse_field(parts[1], 0, 23)?,
        days: parse_field(parts[2], 1, 31)?,
        months: parse_field(parts[3], 1, 12)?,
        weekdays: parse_field(parts[4], 0, 6)?,
        raw: expr.trim().to_string(),
    })
}

fn parse_field(raw: &str, min: u8, max: u8) -> QefroResult<Vec<u8>> {
    if raw == "*" {
        return Ok((min..=max).collect());
    }
    let mut out = Vec::new();
    for part in raw.split(',') {
        if let Some((range, step_s)) = part.split_once('/') {
            let step: u8 = step_s
                .parse()
                .map_err(|_| QefroError::bad_request(format!("invalid cron step '{part}'")))?;
            if step == 0 {
                return Err(QefroError::bad_request("cron step cannot be 0"));
            }
            let span = if range == "*" {
                min..=max
            } else if let Some((a, b)) = range.split_once('-') {
                parse_num(a, min, max)?..=parse_num(b, min, max)?
            } else {
                let start = parse_num(range, min, max)?;
                start..=max
            };
            out.extend(span.step_by(step as usize));
        } else if let Some((a, b)) = part.split_once('-') {
            out.extend(parse_num(a, min, max)?..=parse_num(b, min, max)?);
        } else {
            out.push(parse_num(part, min, max)?);
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(QefroError::bad_request(format!("empty cron field '{raw}'")));
    }
    Ok(out)
}

fn parse_num(s: &str, min: u8, max: u8) -> QefroResult<u8> {
    let n: u8 = s
        .parse()
        .map_err(|_| QefroError::bad_request(format!("invalid cron number '{s}'")))?;
    if n < min || n > max {
        return Err(QefroError::bad_request(format!(
            "cron value {n} out of range {min}-{max}"
        )));
    }
    Ok(n)
}

pub fn parse_timezone(name: &str) -> Tz {
    name.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

/// Next fire time strictly after `from` in the given IANA timezone.
pub fn next_run_after(expr: &CronExpr, from: DateTime<Utc>, tz_name: &str) -> DateTime<Utc> {
    let tz = parse_timezone(tz_name);
    let local = from.with_timezone(&tz) + chrono::Duration::minutes(1);
    let mut cursor = tz
        .with_ymd_and_hms(
            local.year(),
            local.month(),
            local.day(),
            local.hour(),
            local.minute(),
            0,
        )
        .single()
        .unwrap_or(local);
    for _ in 0..(366 * 24 * 60) {
        if matches(
            expr,
            cursor.minute() as u8,
            cursor.hour() as u8,
            cursor.day() as u8,
            cursor.month() as u8,
            cursor.weekday().num_days_from_sunday() as u8,
        ) {
            return cursor.with_timezone(&Utc);
        }
        cursor += chrono::Duration::minutes(1);
    }
    from + chrono::Duration::days(1)
}

fn matches(expr: &CronExpr, minute: u8, hour: u8, day: u8, month: u8, weekday: u8) -> bool {
    expr.minutes.contains(&minute)
        && expr.hours.contains(&hour)
        && expr.months.contains(&month)
        && expr.days.contains(&day)
        && expr.weekdays.contains(&weekday)
}

/// Slot identity for idempotent scheduled runs (tenant + automation + fire time).
pub fn schedule_slot_key(run_at: DateTime<Utc>) -> String {
    run_at.format("%Y%m%d%H%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_nine() {
        let expr = parse_cron("0 9 * * *").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 8, 30, 8, 0, 0).unwrap();
        let next = next_run_after(&expr, from, "UTC");
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 8, 30, 9, 0, 0).unwrap());
    }

    #[test]
    fn rejects_junk() {
        assert!(parse_cron("not a cron").is_err());
        assert!(parse_cron("* * *").is_err());
    }
}
