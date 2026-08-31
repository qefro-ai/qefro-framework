//! Generic scheduling metadata: windows, availability, and conflict rules.
//!
//! Execution (locks, queries, reminders) lives in `EntityService`. This module
//! is the declarative capability an EntityDef opts into.

use crate::entity::EntityDef;
use crate::error::{QefroError, QefroResult};
use crate::field::FieldType;
use crate::registry::EntityRegistry;
use crate::timezone::{canonicalize_datetime, local_to_utc};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Opt-in scheduling on an existing entity. Not a second calendar model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulingConfig {
    pub start_field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_field: Option<String>,
    /// Time-of-day field when `start_field` is a date (e.g. `reservation_time`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_day_field: Option<String>,
    /// Many-to-one relation fields (table, room, doctor, vehicle, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
    #[serde(default)]
    pub conflict: bool,
    #[serde(default)]
    pub calendar: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_before_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_after_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_interval_minutes: Option<u32>,
    /// Booking-side capacity (e.g. `party_size`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_field: Option<String>,
    /// Resource-side capacity (e.g. `seats`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_capacity_field: Option<String>,
    /// Workflow states excluded from conflict and availability (Cancelled, Completed).
    #[serde(
        default = "default_ignore_states",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub ignore_states: Vec<String>,
    /// Recurring working hours. Multiple rows on the same weekday represent breaks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub working_hours: Vec<WorkingHours>,
    /// Unavailable dates (`YYYY-MM-DD`). Applications configure these; no holiday DB.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blackouts: Vec<String>,
    /// Minutes before start to enqueue a reminder job. Emits `{entity}.reminder`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder_minutes: Option<i64>,
}

/// One working interval. `weekday` is ISO: 1 = Monday … 7 = Sunday.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingHours {
    pub weekday: u8,
    pub start: String,
    pub end: String,
}

impl WorkingHours {
    pub fn new(weekday: u8, start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            weekday,
            start: start.into(),
            end: end.into(),
        }
    }

    pub fn everyday(start: &str, end: &str) -> Vec<Self> {
        (1..=7).map(|d| Self::new(d, start, end)).collect()
    }
}

impl SchedulingConfig {
    pub fn new(start_field: impl Into<String>) -> Self {
        Self {
            start_field: start_field.into(),
            end_field: None,
            time_field: None,
            end_time_field: None,
            all_day_field: None,
            resources: Vec::new(),
            conflict: false,
            calendar: false,
            duration_minutes: None,
            buffer_before_minutes: None,
            buffer_after_minutes: None,
            slot_interval_minutes: None,
            capacity_field: None,
            resource_capacity_field: None,
            ignore_states: default_ignore_states(),
            working_hours: Vec::new(),
            blackouts: Vec::new(),
            reminder_minutes: None,
        }
    }

    pub fn end_field(mut self, name: impl Into<String>) -> Self {
        self.end_field = Some(name.into());
        self
    }

    pub fn time_field(mut self, name: impl Into<String>) -> Self {
        self.time_field = Some(name.into());
        self
    }

    pub fn end_time_field(mut self, name: impl Into<String>) -> Self {
        self.end_time_field = Some(name.into());
        self
    }

    pub fn all_day_field(mut self, name: impl Into<String>) -> Self {
        self.all_day_field = Some(name.into());
        self
    }

    pub fn resource(mut self, field: impl Into<String>) -> Self {
        self.resources.push(field.into());
        self
    }

    pub fn resources(mut self, fields: &[&str]) -> Self {
        self.resources = fields.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn conflict(mut self) -> Self {
        self.conflict = true;
        self
    }

    pub fn calendar(mut self) -> Self {
        self.calendar = true;
        self
    }

    pub fn duration_minutes(mut self, minutes: u32) -> Self {
        self.duration_minutes = Some(minutes);
        self
    }

    pub fn buffer(mut self, before: u32, after: u32) -> Self {
        self.buffer_before_minutes = Some(before);
        self.buffer_after_minutes = Some(after);
        self
    }

    pub fn slot_interval_minutes(mut self, minutes: u32) -> Self {
        self.slot_interval_minutes = Some(minutes);
        self
    }

    pub fn capacity(
        mut self,
        booking_field: impl Into<String>,
        resource_field: impl Into<String>,
    ) -> Self {
        self.capacity_field = Some(booking_field.into());
        self.resource_capacity_field = Some(resource_field.into());
        self
    }

    pub fn ignore_states(mut self, states: &[&str]) -> Self {
        self.ignore_states = states.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn working_hours(mut self, hours: Vec<WorkingHours>) -> Self {
        self.working_hours = hours;
        self
    }

    pub fn blackouts(mut self, dates: &[&str]) -> Self {
        self.blackouts = dates.iter().map(|s| (*s).to_string()).collect();
        self
    }

    pub fn reminder_minutes(mut self, minutes: i64) -> Self {
        self.reminder_minutes = Some(minutes);
        self
    }

    pub fn duration(&self) -> Duration {
        Duration::minutes(i64::from(self.duration_minutes.unwrap_or(60)))
    }

    pub fn slot_interval(&self) -> Duration {
        Duration::minutes(i64::from(self.slot_interval_minutes.unwrap_or(30)))
    }

    pub fn ignores_status(&self, status: &str) -> bool {
        if status.is_empty() {
            return false;
        }
        self.ignore_states
            .iter()
            .any(|s| s.eq_ignore_ascii_case(status))
    }
}

fn default_ignore_states() -> Vec<String> {
    vec!["Cancelled".into(), "Completed".into()]
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self::new("starts_at")
    }
}

/// Presentation summary for GET /meta/ui. Not authorization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SchedulingSummary {
    pub start: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_day: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
    #[serde(default)]
    pub conflict: bool,
    #[serde(default)]
    pub calendar: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_interval_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_start_hour: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_end_hour: Option<u32>,
}

impl SchedulingConfig {
    pub fn to_summary(&self) -> SchedulingSummary {
        let (day_start_hour, day_end_hour) = working_hour_bounds(&self.working_hours);
        SchedulingSummary {
            start: self.start_field.clone(),
            end: self.end_field.clone(),
            time: self.time_field.clone(),
            end_time: self.end_time_field.clone(),
            all_day: self.all_day_field.clone(),
            resources: self.resources.clone(),
            conflict: self.conflict,
            calendar: self.calendar,
            duration_minutes: self.duration_minutes,
            slot_interval_minutes: self.slot_interval_minutes,
            day_start_hour,
            day_end_hour,
        }
    }
}

fn working_hour_bounds(hours: &[WorkingHours]) -> (Option<u32>, Option<u32>) {
    if hours.is_empty() {
        return (None, None);
    }
    let mut min_h = 23u32;
    let mut max_h = 0u32;
    for h in hours {
        if let Some(t) = parse_clock(&h.start) {
            min_h = min_h.min(t.hour());
        }
        if let Some(t) = parse_clock(&h.end) {
            let hour = if t.minute() > 0 || t.second() > 0 {
                t.hour().saturating_add(1).min(23)
            } else {
                t.hour()
            };
            max_h = max_h.max(hour);
        }
    }
    if min_h > max_h {
        (None, None)
    } else {
        (Some(min_h), Some(max_h))
    }
}

/// Half-open interval in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
}

impl TimeWindow {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> QefroResult<Self> {
        if end <= start {
            return Err(QefroError::business(
                "scheduling_invalid_range",
                "End must be after start.",
            ));
        }
        Ok(Self {
            start,
            end,
            all_day: false,
        })
    }

    pub fn with_buffer(self, before: u32, after: u32) -> Self {
        Self {
            start: self.start - Duration::minutes(i64::from(before)),
            end: self.end + Duration::minutes(i64::from(after)),
            all_day: self.all_day,
        }
    }

    pub fn overlaps(self, other: TimeWindow) -> bool {
        intervals_overlap(self.start, self.end, other.start, other.end)
    }

    pub fn local_date(&self, tz_name: &str) -> NaiveDate {
        let tz = crate::timezone::parse_tz(tz_name);
        self.start.with_timezone(&tz).date_naive()
    }
}

/// Half-open: `[a_start, a_end)` overlaps `[b_start, b_end)` iff starts collide.
pub fn intervals_overlap<T: Ord>(a_start: T, a_end: T, b_start: T, b_end: T) -> bool {
    a_start < b_end && b_start < a_end
}

pub fn parse_clock(raw: &str) -> Option<NaiveTime> {
    let s = raw.trim();
    const FMTS: &[&str] = &["%H:%M:%S", "%H:%M", "%H:%M:%S%.f"];
    for fmt in FMTS {
        if let Ok(t) = NaiveTime::parse_from_str(s, fmt) {
            return Some(t);
        }
    }
    None
}

pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()
}

fn field_str<'a>(record: &'a Value, name: &str) -> Option<&'a str> {
    record
        .get(name)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn field_bool(record: &Value, name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };
    match record.get(name) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true") || s == "1",
        _ => false,
    }
}

/// Resolve start/end from a record using the scheduling field map and tenant zone.
pub fn parse_window(
    config: &SchedulingConfig,
    record: &Value,
    tz_name: &str,
) -> QefroResult<TimeWindow> {
    let all_day = field_bool(record, config.all_day_field.as_deref());
    let start_raw = field_str(record, &config.start_field).ok_or_else(|| {
        QefroError::business(
            "scheduling_missing_start",
            format!("Scheduling requires '{}'.", config.start_field),
        )
    })?;

    if start_raw.contains('T') || start_raw.ends_with('Z') {
        let start = canonicalize_datetime(start_raw, tz_name).ok_or_else(|| {
            QefroError::business("scheduling_invalid_start", "Start datetime is invalid.")
        })?;
        let end = if let Some(end_name) = &config.end_field {
            if let Some(end_raw) = field_str(record, end_name) {
                canonicalize_datetime(end_raw, tz_name).ok_or_else(|| {
                    QefroError::business("scheduling_invalid_end", "End datetime is invalid.")
                })?
            } else {
                start + config.duration()
            }
        } else {
            start + config.duration()
        };
        let mut window = TimeWindow::new(start, end)?;
        window.all_day = all_day;
        return Ok(window);
    }

    let date = parse_date(start_raw).ok_or_else(|| {
        QefroError::business("scheduling_invalid_start", "Start date is invalid.")
    })?;
    let time_value = config
        .time_field
        .as_ref()
        .and_then(|n| field_str(record, n));
    if all_day || (config.time_field.is_none() && time_value.is_none()) {
        let start_naive = date.and_hms_opt(0, 0, 0).unwrap();
        let end_naive = date
            .succ_opt()
            .unwrap_or(date)
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let mut window = TimeWindow::new(
            local_to_utc(start_naive, tz_name),
            local_to_utc(end_naive, tz_name),
        )?;
        window.all_day = true;
        return Ok(window);
    }

    let start_clock = time_value
        .and_then(parse_clock)
        .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let start_naive = NaiveDateTime::new(date, start_clock);
    let start = local_to_utc(start_naive, tz_name);

    let end = if let Some(end_name) = &config.end_field {
        if let Some(end_raw) = field_str(record, end_name) {
            if end_raw.contains('T') {
                canonicalize_datetime(end_raw, tz_name).ok_or_else(|| {
                    QefroError::business("scheduling_invalid_end", "End datetime is invalid.")
                })?
            } else if let Some(end_date) = parse_date(end_raw) {
                let end_clock = config
                    .end_time_field
                    .as_ref()
                    .and_then(|n| field_str(record, n))
                    .and_then(parse_clock)
                    .unwrap_or(NaiveTime::from_hms_opt(23, 59, 0).unwrap());
                local_to_utc(NaiveDateTime::new(end_date, end_clock), tz_name)
            } else {
                start + config.duration()
            }
        } else if let Some(end_clock) = config
            .end_time_field
            .as_ref()
            .and_then(|n| field_str(record, n))
            .and_then(parse_clock)
        {
            end_from_clock(date, start_clock, end_clock, tz_name)?
        } else {
            start + config.duration()
        }
    } else if let Some(end_clock) = config
        .end_time_field
        .as_ref()
        .and_then(|n| field_str(record, n))
        .and_then(parse_clock)
    {
        end_from_clock(date, start_clock, end_clock, tz_name)?
    } else {
        start + config.duration()
    };

    let mut window = TimeWindow::new(start, end)?;
    window.all_day = all_day;
    Ok(window)
}

fn end_from_clock(
    date: NaiveDate,
    start_clock: NaiveTime,
    end_clock: NaiveTime,
    tz_name: &str,
) -> QefroResult<DateTime<Utc>> {
    if end_clock <= start_clock {
        return Err(QefroError::business(
            "scheduling_invalid_range",
            "End must be after start.",
        ));
    }
    Ok(local_to_utc(NaiveDateTime::new(date, end_clock), tz_name))
}

/// Apply default duration to a payload that has start but no end.
pub fn apply_default_end(config: &SchedulingConfig, record: &mut Value, tz_name: &str) {
    let Some(obj) = record.as_object() else {
        return;
    };
    if let Some(end_name) = &config.end_field {
        if obj
            .get(end_name)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            return;
        }
    }
    if let Some(end_time) = &config.end_time_field {
        if obj
            .get(end_time)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            return;
        }
    }
    let Ok(window) = parse_window(config, record, tz_name) else {
        return;
    };
    let Some(obj) = record.as_object_mut() else {
        return;
    };
    if let Some(end_name) = &config.end_field {
        if obj.get(end_name).is_none() || obj.get(end_name) == Some(&Value::Null) {
            obj.insert(end_name.clone(), Value::String(window.end.to_rfc3339()));
        }
    }
    if let Some(end_time) = &config.end_time_field {
        if obj.get(end_time).is_none() || obj.get(end_time) == Some(&Value::Null) {
            let tz = crate::timezone::parse_tz(tz_name);
            let local = window.end.with_timezone(&tz);
            obj.insert(
                end_time.clone(),
                Value::String(local.format("%H:%M").to_string()),
            );
        }
    }
}

pub fn is_blackout(config: &SchedulingConfig, date: NaiveDate) -> bool {
    let key = date.format("%Y-%m-%d").to_string();
    config.blackouts.iter().any(|d| d == &key)
}

pub fn weekday_iso(date: NaiveDate) -> u8 {
    date.weekday().number_from_monday() as u8
}

/// Working windows for a civil date. Empty config means unrestricted.
pub fn working_windows_for(
    config: &SchedulingConfig,
    date: NaiveDate,
) -> Vec<(NaiveTime, NaiveTime)> {
    if config.working_hours.is_empty() {
        return vec![(
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
        )];
    }
    let wd = weekday_iso(date);
    config
        .working_hours
        .iter()
        .filter(|h| h.weekday == wd)
        .filter_map(|h| {
            let start = parse_clock(&h.start)?;
            let end = parse_clock(&h.end)?;
            if end > start {
                Some((start, end))
            } else {
                None
            }
        })
        .collect()
}

pub fn window_within_working_hours(
    config: &SchedulingConfig,
    window: TimeWindow,
    tz_name: &str,
) -> bool {
    if config.working_hours.is_empty() {
        return true;
    }
    let tz = crate::timezone::parse_tz(tz_name);
    let local_start = window.start.with_timezone(&tz);
    let local_end = window.end.with_timezone(&tz);
    let date = local_start.date_naive();
    if local_end.date_naive() != date && !window.all_day {
        // Overnight bookings must fit a window that spans midnight — not supported
        // in the basic working-hours model. Reject unless all-day.
        return false;
    }
    let start_clock = local_start.time();
    let end_clock = local_end.time();
    working_windows_for(config, date)
        .into_iter()
        .any(|(ws, we)| start_clock >= ws && end_clock <= we)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvailabilitySlot {
    pub start: String,
    pub end: String,
    pub available: bool,
}

/// Generate booking slots for a civil date. `booked` windows are already buffered.
pub fn generate_slots(
    config: &SchedulingConfig,
    date: NaiveDate,
    tz_name: &str,
    booked: &[TimeWindow],
) -> Vec<AvailabilitySlot> {
    if is_blackout(config, date) {
        return Vec::new();
    }
    let duration = config.duration();
    let interval = config.slot_interval();
    if duration <= Duration::zero() || interval <= Duration::zero() {
        return Vec::new();
    }
    let windows = working_windows_for(config, date);
    let mut slots = Vec::new();
    for (ws, we) in windows {
        let mut cursor = NaiveDateTime::new(date, ws);
        let window_end = NaiveDateTime::new(date, we);
        while cursor + duration <= window_end {
            let start = local_to_utc(cursor, tz_name);
            let end = start + duration;
            let candidate = TimeWindow {
                start,
                end,
                all_day: false,
            };
            let available = !booked.iter().any(|b| candidate.overlaps(*b));
            let tz = crate::timezone::parse_tz(tz_name);
            slots.push(AvailabilitySlot {
                start: start.with_timezone(&tz).format("%H:%M").to_string(),
                end: end.with_timezone(&tz).format("%H:%M").to_string(),
                available,
            });
            cursor += interval;
        }
    }
    slots
}

fn type_is_start(t: &FieldType) -> bool {
    matches!(t, FieldType::Date | FieldType::DateTime)
}

fn type_is_time(t: &FieldType) -> bool {
    matches!(t, FieldType::Time)
}

fn type_is_end(t: &FieldType, start: &FieldType) -> bool {
    match start {
        FieldType::DateTime => matches!(t, FieldType::DateTime | FieldType::Date),
        FieldType::Date => matches!(t, FieldType::Date | FieldType::DateTime | FieldType::Time),
        _ => type_is_start(t) || type_is_time(t),
    }
}

fn ensure_field(entity: &EntityDef, name: &str, surface: &str) -> Result<(), String> {
    if entity.get_field(name).is_some() {
        Ok(())
    } else {
        Err(format!(
            "scheduling on '{}': {surface} field '{name}' does not exist",
            entity.name
        ))
    }
}

/// Studio / CLI / `qefro validate` checks. No arbitrary code.
pub fn validate_scheduling(entity: &EntityDef, registry: Option<&EntityRegistry>) -> Vec<String> {
    let Some(cfg) = &entity.scheduling else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    if cfg.start_field.is_empty() {
        errors.push(format!(
            "scheduling on '{}': start_field is required",
            entity.name
        ));
        return errors;
    }
    if let Err(e) = ensure_field(entity, &cfg.start_field, "start") {
        errors.push(e);
    } else if let Some(f) = entity.get_field(&cfg.start_field) {
        if !type_is_start(&f.field_type) {
            errors.push(format!(
                "scheduling on '{}': start field '{}' must be date or datetime, not {}",
                entity.name,
                cfg.start_field,
                f.field_type.as_str()
            ));
        }
    }
    if let Some(end) = &cfg.end_field {
        if let Err(e) = ensure_field(entity, end, "end") {
            errors.push(e);
        } else if let (Some(start_f), Some(end_f)) =
            (entity.get_field(&cfg.start_field), entity.get_field(end))
        {
            if !type_is_end(&end_f.field_type, &start_f.field_type) {
                errors.push(format!(
                    "scheduling on '{}': end field '{}' is not compatible with start field '{}'",
                    entity.name, end, cfg.start_field
                ));
            }
        }
    }
    if let Some(time) = &cfg.time_field {
        if let Err(e) = ensure_field(entity, time, "time") {
            errors.push(e);
        } else if let Some(f) = entity.get_field(time) {
            if !type_is_time(&f.field_type) {
                errors.push(format!(
                    "scheduling on '{}': time field '{}' must be time, not {}",
                    entity.name,
                    time,
                    f.field_type.as_str()
                ));
            }
        }
    }
    if let Some(end_time) = &cfg.end_time_field {
        if let Err(e) = ensure_field(entity, end_time, "end_time") {
            errors.push(e);
        } else if let Some(f) = entity.get_field(end_time) {
            if !type_is_time(&f.field_type) {
                errors.push(format!(
                    "scheduling on '{}': end_time field '{}' must be time, not {}",
                    entity.name,
                    end_time,
                    f.field_type.as_str()
                ));
            }
        }
    }
    if let Some(all_day) = &cfg.all_day_field {
        if let Err(e) = ensure_field(entity, all_day, "all_day") {
            errors.push(e);
        }
    }
    if let Some(cap) = &cfg.capacity_field {
        if let Err(e) = ensure_field(entity, cap, "capacity") {
            errors.push(e);
        }
    }
    for resource in &cfg.resources {
        if let Err(e) = ensure_field(entity, resource, "resource") {
            errors.push(e);
            continue;
        }
        let Some(field) = entity.get_field(resource) else {
            continue;
        };
        let Some(rel) = &field.relation else {
            errors.push(format!(
                "scheduling on '{}': resource '{resource}' is not a relation",
                entity.name
            ));
            continue;
        };
        if let Some(registry) = registry {
            if registry.try_get(&rel.target_entity).is_none() {
                errors.push(format!(
                    "scheduling on '{}': resource '{resource}' references missing entity '{}'",
                    entity.name, rel.target_entity
                ));
            } else if let Some(cap) = &cfg.resource_capacity_field {
                if let Some(target) = registry.try_get(&rel.target_entity) {
                    if target.get_field(cap).is_none() {
                        errors.push(format!(
                            "scheduling on '{}': resource capacity field '{cap}' does not exist on '{}'",
                            entity.name, rel.target_entity
                        ));
                    }
                }
            }
        }
    }
    for hours in &cfg.working_hours {
        if !(1..=7).contains(&hours.weekday) {
            errors.push(format!(
                "scheduling on '{}': weekday {} is invalid (use 1–7, Monday–Sunday)",
                entity.name, hours.weekday
            ));
        }
        if parse_clock(&hours.start).is_none() {
            errors.push(format!(
                "scheduling on '{}': working hours start '{}' is invalid",
                entity.name, hours.start
            ));
        }
        if parse_clock(&hours.end).is_none() {
            errors.push(format!(
                "scheduling on '{}': working hours end '{}' is invalid",
                entity.name, hours.end
            ));
        }
        if let (Some(s), Some(e)) = (parse_clock(&hours.start), parse_clock(&hours.end)) {
            if e <= s {
                errors.push(format!(
                    "scheduling on '{}': working hours end must be after start",
                    entity.name
                ));
            }
        }
    }
    for date in &cfg.blackouts {
        if parse_date(date).is_none() {
            errors.push(format!(
                "scheduling on '{}': blackout date '{date}' must be YYYY-MM-DD",
                entity.name
            ));
        }
    }
    errors
}

pub fn conflict_message(resource_label: &str, existing: TimeWindow, tz_name: &str) -> String {
    let tz = crate::timezone::parse_tz(tz_name);
    let start = existing.start.with_timezone(&tz).format("%H:%M");
    let end = existing.end.with_timezone(&tz).format("%H:%M");
    if resource_label.is_empty() {
        format!("This time is already booked from {start} to {end}. Choose another time.")
    } else {
        format!("This resource is already booked from {start} to {end}. Choose another time.")
    }
}

pub fn lock_key(
    tenant: uuid::Uuid,
    entity: &str,
    resource_field: &str,
    resource_id: &str,
    date: &str,
) -> i64 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"qefro:sched:");
    hasher.update(tenant.as_bytes());
    hasher.update(entity.as_bytes());
    hasher.update(resource_field.as_bytes());
    hasher.update(resource_id.as_bytes());
    hasher.update(date.as_bytes());
    let digest = hasher.finalize();
    i64::from_be_bytes(digest[..8].try_into().expect("sha256 prefix"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityDef, FieldDef};
    use chrono::TimeZone;

    fn reservation_entity() -> EntityDef {
        EntityDef::new("Reservation")
            .field(FieldDef::date("reservation_date").required())
            .field(FieldDef::time("reservation_time").required())
            .field(FieldDef::time("end_time").nullable())
            .field(FieldDef::relation("table_id", "DiningTable").nullable())
            .field(FieldDef::integer("party_size").required())
            .scheduling(
                SchedulingConfig::new("reservation_date")
                    .time_field("reservation_time")
                    .end_time_field("end_time")
                    .resource("table_id")
                    .capacity("party_size", "seats")
                    .conflict()
                    .calendar()
                    .duration_minutes(90)
                    .working_hours(WorkingHours::everyday("11:00", "22:00")),
            )
            .build()
    }

    #[test]
    fn overlap_half_open() {
        let a0 = Utc.with_ymd_and_hms(2026, 8, 30, 10, 0, 0).unwrap();
        let a1 = Utc.with_ymd_and_hms(2026, 8, 30, 11, 0, 0).unwrap();
        let b0 = Utc.with_ymd_and_hms(2026, 8, 30, 11, 0, 0).unwrap();
        let b1 = Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap();
        assert!(!intervals_overlap(a0, a1, b0, b1));
        let c0 = Utc.with_ymd_and_hms(2026, 8, 30, 10, 30, 0).unwrap();
        let c1 = Utc.with_ymd_and_hms(2026, 8, 30, 11, 30, 0).unwrap();
        assert!(intervals_overlap(a0, a1, c0, c1));
    }

    #[test]
    fn date_time_window_uses_tenant_zone() {
        let cfg = SchedulingConfig::new("reservation_date")
            .time_field("reservation_time")
            .end_time_field("end_time")
            .duration_minutes(90);
        let rec = serde_json::json!({
            "reservation_date": "2026-08-30",
            "reservation_time": "19:30",
            "end_time": "21:00"
        });
        let window = parse_window(&cfg, &rec, "Asia/Kolkata").unwrap();
        assert_eq!(window.start.to_rfc3339(), "2026-08-30T14:00:00+00:00");
        assert_eq!(window.end.to_rfc3339(), "2026-08-30T15:30:00+00:00");
    }

    #[test]
    fn missing_end_uses_duration() {
        let cfg = SchedulingConfig::new("reservation_date")
            .time_field("reservation_time")
            .duration_minutes(90);
        let rec = serde_json::json!({
            "reservation_date": "2026-08-30",
            "reservation_time": "10:00"
        });
        let window = parse_window(&cfg, &rec, "UTC").unwrap();
        assert_eq!((window.end - window.start).num_minutes(), 90);
    }

    #[test]
    fn end_must_follow_start() {
        let cfg = SchedulingConfig::new("reservation_date")
            .time_field("reservation_time")
            .end_time_field("end_time");
        let rec = serde_json::json!({
            "reservation_date": "2026-08-30",
            "reservation_time": "11:00",
            "end_time": "10:00"
        });
        let err = parse_window(&cfg, &rec, "UTC").unwrap_err();
        assert!(err.to_string().contains("after start"));
    }

    #[test]
    fn buffer_expands_conflict_window() {
        let start = Utc.with_ymd_and_hms(2026, 8, 30, 10, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 8, 30, 10, 30, 0).unwrap();
        let w = TimeWindow::new(start, end).unwrap().with_buffer(10, 10);
        assert_eq!(
            w.start,
            Utc.with_ymd_and_hms(2026, 8, 30, 9, 50, 0).unwrap()
        );
        assert_eq!(w.end, Utc.with_ymd_and_hms(2026, 8, 30, 10, 40, 0).unwrap());
    }

    #[test]
    fn slots_skip_booked_and_blackout() {
        let cfg = SchedulingConfig::new("starts_at")
            .duration_minutes(30)
            .slot_interval_minutes(30)
            .working_hours(vec![WorkingHours::new(7, "09:00", "11:00")])
            .blackouts(&["2026-08-31"]);
        let sunday = NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
        assert_eq!(weekday_iso(sunday), 7);
        let booked_start = local_to_utc(
            NaiveDateTime::new(sunday, NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
            "UTC",
        );
        let booked = [TimeWindow {
            start: booked_start,
            end: booked_start + Duration::minutes(30),
            all_day: false,
        }];
        let slots = generate_slots(&cfg, sunday, "UTC", &booked);
        assert!(slots.iter().any(|s| s.start == "09:00" && s.available));
        assert!(slots.iter().any(|s| s.start == "10:00" && !s.available));
        let monday = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        assert!(generate_slots(&cfg, monday, "UTC", &[]).is_empty());
    }

    #[test]
    fn breaks_are_separate_windows() {
        let cfg = SchedulingConfig::new("starts_at")
            .duration_minutes(60)
            .slot_interval_minutes(60)
            .working_hours(vec![
                WorkingHours::new(1, "09:00", "13:00"),
                WorkingHours::new(1, "14:00", "17:00"),
            ]);
        let monday = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        let slots = generate_slots(&cfg, monday, "UTC", &[]);
        let starts: Vec<_> = slots.iter().map(|s| s.start.as_str()).collect();
        assert!(starts.contains(&"09:00"));
        assert!(starts.contains(&"12:00"));
        assert!(!starts.contains(&"13:00"));
        assert!(starts.contains(&"14:00"));
    }

    #[test]
    fn working_hours_reject_outside() {
        let cfg = SchedulingConfig::new("reservation_date")
            .time_field("reservation_time")
            .end_time_field("end_time")
            .working_hours(WorkingHours::everyday("11:00", "22:00"));
        let inside = parse_window(
            &cfg,
            &serde_json::json!({
                "reservation_date": "2026-08-30",
                "reservation_time": "19:00",
                "end_time": "20:30"
            }),
            "UTC",
        )
        .unwrap();
        assert!(window_within_working_hours(&cfg, inside, "UTC"));
        let outside = parse_window(
            &cfg,
            &serde_json::json!({
                "reservation_date": "2026-08-30",
                "reservation_time": "08:00",
                "end_time": "09:00"
            }),
            "UTC",
        )
        .unwrap();
        assert!(!window_within_working_hours(&cfg, outside, "UTC"));
    }

    #[test]
    fn validate_requires_existing_fields() {
        let entity = reservation_entity();
        let table = EntityDef::new("DiningTable")
            .field(FieldDef::integer("seats").required())
            .build();
        let mut registry = EntityRegistry::new();
        registry.register(entity.clone()).unwrap();
        registry.register(table).unwrap();
        assert!(validate_scheduling(&entity, Some(&registry)).is_empty());

        let bad = EntityDef::new("Reservation")
            .field(FieldDef::date("reservation_date"))
            .scheduling(SchedulingConfig::new("starts_at").resource("table_id"))
            .build();
        let errs = validate_scheduling(&bad, None);
        assert!(errs.iter().any(|e| e.contains("starts_at")));
        assert!(errs.iter().any(|e| e.contains("table_id")));
    }

    #[test]
    fn datetime_rfc3339_is_utc() {
        let cfg = SchedulingConfig::new("starts_at").end_field("ends_at");
        let rec = serde_json::json!({
            "starts_at": "2026-08-30T14:00:00Z",
            "ends_at": "2026-08-30T15:00:00Z"
        });
        let window = parse_window(&cfg, &rec, "Asia/Kolkata").unwrap();
        assert_eq!(window.start.to_rfc3339(), "2026-08-30T14:00:00+00:00");
    }
}
