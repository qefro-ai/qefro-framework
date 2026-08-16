# List views

`EntityList` is the only collection page. List, Kanban, and Calendar are registered views on that page — never per-entity React routes. Columns come from `list.columns` or `list_visible` fields. Sorting, filters, and search are query parameters the API already understands (`eq`, `contains`, `between`, `in`, …). The client never sends SQL.

`views.list.group_by` (or `list.group_by`) groups the current page of permitted rows. Numeric/currency/percentage columns show a footer total. The first two columns can freeze; headers resize.

Deep links: `/{slug}?view=list|kanban|calendar`. Filters stay in the query string. Preferred view is stored with other table prefs (tenant + user + entity).

## Personalization

Per tenant + user + entity (local):

- column visibility
- page size
- default sort
- preferred view

## Filters

`FilterBar` translates UI operators and date presets (`today`, `last_7_days`, …) into `field.between=from,to`. Saved views call `GET/POST /api/v1/saved-filters` and include `view=` so a saved “Pending board” can reopen Kanban.

## Bulk and export

Row selection can export the current readable columns as CSV, or delete via `DELETE /api/v1/{slug}/{id}` (server still enforces permission). There is no client-side mutation of the database.

Empty, loading, and permission errors use the shared empty/skeleton/error states. Backend codes are mapped to human copy; SQL and stack traces are stripped.

See [views.md](views.md).


`EntityList` is the only list page. Columns come from `list.columns` or `list_visible` fields. Sorting, filters, and search are query parameters the API already understands (`eq`, `contains`, `between`, `in`, …). The client never sends SQL.

## Personalization

Per tenant + user + entity (local):

- column visibility
- page size
- default sort

## Filters

`FilterBar` translates UI operators and date presets (`today`, `last_7_days`, …) into `field.between=from,to`. Saved views call `GET/POST /api/v1/saved-filters`.

## Bulk and export

Row selection can export the current readable columns as CSV, or delete via `DELETE /api/v1/{slug}/{id}` (server still enforces permission). There is no client-side mutation of the database.

Empty, loading, and permission errors use the shared empty/skeleton/error states. Backend codes are mapped to human copy; SQL and stack traces are stripped.
