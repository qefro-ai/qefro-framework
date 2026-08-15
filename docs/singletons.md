# Singleton entities

A singleton is one document per tenant, not a collection of rows. Typical uses: restaurant settings, invoice settings, tax settings.

```rust
EntityDef::single("RestaurantSettings")
    .field(FieldDef::string("restaurant_name"))
    .field(FieldDef::string("timezone"))
    .build();
```

Storage uses the same entity table with a unique `(tenant_id)` constraint for live rows. `EntityService::create` rejects a second row. There is no parallel settings store.

## API

```http
GET  /api/v1/settings/{slug}
PATCH /api/v1/settings/{slug}
```

`GET` creates the row with defaults if none exists. Collection `POST` on a singleton returns 409/400. The same authentication → tenant → entitlement → RBAC → validation → audit pipeline applies.

## UI

The generic list page renders a settings form instead of a table. Studio labels the entity type as **Singleton**.
