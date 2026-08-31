# Business rules

Business rules live on `EntityDef` / `FieldDef`. There is one evaluator: `Condition` + `ValidationRules` + `ValidationRule` + the existing formula AST. Do not add a second rule engine, workflow engine, or metadata system.

```
             Metadata
                 │
        ┌────────┼────────┐
        ▼        ▼        ▼
       UI       REST      SDK
        │        │        │
        └────────┼────────┘
                 ▼
            EntityService
                 │
        ┌────────┼────────┐
        ▼        ▼        ▼
     Workflow Automation Operations
```

Frontend checks are for UX. `EntityService` is authoritative. Hidden UI fields are still validated. A client cannot bypass required, validation, readonly, or computed by calling REST directly.

`UI_SCHEMA_VERSION` stays `"1"`. Rules are additive.

## Rule types

| Type | Metadata | Server | UI |
|---|---|---|---|
| `required` | `FieldDef::required()` | `validate_record` | required marker |
| `required_when` | `FieldDef::required_when(field, equals)` | `apply_field_rules` | required marker when the condition matches |
| `validation` | `FieldDef::min` / `max` / `greater_than` / `email` / … | `validate_record` | widget min/max, error text |
| cross-field | `ValidationRule::compare` | `apply_entity_rules` | field error on the left-hand field |
| `default` | `default_value` / `default_from` | `prepare_record` | preview |
| `readonly` / `readonly_when` | `FieldDef::readonly()` / `readonly_when` | `reject_readonly_writes` | disabled control |
| `visible_when` | `FieldDef::visible_when` | still validates submitted values | show/hide |
| `computed` | `FieldDef::computed(formula)` | `apply_computed_fields` | calculated, not editable |

Conditional require also exists as an entity rule (`when` + `require`). Use that for multi-field require lists; use `required_when` when the requirement hangs off one field.

## Expressions

Reuse `Condition` (automation, workflow guards, `WhenClause`) and the formula AST (computed fields).

Operators: `=` `!=` `>` `>=` `<` `<=` `AND` `OR` `NOT` `IN` `IS NULL` / `IS NOT NULL` (`is_empty` / `is_not_empty`).

Dotted paths read nested objects and one hop of `_expanded` (for example `customer.party_type`). Array `.length` is supported (`items.length > 0`). This is not arbitrary graph traversal and is not SQL.

Formulas stay the restricted AST: `+ - * / %`, `SUM MIN MAX COUNT ROUND CONCAT`, field refs, string concat. No `eval`, SQL, Rust, JavaScript, Python, or shell. Unknown functions fail at parse time. Circular computed fields fail metadata validation.

`today()` / `now()` are not added; defaults use `default_from("current_date")` / `current_datetime`.

Type mismatches (`integer` compared with `string`) fail metadata validation (`qefro validate`) and, if they reach runtime, return `invalid_type`.

## Execution order

`EntityService` create/update (unchanged besides readonly + `required_when`):

1. Load metadata, RBAC, tenant
2. Strip computed client overrides
3. Apply defaults (`default` / `default_from` / workflow initial)
4. Normalize / sanitize
5. `validate_record` (types, required, per-field rules, `required_when` on create)
6. Merge (update) then `apply_entity_rules` + `required_when` on the merged record
7. Relation existence, uniques, field permissions
8. Transaction, recompute formulas, audit, outbox, commit

Operations call the same `validate_record` / `apply_entity_rules` / `reject_readonly_writes` on `OperationCtx`. They cannot skip EntityService rules.

## Validation errors

HTTP **422** `validation_failed`. Existing envelope:

```json
{
  "error": "validation_failed",
  "message": "validation failed",
  "fields": [
    {
      "field": "end_date",
      "code": "invalid_range",
      "message": "End date must be after start date.",
      "rule": "compare"
    }
  ]
}
```

`field` / `code` / `message` stay required. `entity`, `record`, and `rule` are optional. Child rows use `items.0.quantity`.

The SDK throws `ValidationError` (a `ApiError`) for 422, with `fields[].field` and `fields[].code`.

## Defaults

Applied server-side: `default_value(json!(…))` and `default_from("current_user" | "current_date" | "current_datetime" | "tenant_timezone" | "tenant_currency")`. The form may preview them. Do not rely on browser defaults.

## Computed values

```rust
.field(FieldDef::currency("amount").computed("quantity * unit_price"))
.field(FieldDef::string("full_name").computed(r#"first_name + " " + last_name"#))
```

Deterministic, typed, cycle-resistant. Client-supplied computed values are discarded. The generic UI marks them calculated and disabled. See [formulas.md](formulas.md).

## Visibility

`visible_when` is presentation. The same `UiWhen` metadata drives the form. The backend still validates whatever is submitted. Hidden is not trusted.

## Readonly

`readonly_when` is now server-authoritative on update. Identical values are allowed; mutations return `code: readonly`. Document lock states continue to apply where configured.

## Workflow guards

Continue to use `Condition` / `TransitionDef::requires` / `TransitionDef::guard`. Do not invent a second syntax.

```rust
TransitionDef::new("confirm", "Draft", "Confirmed")
    .requires(&["items"])
```

Confirm Order still checks menu availability in the operation handler. The guard is the generic “items must exist” rule. Child tables are attached to the record before the guard runs (one query per child table, on the transition/operation path only — not on list).

## Automation

Automation `conditions:` is the same `Condition` type.

## Permissions

Rules never encode role names. Field writes still go through `PermissionRegistry` / `permission_level`. Admins are not a special case inside the evaluator.

## Tenant and query safety

Relation lookups use existing tenant-scoped `EntityRepository` queries. Expressions never become SQL. Cross-tenant ids fail existence checks the same way as other relations.

## Metadata validation

`qefro validate` / `EntityDef::validate_rules` reject:

- unknown field or relation in `required_when`, `visible_when`, `readonly_when`, `compare`, `require`
- unknown operator
- incomparable field types
- invalid formula
- circular computed fields

## Inspect

```text
qefro inspect Order

Rules
  quantity
    required
    validation: >= 1
  amount
    computed: quantity * unit_price
  discount
    readonly when status = Completed
    default: 0
  customer_id
    readonly when status = Completed
  Validation
    end_time > reservation_time
```

## Studio

Field editor: required, required when, readonly when, visible when, greater than, minimum. Not a visual programming language. Changes stay YAML/JSON-friendly overlays.

## Examples

Restaurant: Order Item `quantity >= 1`, Reservation `party_size >= 1`, Reservation `end_time > reservation_time`, Order confirm requires `items`, Order `discount` / `customer_id` readonly when `status = Completed`.

CRM: Lead email required when `contact_method = email`, Opportunity line `rate >= 0`, Task `due_at >= created_at`.

Entities with no rules behave exactly as before.
