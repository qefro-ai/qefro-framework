# Detail views

`EntityDetail` is the generic document screen.

Header (from the record + metadata):

- document number (`naming.field`, then `name` / `code`)
- status (workflow current or `status` field)
- owner / created (when present)
- primary **Edit** plus `_actions` (fallback: workflow transitions)
- More: print, PDF, delete

Tabs are generic: **Details**, each child table, **Related records** (`_links` / `_related`), **Attachments**, **Activity**. Sections render only when the matching capability or data exists.

Related links open the generic list with a filter on the relation field. The frontend does not join tables.

Print / PDF use the existing `/api/v1/{slug}/{id}/print` endpoints when the user may access them.

Realtime SSE refreshes the record on `record.updated` / workflow events. The connection reconnects with backoff; the shell shows a live-status dot.
