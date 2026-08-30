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

Navigation items may include a `section` heading. The generic sidebar groups those items. Workspace `shortcuts` (create, list, dashboard, reports, page) are derived from the same navigation, dashboards, reports, and pages — filtered by permission.

```rust
AppModule::new("restaurant")
    .nav(NavItem::page_link("Operations", "restaurant-operations").section("Operations"))
    .nav(NavItem::new("Orders", "Order").section("Operations"))
    .nav(NavItem::new("Kitchen", "Order").query("status=Preparing").view("kanban").section("Operations"))
    .nav(NavItem::new("Customers", "Customer").section("Catalog"))
    .dashboard(dashboard::ops())
    .page(pages::restaurant_operations())
```

A **page** (`PageDef`) is an operational workspace that embeds existing entity views, reports, widgets, and actions. A **dashboard** remains KPIs and charts. See [Pages](pages.md) and [Dashboards](dashboards.md).

The homepage still loads `GET /api/v1/dashboards/{name}`. Composed pages load `GET /api/v1/meta/pages/{name}` (metadata only) and then the existing Entity REST / dashboard / report APIs. Restaurant, CRM, Inventory, and Helpdesk all use this workspace. Configure cards, pages, and nav in app YAML / Studio, not in `frontend/src/pages`.
