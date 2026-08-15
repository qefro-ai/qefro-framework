# Dashboards

Dashboard definitions stay in application metadata. The frontend does not embed SQL.

```rust
DashboardDef::new("restaurant-ops", "Restaurant operations")
    .card(DashboardCard::count("Today's reservations", "Reservation").filter("reservation_date", "today"))
    .card(DashboardCard::sum("Today's sales", "Payment", "amount").filter("status", "captured"))
    .card(DashboardCard::status_breakdown("Reservations by status", "Reservation", "status"))
    .card(DashboardCard::recent("Recent reservations", "Reservation", 8))
    .card(DashboardCard::chart("Orders", "Order", "bar", "status"))
```

Card kinds:

| kind | API payload |
| --- | --- |
| `metric` | `{ value }` (`count` or `sum`) |
| `chart` / `status_breakdown` | `{ series: [{ label, value }], chart: bar\|line\|pie\|donut }` |
| `list` / `table` / `activity` | `{ items, total }` from the entity list API |

All queries run through `EntityService` with tenant, app entitlement, and `List` permission. Charts are simple SVG; there is no drag-and-drop dashboard builder.
