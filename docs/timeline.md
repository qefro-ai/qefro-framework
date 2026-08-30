# Timeline

Activity on a document is `GET /api/v1/{slug}/{id}/activity`. The generic `Timeline` groups rows by day (Today / Yesterday) and maps `activity_type` to presentation kinds (`created`, `updated`, `workflow`, `comment`, `attachment`, …) for styling only. It does not invent events.

Comments are Activity records (`POST /api/v1/{slug}/{id}/comments`). There is no separate messaging product.

Audit history is **not** this timeline. Administrators use `GET /api/v1/audit` / `/settings/audit`. See [Activity](activity.md) and [Audit](audit.md).

Only rows the API returns are shown. Field permissions and tenant scope are enforced server-side.

