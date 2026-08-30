# Formulas

Computed fields are declared in metadata and evaluated on the server. The browser may preview values; the server always recalculates and ignores client-supplied computed numbers.

```rust
.field(FieldDef::currency("amount").computed("quantity * unit_price"))
.field(FieldDef::currency("subtotal").computed("SUM(items.amount)"))
.field(FieldDef::currency("grand_total").computed("subtotal - discount"))
```

```json
{ "name": "amount", "type": "decimal", "computed": true, "formula": "quantity * rate" }
```

## Language

Restricted expressions only. No `eval`, no dynamic SQL, no arbitrary functions.

- Arithmetic: `+ - * / %` and parentheses
- Functions: `SUM MIN MAX COUNT ROUND CONCAT`
- Field references: `quantity`, `items.amount`
- String literals: `" "` and concatenation: `first_name + " " + last_name`
- `CONCAT(first_name, " ", last_name)` (same language, not a second engine)
- Aggregations: `SUM(items.amount)`, `COUNT(items)`

Unknown functions and leftover tokens (including SQL) are rejected at parse time.

Circular dependencies fail metadata validation:

```text
A = B + 1
B = A + 1  → error
```

Computed fields are stored after calculation so they can be filtered and reported. They are stripped from client payloads before validation.
