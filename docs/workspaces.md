# Workspaces

The homepage is still `GET /api/v1/meta/dashboards` + `GET /api/v1/dashboards/{name}`. UI 2.0 does not add a custom React dashboard per app.

Card `kind` values (`metric`, `chart`, `list`, `activity`, …) render as KPI tiles, charts, or recent-record lists. Quick actions are “New {entity}” links for standalone entities in the current tenant’s enabled apps.

Restaurant, CRM, Inventory, and Helpdesk all use this workspace. Configure cards in app YAML / Studio, not in `frontend/src/pages`.
