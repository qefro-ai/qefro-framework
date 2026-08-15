# Child tables

A parent entity can own nested child rows. One metadata definition produces schema, nested REST, validation, and a generic editable table — not a custom React page.

```rust
EntityDef::new("Order")
    .field(FieldDef::relation("customer_id", "Customer").required())
    .child_table(ChildTableDef::new("items", "OrderItem").parent_field("order_id"))
    .build();

EntityDef::new("OrderItem")
    .field(FieldDef::many_to_one("order_id", "Order").required().hidden())
    .field(FieldDef::relation("menu_item_id", "MenuItem").required())
    .field(FieldDef::integer("quantity").required().min(1.0))
    .child_of("Order", "items")
    .build();
```

`child_of` hides the child from navigation unless `.standalone()` is set. The child still has a table, REST, and RBAC.

## Database

Child rows store `tenant_id` and a foreign key to the parent (`parent_id` by default, or `order_id` when configured). The parent FK uses `ON DELETE CASCADE`. An index is created on `(tenant_id, parent_fk)`.

Schema apply never silently drops columns. If an earlier metadata version created `parent_id NOT NULL` and the child later used `invoice_id` instead, auto-migrate relaxes leftover and computed columns so nested inserts are not blocked. New tables omit unused columns.

## Nested CRUD

Create/update accept the child array on the parent:

```json
{
  "customer_id": "...",
  "items": [
    { "menu_item_id": "...", "quantity": 2, "unit_price": 300 }
  ]
}
```

Parent and children write in one transaction. A failing child row rolls the parent back.

Updates sync by child `id`:

- no `id` → insert
- existing `id` on this parent → update
- omitted existing row → delete
- `id` from another parent or tenant → rejected

Do not send another tenant's child id. Relation pickers and GET are tenant-scoped.

Child rows keep insertion order via a hidden `sort_order` column. Move up/down in the form updates that order on save.

## Validation

Child field errors are nested:

```json
{ "field": "items.0.quantity", "message": "Quantity is required" }
```

## UI

The generic form renders a `child_table` widget (add, delete, duplicate, reorder). Row operations still submit through the parent PATCH and server validation.
