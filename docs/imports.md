# CSV import

Generic import for any entity the user can create.

```
Upload CSV → detect columns → map fields → preview → validate → import → results
```

Preview writes nothing. Import calls `EntityService::create` per row (batches of 100 by default). Validation, RBAC, formulas, workflows, audit, and tenant isolation all apply. There is no `COPY` bypass.

```http
POST /api/v1/{slug}/import/preview
POST /api/v1/{slug}/import
```

Mapping: `ignore`, map column → field, or a default value. Failed batches are reported with row numbers. Partial success is explicit (`imported` / `failed` / `errors`), not silent.
