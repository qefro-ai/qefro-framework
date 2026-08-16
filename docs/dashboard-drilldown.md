# Dashboard drill-down

Metric cards that already declare filters become links into the generic list with those filters translated into the existing query model.

```
Today's Reservations  32
        ↓ click
/reservations?reservation_date.between=2026-08-16,2026-08-16
```

`today` / `this_month` / other date presets use `datePresetRange` — never SQL, never a new filter language.

Charts expose `group_by`. Clicking a segment sets a dashboard-level extra query parameter (for example `status=Pending`) and **refetches** every card through `GET /api/v1/dashboards/{name}?status=Pending`. Extra filters replace the same field on each card; unknown columns are ignored by `parse_query`.

Dashboard-level Date range / Status / Branch controls work the same way. Widgets refresh from the server. The frontend does not aggregate or filter rows in React.
