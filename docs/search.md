# Global search

```http
GET /api/v1/search?q=Ahmed
```

PostgreSQL `ILIKE` over fields marked `searchable: true`. Secret fields are never searched or returned. Modes: exact (`search_exact` or quoted query), prefix (`Ahmed*`), contains. Elasticsearch is not required.

Each entity is skipped unless the caller has app entitlement and `list` permission. Hits are presented through `EntityService` so field permissions strip sensitive values from snippets. Records the user cannot read do not appear. Attachment filename and description are searched the same way (not binary contents) and grouped under **Attachments**. See [Files](files.md).

Response:

```json
{
  "results": [{ "entity": "Customer", "slug": "customers", "id": "...", "label": "Ahmed Khan", "snippet": "Ahmed Khan", "score": 80 }],
  "groups": [{ "entity": "Customer", "label": "Customers", "hits": [ "..." ] }]
}
```

Ranking uses `search_weight` (default 1), exact/prefix/contains quality, and the entity `display_field`. Relation labels already present on the record (`_expanded`) contribute to the score.

## Entity search

List queries (`GET /api/v1/{slug}?search=`) reuse the same searchable metadata. `search_exact` matches the whole value; other searchable fields use `ILIKE`.

```rust
FieldDef::string("name").search_weight(10)
FieldDef::string("code").search_exact()
```

The generic command palette (`⌘K`) calls `QefroClient.getSearch()` / `search()`. Recent searches are stored locally. Studio keeps a separate metadata search.

Agents use `EntityOps::search` (`search` tool) — the same `EntityService` path.
