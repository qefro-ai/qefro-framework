# Document numbering

```rust
.naming(NamingConfig::new("INV-{YYYY}-{#####}").field("doc_no").assign_on("submit"))
```

Tokens:

- `{YYYY}` / `{YY}` year
- `{MM}` month
- `{#####}` zero-padded sequence (padding = number of `#`)

Sequences are per tenant, entity, and period (`YYYY` or `YYYY-MM`). Allocation is a single `INSERT … ON CONFLICT … RETURNING` statement, so concurrent requests cannot share a number.

`assign_on`:

- `create` — number on insert
- `submit` — number when the document leaves the initial workflow state via submit/confirm (not cancel)

The naming field (`doc_no` by default) is added automatically if missing, unique per tenant, and readonly in the UI.
