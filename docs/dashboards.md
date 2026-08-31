# Dashboards

Dashboard definitions stay in application metadata. The frontend does not embed SQL.

```rust
DashboardDef::new("restaurant-ops", "Floor operations")
    .card(DashboardCard::kpi("Today's reservations", "Reservation").filter("reservation_date", "today"))
    .card(DashboardCard::sum("Today's sales", "Payment", "amount").filter("status", "captured").roles(&["Admin", "Manager"]))
    .card(DashboardCard::count("Upcoming pickups", "Order").filter("status", "Scheduled"))
    .card(DashboardCard::workflow("Kitchen status", "Order"))
    .card(DashboardCard::chart("Sales trend", "Order", "area", "order_date").metric_name("sum").measure_field("grand_total"))
    .card(DashboardCard::activity("Recent order events", "Order", 8))
    .card(DashboardCard::audit("Changes today").roles(&["Admin"]))
```

Card kinds:

| kind | API payload |
| --- | --- |
| `metric` / `kpi` | `{ value }` (`count`, `sum`, `avg`, `min`, `max`) |
| `chart` / `status_breakdown` / `workflow` | `{ series: [{ label, value }], chart: bar\|line\|area\|pie\|donut }` |
| `list` / `table` / `saved_view` | `{ items, total }` from the entity list API |
| `activity` | `{ items }` from `qefro_activity` (not a second timeline store) |
| `report` | `{ rows, series }` from `run_report` |
| `audit` | `{ value }` — Admin only |

Unauthorized widgets are **skipped**, not 403 for the whole dashboard. `roles` on a card hides it from other roles; do not duplicate dashboards per role.

A dashboard is not a composed **page**. Keep KPIs and charts here. Put operational lists, kanban, filters, and actions on `PageDef` — see [Pages](pages.md). Pages may embed a dashboard card as a widget; they still load that card through `GET /api/v1/dashboards/{name}`.

All queries run through `EntityService` with tenant, app entitlement, and `List` permission. Charts are simple SVG; Studio edits metadata overlays (add / reorder / title / source / size / saved report or view), not custom React widgets.

Activity widgets use `qefro_activity`. Workflow widgets group by existing status / `_workflow` data. Clicking a workflow segment opens the generic list with that filter.

Metric cards drill into the generic list using the card's existing filters. See [dashboard-drilldown.md](dashboard-drilldown.md).

`QefroClient.getDashboard()` loads cards. Agents use `EntityOps::get_dashboard`.
