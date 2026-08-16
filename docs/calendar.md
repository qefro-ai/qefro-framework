# Calendar

A registered collection view for entities with a date or datetime field.

```yaml
views:
  calendar:
    start: reservation_date
    time: reservation_time
    title: guest_name
    subtitle: status
```

Day and week layouts show hour slots; month is a grid. Datetimes are stored as UTC and displayed in the tenant timezone (existing `utcToLocalParts` / `localToUtcIso`).

## Create

Clicking an empty slot opens the **generic entity form** with query defaults, for example:

`/reservations/new?reservation_date=2026-08-16&reservation_time=19:00`

No custom React create page.

## Record click

Events link to the generic detail view `/{slug}/{id}`.

## Reschedule

Drag uses `PATCH` on the start field through EntityService. Readonly fields and document `lock_states` show `Cannot reschedule this record.` The server remains the authority.

## Filters

The same `FilterBar` and list query language (`field.between=from,to`, status, owner, …). The calendar range is applied as a `between` filter on the start field so the client does not download the whole database.
