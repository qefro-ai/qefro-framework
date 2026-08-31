# Scheduling

Scheduling is a reusable business capability. An existing entity opts in. Workflow still owns the record lifecycle.

```text
EntityDef
   │
   ▼
EntityService
   │
Scheduling capability
   │
┌──────────┼──────────┐
▼          ▼          ▼
Calendar  Availability  Resources
   │          │          │
   └──────────┼──────────┘
              ▼
      Conflict detection
              │
       Business rules
              │
           Workflow
              │
        Outbox / Activity
              │
        Communication
```

**Scheduling determines whether a time or resource is available. Workflow determines what happens to the business record.**

Do not build RestaurantScheduling, ClinicScheduling, or a second calendar framework.

## Schedulable entities

```rust
EntityDef::new("Reservation")
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
```

`start_field` is a date or datetime. When it is a date, `time_field` / `end_time_field` hold clock times (the restaurant Reservation model). Datetime entities use `start_field` / `end_field` only.

YAML under `entities/` is equivalent. Studio **Scheduling** publishes `entity.scheduling`. `qefro inspect Reservation` prints Start, End, Resource, Calendar, and Conflict. `qefro validate` rejects missing fields, incompatible types, and unknown resource relations.

## Start and end

`ends_at > starts_at` is enforced by the existing `ValidationRule::compare` when both fields exist, and again when the scheduler parses a window. Missing end uses `duration_minutes` (default 60). Half-open intervals: `10:00–11:00` does not conflict with `11:00–12:00`.

## Timezones

API and database values stay UTC for datetime fields. Date + time pairs are civil times in the **tenant** timezone (`OpContext.timezone`). The generic UI uses `utcToLocalParts` / `localToUtcIso`. Do not assume the server local zone.

## All-day

Set `all_day_field` or omit a time field on a date-only start. Day and week views render all-day rows separately from timed slots.

## Resources

Resources are normal many-to-one relations: Table, Room, Doctor, Vehicle. `resources` is a list of field names. Conflict and capacity run for each populated resource. There is no separate resource planner.

Capacity compares the booking field (`party_size`) to the related record (`seats`). A party of 12 at a table of 10 fails even if the time is free.

## Availability

Working hours are recurring weekday windows (`weekday` 1–7, Monday–Sunday). Two windows on the same day represent a break (`09:00–13:00` and `14:00–17:00`). `blackouts` are explicit `YYYY-MM-DD` dates — applications configure them; Qefro does not ship a country holiday database.

`GET /api/v1/{slug}/availability?date=2026-08-31&table_id=…` returns slots from working hours, duration, interval, blackouts, and existing bookings. Availability is computed on demand. Do not cache it in a way that would allow double booking.

## Conflict detection

Create and update run inside one transaction:

```text
BEGIN
advisory lock (tenant, entity, resource, date)
SELECT overlapping rows FOR UPDATE
reject if overlap
INSERT / UPDATE
outbox
COMMIT
```

The frontend is not authoritative. The error is `scheduling_conflict` (HTTP 409) with a message such as “This resource is already booked from 10:00 to 11:00. Choose another time.” That is a **business** 409, not optimistic concurrency. Optimistic lock still uses `_expected_updated_at` and the “Record changed / Reload” dialog.

Cancelled and Completed (configurable `ignore_states`) do not occupy the resource.

Two concurrent bookings for the same resource and overlapping time: one commit succeeds, the other receives `scheduling_conflict`. PostgreSQL `pg_advisory_xact_lock` serializes the check.

## Calendar

The existing generic Calendar view (not a second library) supports month, week, day, and agenda, plus today, previous, next, and a date picker. Range queries use `start.between` so the client does not download the year. Events render from metadata (`title`, `subtitle`, resource expansion). Click opens EntityDetail. Empty slots open the generic form with start/end query defaults. Drag and hour-drop PATCH start/end through EntityService, including `_expected_updated_at`. Overlapping timed events sit side by side. Status color uses theme tokens, not application-specific hex.

## Operations, workflow, activity

Cancellation and confirm stay on Workflow transitions. **Reschedule** is a generic operation registered for every schedulable entity. It patches time fields through EntityService (validation, conflict, activity, audit, outbox). Domain events: `{entity}.created`, `{entity}.rescheduled`, `{entity}.reminder`, plus whatever Workflow already emits (`reservation.confirmed`, `reservation.cancelled`).

## Notifications and reminders

The scheduler does not send mail. `reminder_minutes` enqueues the existing JobQueue (`schedule.reminder`). The job emits `{entity}.reminder` on the Outbox. A `CommunicationDef` on that event delivers through the Communication Runtime.

## Permissions

Entity, resource, row policy, workflow, and operation permissions apply as usual. Calendar metadata only includes fields the user can already see. All queries are tenant-scoped.

## SDK

```ts
api.list("reservations", params);           // date range via field.between
api.create("reservations", body);
api.update("reservations", id, patch);      // drag / resize
api.execute({ entity: "reservations", id, action: "reschedule", inputs });
api.availability("reservations", params);
```

There is no separate scheduling SDK.

## Reports

Use `ReportDef` on the schedulable entity (bookings by day, counts by resource). Utilization is that count against configured capacity — not a second analytics engine.

## Out of scope

AI scheduling, route optimization, workforce planning, Google/Outlook sync, complex recurrence, country holiday databases, appointment marketplaces, video conferencing, and payment scheduling.
