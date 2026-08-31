//! Server-side scheduling: conflict locks, availability, and reminder jobs.
//!
//! Conflict checks run inside the same transaction as insert/update. Availability
//! is computed on demand and is never cached per user.

use crate::jobs::JobHandler;
use crate::outbox::Outbox;
use crate::query::{column_ident, table_ident};
use crate::repository::{record_id, EntityRepository};
use crate::service::EntityService;
use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use qefro_core::{
    apply_default_end, conflict_message, generate_slots, ident::snake_case, is_blackout, lock_key,
    parse_date, parse_window, quote_ident, window_within_working_hours, EntityDef, EntityRegistry,
    OpContext, QefroError, QefroResult, SchedulingConfig, TimeWindow,
};
use qefro_events::DomainEvent;
use qefro_permissions::Action;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, QueryBuilder, Row, Transaction};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

pub const SCHEDULE_REMINDER_JOB: &str = "schedule.reminder";

pub async fn enforce_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    repo: &EntityRepository,
    registry: &EntityRegistry,
    ctx: &OpContext,
    entity: &EntityDef,
    record: &Value,
    exclude_id: Option<Uuid>,
) -> QefroResult<()> {
    let Some(config) = &entity.scheduling else {
        return Ok(());
    };
    let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if config.ignores_status(status) {
        return Ok(());
    }
    if record
        .get(&config.start_field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .is_none()
    {
        return Ok(());
    }
    let window = parse_window(config, record, &ctx.timezone)?;
    let date = window.local_date(&ctx.timezone);
    if is_blackout(config, date) {
        return Err(QefroError::business(
            "scheduling_unavailable",
            "This date is unavailable.",
        ));
    }
    if !window_within_working_hours(config, window, &ctx.timezone) {
        return Err(QefroError::business(
            "scheduling_outside_hours",
            "This time is outside working hours. Choose another time.",
        ));
    }
    let buffered = window.with_buffer(
        config.buffer_before_minutes.unwrap_or(0),
        config.buffer_after_minutes.unwrap_or(0),
    );
    if !config.conflict {
        return Ok(());
    }
    if config.resources.is_empty() {
        advisory_lock(tx, ctx.tenant_id, &entity.name, "", "", &date.to_string()).await?;
        let existing = load_candidates(
            tx, entity, ctx, config, None, None, date, buffered, exclude_id, true,
        )
        .await?;
        reject_overlap(config, buffered, &existing, ctx, "")?;
        return Ok(());
    }
    for field_name in &config.resources {
        let Some(resource_id) = record
            .get(field_name)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        advisory_lock(
            tx,
            ctx.tenant_id,
            &entity.name,
            field_name,
            resource_id,
            &date.to_string(),
        )
        .await?;
        if let (Some(cap_field), Some(res_cap)) =
            (&config.capacity_field, &config.resource_capacity_field)
        {
            check_capacity(
                tx,
                repo,
                registry,
                ctx,
                entity,
                field_name,
                resource_id,
                record,
                cap_field,
                res_cap,
            )
            .await?;
        }
        let existing = load_candidates(
            tx,
            entity,
            ctx,
            config,
            Some(field_name.as_str()),
            Some(resource_id),
            date,
            buffered,
            exclude_id,
            true,
        )
        .await?;
        reject_overlap(config, buffered, &existing, ctx, field_name)?;
    }
    Ok(())
}

async fn advisory_lock(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Uuid,
    entity: &str,
    resource_field: &str,
    resource_id: &str,
    date: &str,
) -> QefroResult<()> {
    let key = lock_key(tenant, entity, resource_field, resource_id, date);
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    Ok(())
}

async fn check_capacity(
    tx: &mut Transaction<'_, Postgres>,
    repo: &EntityRepository,
    registry: &EntityRegistry,
    ctx: &OpContext,
    entity: &EntityDef,
    resource_field: &str,
    resource_id: &str,
    record: &Value,
    capacity_field: &str,
    resource_capacity_field: &str,
) -> QefroResult<()> {
    let Some(needed) = record.get(capacity_field).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().map(|n| n as i64))
            .or_else(|| v.as_f64().map(|n| n as i64))
    }) else {
        return Ok(());
    };
    let Some(field) = entity.get_field(resource_field) else {
        return Ok(());
    };
    let Some(rel) = &field.relation else {
        return Ok(());
    };
    let target = registry.get(&rel.target_entity)?;
    let id = Uuid::parse_str(resource_id)
        .map_err(|_| QefroError::bad_request(format!("invalid {resource_field}")))?;
    let resource = repo.get_tx(tx, &target, ctx, id, true).await?;
    let Some(seats) = resource.get(resource_capacity_field).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().map(|n| n as i64))
            .or_else(|| v.as_f64().map(|n| n as i64))
    }) else {
        return Ok(());
    };
    if needed > seats {
        return Err(QefroError::business(
            "scheduling_capacity",
            format!("This resource holds {seats}, but the booking needs {needed}."),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn load_candidates(
    tx: &mut Transaction<'_, Postgres>,
    entity: &EntityDef,
    ctx: &OpContext,
    config: &SchedulingConfig,
    resource_field: Option<&str>,
    resource_id: Option<&str>,
    date: NaiveDate,
    window: TimeWindow,
    exclude_id: Option<Uuid>,
    lock: bool,
) -> QefroResult<Vec<Value>> {
    let table = table_ident(entity)?;
    let mut qb = QueryBuilder::<Postgres>::new("SELECT to_jsonb(t.*) FROM ");
    qb.push(&table);
    qb.push(" t WHERE TRUE");
    if entity.tenant_owned {
        qb.push(" AND ");
        qb.push(quote_ident("tenant_id")?);
        qb.push(" = ");
        qb.push_bind(ctx.tenant_id);
    }
    if entity.soft_delete {
        qb.push(" AND ");
        qb.push(quote_ident("deleted_at")?);
        qb.push(" IS NULL");
    }
    if let (Some(field), Some(id)) = (resource_field, resource_id) {
        qb.push(" AND ");
        qb.push(column_ident(entity, field)?);
        qb.push(" = ");
        if let Ok(u) = Uuid::parse_str(id) {
            qb.push_bind(u);
        } else {
            qb.push_bind(id.to_string());
        }
    }
    let start_field = entity.get_field(&config.start_field);
    match start_field.map(|f| &f.field_type) {
        Some(qefro_core::FieldType::Date) => {
            qb.push(" AND ");
            qb.push(column_ident(entity, &config.start_field)?);
            qb.push(" = ");
            qb.push_bind(date);
        }
        _ => {
            qb.push(" AND ");
            qb.push(column_ident(entity, &config.start_field)?);
            qb.push(" < ");
            qb.push_bind(window.end + Duration::hours(24));
            qb.push(" AND ");
            qb.push(column_ident(entity, &config.start_field)?);
            qb.push(" >= ");
            qb.push_bind(window.start - Duration::hours(24));
        }
    }
    if let Some(id) = exclude_id {
        qb.push(" AND ");
        qb.push(quote_ident("id")?);
        qb.push(" <> ");
        qb.push_bind(id);
    }
    if lock {
        qb.push(" FOR UPDATE");
    }
    let rows = qb
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    let mut items = Vec::new();
    for row in rows {
        let value: Value = row
            .try_get(0)
            .map_err(|e| QefroError::database(e.to_string()))?;
        items.push(value);
    }
    Ok(items)
}

fn reject_overlap(
    config: &SchedulingConfig,
    proposed: TimeWindow,
    existing: &[Value],
    ctx: &OpContext,
    resource_field: &str,
) -> QefroResult<()> {
    for row in existing {
        let status = row.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if config.ignores_status(status) {
            continue;
        }
        let Ok(other) = parse_window(config, row, &ctx.timezone) else {
            continue;
        };
        let other = other.with_buffer(
            config.buffer_before_minutes.unwrap_or(0),
            config.buffer_after_minutes.unwrap_or(0),
        );
        if proposed.overlaps(other) {
            return Err(QefroError::business(
                "scheduling_conflict",
                conflict_message(resource_field, other, &ctx.timezone),
            ));
        }
    }
    Ok(())
}

pub async fn availability(
    service: &EntityService,
    ctx: &OpContext,
    entity_name: &str,
    params: &std::collections::HashMap<String, String>,
) -> QefroResult<Value> {
    let entity = service.registry().get(entity_name)?;
    service
        .permissions()
        .check(ctx, &entity.name, Action::List)?;
    let Some(config) = &entity.scheduling else {
        return Err(QefroError::bad_request(format!(
            "{} is not a schedulable entity",
            entity.name
        )));
    };
    let date = params
        .get("date")
        .and_then(|s| parse_date(s))
        .ok_or_else(|| QefroError::bad_request("date (YYYY-MM-DD) is required"))?;
    let mut probe = json!({
        config.start_field.clone(): date.format("%Y-%m-%d").to_string(),
    });
    if let Some(time) = &config.time_field {
        if let Some(obj) = probe.as_object_mut() {
            obj.insert(time.clone(), json!("00:00"));
        }
    }
    for field in &config.resources {
        if let Some(value) = params.get(field) {
            if let Some(obj) = probe.as_object_mut() {
                obj.insert(field.clone(), json!(value));
            }
        }
    }
    if let Some(raw) = params.get("duration") {
        if let Ok(mins) = raw.parse::<u32>() {
            let mut cfg = config.clone();
            cfg.duration_minutes = Some(mins);
            return availability_with(service, ctx, &entity, &cfg, date, &probe).await;
        }
    }
    availability_with(service, ctx, &entity, config, date, &probe).await
}

async fn availability_with(
    service: &EntityService,
    ctx: &OpContext,
    entity: &EntityDef,
    config: &SchedulingConfig,
    date: NaiveDate,
    probe: &Value,
) -> QefroResult<Value> {
    let dummy_start = date.and_hms_opt(12, 0, 0).unwrap();
    let dummy = TimeWindow {
        start: qefro_core::local_to_utc(dummy_start, &ctx.timezone),
        end: qefro_core::local_to_utc(dummy_start, &ctx.timezone) + config.duration(),
        all_day: false,
    };
    let mut tx = service
        .pool()
        .begin()
        .await
        .map_err(|e| QefroError::database(e.to_string()))?;
    let mut booked = Vec::new();
    if config.resources.is_empty() {
        let rows = load_candidates(
            &mut tx, entity, ctx, config, None, None, date, dummy, None, false,
        )
        .await?;
        collect_booked(config, ctx, &rows, &mut booked);
    } else {
        let mut any = false;
        for field in &config.resources {
            let Some(id) = probe.get(field).and_then(|v| v.as_str()) else {
                continue;
            };
            any = true;
            let rows = load_candidates(
                &mut tx,
                entity,
                ctx,
                config,
                Some(field.as_str()),
                Some(id),
                date,
                dummy,
                None,
                false,
            )
            .await?;
            collect_booked(config, ctx, &rows, &mut booked);
        }
        if !any {
            let rows = load_candidates(
                &mut tx, entity, ctx, config, None, None, date, dummy, None, false,
            )
            .await?;
            collect_booked(config, ctx, &rows, &mut booked);
        }
    }
    let _ = tx.rollback().await;
    let slots = generate_slots(config, date, &ctx.timezone, &booked);
    Ok(json!({
        "entity": entity.name,
        "date": date.format("%Y-%m-%d").to_string(),
        "duration_minutes": config.duration_minutes.unwrap_or(60),
        "slots": slots,
    }))
}

fn collect_booked(
    config: &SchedulingConfig,
    ctx: &OpContext,
    rows: &[Value],
    booked: &mut Vec<TimeWindow>,
) {
    for row in rows {
        let status = row.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if config.ignores_status(status) {
            continue;
        }
        if let Ok(window) = parse_window(config, row, &ctx.timezone) {
            booked.push(window.with_buffer(
                config.buffer_before_minutes.unwrap_or(0),
                config.buffer_after_minutes.unwrap_or(0),
            ));
        }
    }
}

pub async fn enqueue_reminder_tx(
    jobs: &crate::jobs::JobQueue,
    tx: &mut Transaction<'_, Postgres>,
    ctx: &OpContext,
    entity: &EntityDef,
    record: &Value,
) -> QefroResult<()> {
    let Some(config) = &entity.scheduling else {
        return Ok(());
    };
    let Some(minutes) = config.reminder_minutes else {
        return Ok(());
    };
    let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if config.ignores_status(status) {
        return Ok(());
    }
    let Ok(window) = parse_window(config, record, &ctx.timezone) else {
        return Ok(());
    };
    let run_at = window.start - Duration::minutes(minutes);
    if run_at <= Utc::now() {
        return Ok(());
    }
    let id = record_id(record)?;
    let key = format!(
        "sched-reminder:{}:{}:{}",
        entity.name,
        id,
        window.start.to_rfc3339()
    );
    jobs.enqueue_tx(
        tx,
        ctx,
        SCHEDULE_REMINDER_JOB,
        json!({
            "entity": entity.name,
            "record_id": id,
            "starts_at": window.start.to_rfc3339(),
            "run_at": run_at.to_rfc3339(),
            "idempotency_key": key,
        }),
    )
    .await?;
    Ok(())
}

fn reminder_id(kind: &str, entity: &str, record: Uuid, start: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"qefro:sched-reminder:");
    hasher.update(kind.as_bytes());
    hasher.update(entity.as_bytes());
    hasher.update(record.as_bytes());
    hasher.update(start.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

pub struct ScheduleReminderJob {
    entities: OnceLock<Arc<EntityService>>,
}

impl ScheduleReminderJob {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entities: OnceLock::new(),
        })
    }

    pub fn bind(&self, entities: Arc<EntityService>) {
        let _ = self.entities.set(entities);
    }
}

#[async_trait]
impl JobHandler for ScheduleReminderJob {
    fn worker_safe(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &OpContext, payload: &Value) -> QefroResult<()> {
        let Some(entities) = self.entities.get() else {
            return Err(QefroError::internal("schedule reminder job is not bound"));
        };
        let entity_name = payload
            .get("entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| QefroError::bad_request("entity is required"))?;
        let id = payload
            .get("record_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| QefroError::bad_request("record_id is required"))?;
        let expected = payload
            .get("starts_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let entity = entities.registry().get(entity_name)?;
        let Some(config) = &entity.scheduling else {
            return Ok(());
        };
        let record = match entities.repo.get(&entity, ctx, id).await {
            Ok(row) => row,
            Err(QefroError::NotFound { .. }) => return Ok(()),
            Err(e) => return Err(e),
        };
        if record.get("deleted_at").and_then(|v| v.as_str()).is_some() {
            return Ok(());
        }
        let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if config.ignores_status(status) {
            return Ok(());
        }
        let Ok(window) = parse_window(config, &record, &ctx.timezone) else {
            return Ok(());
        };
        if window.start.to_rfc3339() != expected {
            return Ok(());
        }
        let mut event_payload = record.clone();
        qefro_core::strip_secrets(Some(&entity), &mut event_payload);
        let record_uuid = record_id(&record)?;
        let mut specific = DomainEvent::new(
            format!("{}.reminder", snake_case(&entity.name)),
            entity.name.clone(),
            record_uuid,
            ctx.tenant_id,
            event_payload.clone(),
        );
        specific.id = reminder_id("specific", &entity.name, record_uuid, expected);
        let mut generic = DomainEvent::new(
            "entity.reminder".to_string(),
            entity.name.clone(),
            record_uuid,
            ctx.tenant_id,
            event_payload,
        );
        generic.id = reminder_id("generic", &entity.name, record_uuid, expected);
        let mut tx = entities
            .pool()
            .begin()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        Outbox::enqueue_many_tx(&mut tx, &[specific, generic]).await?;
        tx.commit()
            .await
            .map_err(|e| QefroError::database(e.to_string()))?;
        let _ = entities.dispatch_outbox().await;
        Ok(())
    }
}

pub fn prepare_record(entity: &EntityDef, data: &mut Value, tz_name: &str) {
    if let Some(config) = &entity.scheduling {
        apply_default_end(config, data, tz_name);
    }
}
