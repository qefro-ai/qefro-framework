# Views

Qefro UI 2.1 turns the generic renderer into a **Business Views Engine**. The same entity can be shown as List, Cards, Kanban, Calendar, Form, or Detail without a per-entity React page.

```
EntityDef → UI Schema v1 → View Registry → List | Card | Kanban | Calendar | Form | Detail → QefroClient → EntityService
```

`schema_version` stays `"1"`. View configuration is optional. Entities without `views:` keep working from automatic defaults.

## Automatic detection

| View | Shown when |
| --- | --- |
| List | Always |
| Cards | Only when `views.card` is present and `enabled !== false` |
| Kanban | `views.kanban` is set, or the entity has a **workflow** and a grouping field (`status` / enum) |
| Calendar | `views.calendar` is set, or the entity has a non-system date/datetime field |

Set `views.kanban.enabled: false` or `views.calendar.enabled: false` to hide a detected view. A status enum **without** a workflow (for example restaurant tables) stays list-only. **Do not force Card** — omitted `views.card` means no Cards tab.

Form and Detail are routes (`/:slug/new`, `/:slug/:id`, `/:slug/:id/edit`), not collection-switcher tabs.

## View registry

```ts
import { registerView } from "./views/registry";
registerView("card", CardView);
registerView("kanban", KanbanView);
registerView("calendar", CalendarView);
```

Custom views register the same way widgets do. Qefro core does not need to change.

## Selector, URL, preferences

On a collection page the renderer shows `[List] [Cards] [Kanban] [Calendar]` for valid views only. Default remains **list**. Selection is stored in:

- URL: `/{slug}?view=card` (filters, search, and sort stay in the query string)
- Preferences: tenant + user + entity (`TablePrefs.view`)

Saved filters already persist the current query, including `view=`. There is no second saved-view system.

## Permissions

Views only hide unavailable UI. Tenant, app entitlement, RBAC, field permissions, workflow, and record visibility stay on EntityService. `permissions` on `/meta/ui` and `_permissions` on GET are chrome hints. Kanban never PATCHes a workflow status field. Calendar reschedule uses `PATCH` on the datetime field and still fails closed on the server.

See [view metadata](view-metadata.md), [Kanban](kanban.md), [Calendar](calendar.md), and [sdk.md](sdk.md).
