# UI schema

`GET /api/v1/meta/ui` remains **`schema_version: "1"`**. UI 2.0 only adds optional fields. Older metadata still renders.

## Additive list metadata

```yaml
list:
  columns:
    - field: name
      width: 240
    - field: status
      widget: status
    - field: total
      widget: currency
  default_sort:
    field: created_at
    direction: desc
```

If `list.columns` is omitted, the renderer uses `list_visible` / `list` on each field (schema v1 behavior).

## Status appearance

```yaml
- name: status
  type: enum
  enum: [Pending, Confirmed, Cancelled]
  ui:
    widget: status
    widget_options:
      indicators:
        Pending: warning
        Confirmed: info
        Cancelled: danger
```

Do not hardcode those labels in React.

## Form / detail (unchanged keys)

`tab`, `section`, `width` (`full` | `half` | `third`), `widget`, `placeholder`, `help`, `visible_when`, `readonly_when` / `read_only_when`, `widget_options.collapsed`, `widget_options.allow_create`, and optional `views.form.sections` / `views.detail.sections` (title, tab, columns, fields, visible_when, collapsed) continue to drive the generic form and document views. Child tables use `widget_options.column_fields`. Related lists may set `columns`, `limit`, and `filters` on `LinkDef`.

User personalization (column order, page size, density, theme, preferred view) is stored in the browser, keyed by tenant + user + entity. It is not a second metadata registry.

## Views (UI 2.1, additive)

```yaml
views:
  list:
    group_by: status
  kanban:
    group_by: status
    card:
      title: guest_name
      subtitle: reservation_time
  calendar:
    start: reservation_date
    time: reservation_time
    title: guest_name
```

Omitted `views` uses automatic detection (workflow → Kanban, date/datetime → Calendar). See [views.md](views.md) and [view-metadata.md](view-metadata.md).

