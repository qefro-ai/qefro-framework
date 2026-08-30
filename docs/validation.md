# Validation

Authoritative validation runs on the server in `EntityService` (`validate_record` + entity-level rules). The generic UI may mirror rules for UX. REST, SDK, and automation all share the same checks.

Do not replace per-field `ValidationRules`. Entity-level `validation:` is additive.

## Field rules

```rust
FieldDef::string("email").required().email().unique()
FieldDef::integer("quantity").greater_than(0.0)
FieldDef::integer("qty").min(1.0).max(10.0) // greater_or_equal / less_or_equal
FieldDef::integer("score").range(0.0, 100.0)
```

| Rule | Semantics |
|---|---|
| `required` | Non-empty on create |
| `unique` | Tenant-scoped uniqueness (database layer) |
| `min` / `greater_or_equal` | Inclusive lower bound |
| `max` / `less_or_equal` | Inclusive upper bound |
| `greater_than` | Exclusive lower bound |
| `less_than` | Exclusive upper bound |
| `range` | Inclusive min and max |
| `min_length` / `max_length` | Character counts |
| `regex` | Pattern |
| `email` / `phone` / `url` | Format |

`UiWhen` remains presentation-only. Hidden fields are still validated.

## Entity-level YAML

```yaml
validation:
  - field: email
    rule: email

  - field: quantity
    rule: greater_than
    value: 0

  - when:
      field: status
      equals: confirmed
    require:
      - customer_id

  - compare:
      field: end_date
      greater_than: start_date

  - field: customer_id
    rule: exists
```

```rust
EntityDef::new("Reservation")
    .validation_rule(ValidationRule {
        when: Some(WhenClause {
            field: "status".into(),
            equals: Some(json!("confirmed")),
            not_equals: None,
        }),
        require: vec!["customer_id".into()],
        ..Default::default()
    })
```

Conditional `when` / `require` and `compare` are evaluated on the server against the merged record. There is no executable expression language.

## Relation existence

`rule: exists` looks up the related row with the same `tenant_id`. It never scans other tenants. Missing related rows return `validation_failed` with field code `exists`.

## Errors

HTTP 422, error code `validation_failed`, with `{ "fields": [{ "field", "code", "message" }] }`. Stack traces are never returned.
