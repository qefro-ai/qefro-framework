# Studio entities

The entity inspector is a view of `EntityDef` from the runtime registry (including Studio overlays).

It shows fields, relations, child tables, computed formulas, form/list/detail layout, and a preview that uses the same `FormLayout` widgets as the business UI. Overlays do not change RBAC, tenant isolation, or field types.

## Safe edits (no migration)

- label, description, help, placeholder
- required / readonly / hidden
- searchable / sortable / filterable
- widget and widget options (currency code, precision, timezone, …)
- section, tab, width, order
- `entity.views` overlay: list columns, card, kanban group/card, form/detail sections (presentation only; permissions, workflow, and field types are rejected)

The **Layout** tab publishes field order / section / tab / label via `entity.field.ui`. The **Views** tab publishes `entity.views` and previews with the production view registry (including Cards). There is no page builder.

The **custom fields** tab publishes `entity.custom_field`. That is **Safe** (JSONB bag, no `ADD COLUMN`). See [custom fields](custom-fields.md).

## Additive edits (migration required)

Adding a stored field publishes an overlay and runs the existing `apply_schema` path (`ADD COLUMN IF NOT EXISTS`). In production, `confirm_migration` must be true.

## Rejected in V0.8

- Changing a field type (`string` → `relation`, `decimal` → `datetime`)
- Deleting a field
- Renaming an entity or column
- Pointing a relation at an entity that is not in the registry

Studio reports `⚠ Database migration required` and does not drop or convert columns.

## Formulas

The formula editor uses the same restricted language as the runtime (`SUM MIN MAX COUNT ROUND + - * / % ()`). Preview is metadata-only; persisted computed values are still calculated in `EntityService`.

## Relations and child tables

Relation target, display field, and search fields are validated before save. Child table inspector surfaces editable / add / delete flags from widget options. Opening the child entity uses the same inspector.
