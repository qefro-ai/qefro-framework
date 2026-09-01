# UI components

The generic frontend is a small set of renderers. There is no `CustomerPage.tsx`.

| Area | Component | Role |
| --- | --- | --- |
| Shell | `AppShell` | Metadata nav, branding, search, notifications, user prefs |
| Workspace | `Dashboard` | KPI / chart / list cards from dashboard metadata |
| List | `EntityList` | Search, filters, columns, export, bulk delete |
| Form | `EntityForm` + `FormLayout` | Create/edit through the widget registry |
| Detail | `EntityDetail` | Document header, actions, related, timeline, attachments |
| Filters | `FilterBar` | Operators + date presets + saved views |
| Relations | `RelationPicker` | Search, pagination, optional quick create |
| Children | `ChildTable` | Inline edit, duplicate, totals |
| Actions | `ActionBar` | `_actions` then workflow transitions |
| Timeline | `Timeline` | Audit events the user is allowed to see |
| Files | `AttachmentsPanel` | Drag/drop; never shows storage keys |
| Palette | `CommandPalette` | Create / Go to / Search / Reports |
| Status | `StatusBadge` | Appearance from field metadata |

Extension point: `registerWidget(name, component)` in `@qefro/js` (`packages/qefro-js/src/metadata/registry.ts`). Applications can also `qefro.register({ field, entity, page, widget })` — see [qefro.js](qefro-js.md).
