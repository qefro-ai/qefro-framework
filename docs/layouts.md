# Layouts

Layout comes from field metadata, not custom pages.

| Metadata | Effect |
| --- | --- |
| `section` | Fieldset / detail group heading |
| `tab` | Declarative tab on form and detail |
| `order` | Sort order |
| `width` | `full` (default), `half`, `third` |
| `list` / `list_visible` | Column on the generic list |
| `form` / `form_visible` | Shown on create/edit |
| `detail` / `detail_visible` | Shown on the record page |
| `visible_when` | Presentation-only show/hide |
| `readonly_when` | Presentation-only lock (`read_only_when` alias) |
| `views.form.sections` / `views.detail.sections` | Shared layout: section, columns, tab, fieldset grouping |

Form and detail share the same section language. Omit `views.form` to keep the generic field.section grouping (schema v1).

```yaml
form:
  layout: # stored as views.form.sections
    - title: Customer Information
      columns:
        - fields: [name, email, phone]
        - fields: [party_type, person_id]
    - title: Address
      tab: Address
      fields: [address, city, country]
```

```yaml
- name: cancellation_reason
  type: text
  ui:
    section: Additional Information
    visible_when:
      field: status
      equals: Cancelled
```

UI schema is versioned:

```json
{ "schema_version": "1", "entity": "Reservation", "fields": [] }
```

Lists support search, type-aware filters, sort, pagination, empty/loading/error states, row links, and bulk selection. Saved filters are scoped to `tenant + user + entity` via `/api/v1/saved-filters`.
