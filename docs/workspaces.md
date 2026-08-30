# Workspaces

A workspace is navigation plus a default dashboard, derived from `AppModule` metadata — not hardcoded restaurant (or CRM) chrome in the framework.

```http
GET /api/v1/meta/workspace
```

```json
{
  "navigation": [
    { "label": "Orders", "entity": "Order", "slug": "orders" },
    { "label": "Kitchen", "entity": "Order", "slug": "orders", "query": "status=Preparing", "view": "kanban" }
  ],
  "default_dashboard": "restaurant-ops",
  "dashboards": [{ "name": "restaurant-ops", "label": "Restaurant operations" }],
  "reports": []
}
```

`GET /api/v1/meta/ui` includes the same `workspace` object. `TenantUiConfig.navigation` can still override the slug list.

Navigation items may include a `section` heading. The generic sidebar groups those items. Workspace `shortcuts` (create, list, dashboard, reports) are derived from the same navigation, dashboards, and reports — filtered by permission.

```rust
AppModule::new("restaurant")
    .nav(NavItem::new("Orders", "Order").section("Operations"))
    .nav(NavItem::new("Kitchen", "Order").query("status=Preparing").view("kanban").section("Operations"))
    .nav(NavItem::new("Customers", "Customer").section("Catalog"))
    .dashboard(dashboard::ops())
```

The homepage still loads `GET /api/v1/dashboards/{name}`. Restaurant, CRM, Inventory, and Helpdesk all use this workspace. Configure cards and nav in app YAML / Studio, not in `frontend/src/pages`.
