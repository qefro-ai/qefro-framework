# QefroClient (browser SDK)

`QefroClient` is the browser SDK. It lives at `frontend/src/sdk/client.ts`. There is no published npm package.

```
UI / widgets / Studio  →  QefroClient  →  /api/v1  →  EntityService
Agents                 →  EntityOps    →  EntityService
```

`frontend/src/api.ts` re-exports the same `api` instance so existing imports keep working.

## Methods

Typed methods cover UI metadata, records, workflow, and uploads:

- `ui()` — `GET /api/v1/meta/ui`
- `list` / `get` / `create` / `update` / `remove`
- `action` / `transition` / `workflow` / `getWorkflow`
- `activity` / `getActivity` / `addComment`
- `attachments` / `getAttachments` / `uploadAttachment`
- `notifications` / `getNotifications`
- `audit` (Admin)
- `upload` / `uploadAttachment`

There is no `IdentityClient`, `WorkflowClient`, or `BusinessObjectClient`. Studio, RelationPicker, Kanban drag, and entity pages all call this client. Do not add a second UI API.

## Related SDKs

| Surface | SDK | Path |
| --- | --- | --- |
| Browser | `QefroClient` | REST `/api/v1` |
| Agents | `EntityOps` | in-process `EntityService` |

Same tenant, RBAC, validation, and workflow on every path.
