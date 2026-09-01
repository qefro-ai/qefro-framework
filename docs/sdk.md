# QefroClient (browser SDK)

`QefroClient` is the browser SDK. It lives in `@qefro/js` (`packages/qefro-js/src/sdk/client.ts`). The generic UI in `frontend/` imports it from `@qefro/js`. Studio still uses `frontend/src/api.ts`, which re-exports the same `api` instance.

```
UI / widgets / Studio  →  QefroClient  →  /api/v1  →  EntityService
Agents                 →  EntityOps    →  EntityService
```

`@qefro/js` also exports the `Qefro` UI runtime (`qefro.ui.list`, extensions, theme). See [qefro.js](qefro-js.md). Do not add a second UI HTTP client.

## Methods

Typed methods cover UI metadata, records, workflow, search, reports, dashboards, and uploads:

- `ui()` — `GET /api/v1/meta/ui`
- `getSearch()` / `search()`
- `getSavedViews()` / `saveView()` / `deleteView()`
- `getReport()` / `runReport()`
- `getDashboard()` / `workspace()` / `aggregates()`
- `list` / `get` / `create` / `update` / `remove`
- `action` / `transition` / `workflow` / `getWorkflow`
- `activity` / `getActivity` / `addComment`
- `attachments` / `getAttachments` / `uploadAttachment` / `files.list` / `files.upload` / `files.download` / `files.delete`
- `notifications` / `getNotifications`
- `audit` (Admin)
- `importPreview` / `importRun` / `importUpload` / `importJobs` / `importJob` / `cancelImport` / `retryImport`

There is no `IdentityClient`, `WorkflowClient`, `ReportClient`, `DashboardClient`, or `SearchClient`. Studio, RelationPicker, Kanban drag, and entity pages all call this client. Do not add a second UI API.

## Related SDKs

| Surface | SDK | Path |
| --- | --- | --- |
| Browser | `QefroClient` / `@qefro/js` | REST `/api/v1` |
| Agents | `EntityOps` | in-process `EntityService` |

Same tenant, RBAC, validation, and workflow on every path. 422 responses throw `ValidationError` (subclass of `ApiError`) with `fields: [{ field, code, message }]`.
