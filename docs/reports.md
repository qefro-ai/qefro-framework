# Reports

Metadata-driven reports reuse the existing filter engine. Arbitrary SQL is rejected.

```rust
ReportDef::new("sales-by-day", "Order")
    .label("Sales By Day")
    .module("restaurant")
    .fields(&["order_date", "grand_total"])
    .group_by(&["order_date"])
    .sum("grand_total")
    .chart("bar")
```

```json
{
  "name": "sales-by-day",
  "entity": "Order",
  "fields": ["order_date", "grand_total"],
  "group_by": ["order_date"],
  "aggregations": { "grand_total": "SUM" }
}
```

## Aggregations

`COUNT`, `SUM`, `AVG`, `MIN`, `MAX`. `SUM`/`AVG`/`MIN`/`MAX` require a numeric field. `SUM(string)` is rejected.

## Filters

`equals`, `not equals`, `contains`, `starts with`, `between`, `in`, `not in`, `empty`, `not empty`, `greater than`, `less than` (and `gt`/`lt`/`eq` aliases). Payloads with `sql` or `query` keys are rejected.

## Security

Reports honor tenant isolation, application entitlements, RBAC list permission, and hidden fields. A user cannot invent a field name to bypass visibility.

## UI

`GET /api/v1/meta/reports` lists definitions. `POST /api/v1/reports/{name}/run` returns `rows` plus `series` for the existing dashboard chart component (`bar`, `line`, `donut`).
