# Saved views

Users save the current list combination of filters, sort, visible columns, view type, and search:

```http
GET  /api/v1/saved-views?entity=Customer
POST /api/v1/saved-views
DELETE /api/v1/saved-views/{id}
```

`/api/v1/saved-filters` remains as an alias. Rows are tenant-scoped **and** user-scoped. Creating or listing a view requires `List` on the entity. A user cannot reopen another user's view or an entity they cannot list.

`QefroClient.getSavedViews()` / `saveView()` / `deleteView()` wrap the same endpoints. The FilterBar labeled **Saved views** calls these methods.

Default presentation still comes from `EntityDef.views` (not a second schema):

```yaml
views:
  default: list
  list: { columns: [...], default_sort: { field: created_at, direction: desc } }
  kanban: { group_by: status }
  chart:
    type: bar
    dimension: status
    measure: { field: amount, aggregation: sum }
```

`UI_SCHEMA_VERSION` stays `"1"`.
