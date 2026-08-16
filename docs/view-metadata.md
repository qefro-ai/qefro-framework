# View metadata

Presentation-only. View YAML must not redefine permissions, workflow, validation, or business logic. `schema_version` remains `"1"`.

```yaml
views:
  list:
    group_by: status
    columns:
      - field: name
      - field: status
        widget: status
  form:
    sections:
      - title: Business Information
        visible_when:
          field: customer_type
          equals: business
  detail:
    sections:
      - title: Overview
        fields: [customer, date, status]
      - title: Financial
        fields: [subtotal, tax, total]
  kanban:
    group_by: status
    card:
      title: customer
      subtitle: reservation_time
      fields: [guests, reservation_date]
  calendar:
    start: reservation_datetime
    time: reservation_time   # when start is a date
    title: customer
    subtitle: status
```

`visible_when` / `readonly_when` remain `{ field, equals }` presentation hints. The backend is authoritative.

Field-level `visible_when` already exists on UI schema v1. Section `visible_when` is the same shape, applied by `FormLayout`.

If `views` is omitted, the renderer uses [automatic detection](views.md).
