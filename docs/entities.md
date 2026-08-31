# Entities

An entity is the unit of metadata. One definition produces schema, REST, UI, validation, audit, and tools.

## Builder

```rust
EntityDef::new("Customer")
    .field(FieldDef::string("name").required().searchable())
    .field(FieldDef::string("email").required().email().unique())
    .field(FieldDef::many_to_one("branch_id", "Branch").required())
    .field(FieldDef::one_to_many("reservations", "Reservation", "customer_id"))
    .build()
```

YAML:

```yaml
name: Customer
fields:
  - name: name
    type: string
    required: true
    searchable: true
```

`qefro entity create Customer` writes that YAML into `entities/` (or `apps/<app>/entities` with `--app`).

## Relationships

| Kind | Storage | UI |
| --- | --- | --- |
| many-to-one | UUID column | relation picker |
| one-to-many | none (inverse filter) | related list on detail |
| child table | child table + parent FK | nested editable table on the parent form |
| many-to-many | junction `{table}_{field}` | relation widget |

List/get expand many-to-one in `_expanded` with a batched `IN` query. Expansions nest one further many-to-one hop so Customer → Person → User can render from existing relation widgets. Inverse one-to-many collections appear in `_related` / `_links`. A business entity that uses the `person_id` convention is listed automatically on Person detail.

When `person_id` is set, Person is the source of truth for name/email/phone. The business entity still stores its own fields for unlinked and legacy rows — do not drop Customer name/email/phone. See [identity](identity.md).

## UI field flags

`label`, `description`, `placeholder`, `help` / `help_text`, `hidden`, `disabled`, `readonly`, `required`, `list_visible` / `list`, `form_visible` / `form`, `detail_visible` / `detail`, `searchable`, `sortable`, `filterable`, `width`, `widget`, `widget_options`, `section`, `tab`, `order`, `visible_when`, `readonly_when`, `permission_level`, `allow_on_submit`.

`EntityDef::single("RestaurantSettings")` marks a singleton (one row per tenant). `.attachments()`, `.action()`, `.link()`, and `.public_form()` add V0.9 primitives without a second registry. See [singletons](singletons.md), [files](files.md), and [field permissions](field-permissions.md). Framework Person and User are also `EntityDef`s — see [identity](identity.md). `.with_tasks()` adds a Related-panel link to the platform Task entity — see [tasks](tasks.md).

Data types: `string`, `text`, `integer`, `decimal`, `boolean`, `date`, `time`, `datetime`, `uuid`, `enum`, `json`, `relation`, `child_table`. Convenience builders `email()`, `phone()`, `url()`, `color()`, `FieldDef::currency("amount")`, `percentage()` keep the storage type and set validation + widget. `.computed("quantity * rate")` marks a server-calculated field.

```rust
FieldDef::date("reservation_date").required().ui(UiConfig::date())
FieldDef::datetime("appointment_at").ui(UiConfig::datetime().tenant_timezone())
FieldDef::currency("price")
FieldDef::string("brand_color").color()
FieldDef::relation("customer", "Customer").required()
.child_table(ChildTableDef::new("items", "OrderItem"))
```

## Inspect

```bash
qefro entity list
qefro entity show Reservation
```

If Reservation points at an unknown entity, startup fails with a suggestion:

```
Entity 'Reservation' references unknown entity 'Table'. Did you mean 'DiningTable'?
```
