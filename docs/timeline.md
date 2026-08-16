# Timeline

Activity on a document is `GET /api/v1/audit?entity=&entity_id=`. The generic `Timeline` maps audit rows to presentation kinds (`created`, `updated`, workflow, attachment, …) for styling only. It does not invent events.

Comments as a first-class thread would need a new backend primitive. UI 2.0 does not add that; keep comments as a future extension on the same activity surface.

Only rows the API returns are shown. Field permissions and tenant scope are enforced server-side.
