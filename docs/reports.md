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

`COUNT`, `SUM`, `AVG`, `MIN`, `MAX`. `SUM`/`AVG`/`MIN`/`MAX` require a numeric field. `SUM(string)` is rejected. Execution is server-side (`LIMIT 500` groups). The browser never totals large record sets.

```http
GET /api/v1/{slug}/aggregates?group_by=status&metric=sum&field=amount
POST /api/v1/reports/{name}/run
```

## Filters

`equals`, `not equals`, `contains`, `starts with`, `between`, `in`, `not in`, `empty`, `not empty`, `greater than`, `less than` (and `gt`/`lt`/`eq` aliases). Payloads with `sql` or `query` keys are rejected.

## Security

Reports honor tenant isolation, application entitlements, RBAC list permission, and hidden fields. A user cannot invent a field name to bypass visibility.

## UI and agents

`GET /api/v1/meta/reports` lists definitions. `QefroClient.getReport()` / `runReport()` call the same routes. Agents use `EntityOps::run_report` (`run_report` tool). Charts are `bar`, `line`, `area`, `pie`, `donut` — the renderer does not branch on entity name.
