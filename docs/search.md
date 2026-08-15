# Global search

```http
GET /api/v1/search?q=Ahmed
```

PostgreSQL `ILIKE` over fields marked `searchable: true`. Modes: exact, prefix, contains. Elasticsearch is not required.

Each entity is skipped unless the caller has app entitlement and `list` permission. Hits are presented through `EntityService` so field permissions strip sensitive values from snippets. Records the user cannot read do not appear.

The generic command palette (`⌘K`) calls this endpoint. Studio keeps a separate metadata search.
