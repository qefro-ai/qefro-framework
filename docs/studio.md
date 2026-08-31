# Qefro Studio

Qefro Studio is the developer/admin console for application metadata. It is not a visual no-code builder.

```
Developer Source → YAML / Rust / app.toml → Validation → Normalized Metadata → Runtime Registry → Studio
```

Studio never creates a second metadata system. It reads the same `EntityRegistry`, workflow registry, permission registry, reports, dashboards, and print formats the runtime already uses. `EntityService`, RBAC, tenant isolation, and audit stay on the path.

## Who can open Studio

Capabilities are explicit. Being a tenant Admin does not grant every Studio action in production.

| Capability | Typical roles |
| --- | --- |
| `studio.view` | Admin, StudioViewer, PlatformAdmin |
| `studio.edit` | Admin, StudioEditor, PlatformAdmin |
| `studio.publish` | StudioPublisher, PlatformAdmin; Admin **only in development** |
| `studio.manage_apps` | StudioAppManager, PlatformAdmin; Admin **only in development** |
| `studio.manage_permissions` | Admin, StudioPermissionManager, PlatformAdmin |
| `studio.manage_workflows` | Admin, StudioWorkflowManager, PlatformAdmin |

Staff and Customer have no Studio access.

## Development vs production

**Development** (`QEFRO_ENV=development`, the default): Admin can draft, validate, and publish overlays, including additive `ADD COLUMN` after confirmation.

**Production**: Admin can inspect and draft, and can change **tenant** configuration (branding, navigation, terminology, locale). Publishing application metadata and managing installed apps requires `StudioPublisher` / `PlatformAdmin`. Additive schema changes also require `confirm_migration=true`. Destructive changes (type change, field delete, entity rename) are rejected; Studio will not drop columns or convert data.

## Source vs runtime

| App kind | Studio can |
| --- | --- |
| YAML (`entities/*.yaml`) | Inspect, overlay, and write the changed entity file after validation. Comments in that file may be rewritten. Unrelated files are left alone. |
| Rust (`examples/…`, catalog) | Inspect. Overlay safe presentation and additive fields at runtime. **Does not rewrite** `OperationHandler` or other Rust. |

Live reload uses an overlay on the existing registries. `GET /api/v1/meta/ui` and `EntityService` both see the overlay. There is no second frontend renderer: form preview reuses `FormLayout` / the widget registry, and the Views tab reuses the production view registry (`ListView`, `KanbanView`, `CalendarView`). Page Studio composes those same components from existing entities, views, reports, and dashboard widgets — custom JavaScript, HTML, and SQL are rejected. See [Pages](pages.md).

Studio inspects V0.9 primitives on the same entity page: singleton flag, field permission levels, allow-on-submit, actions, links, public forms, plus dedicated Notifications / Webhooks / Public Forms / Automations lists. Secrets are never shown. Field permission_level and allow_on_submit publish through the existing `entity.field.ui` change path.

## Platform vs tenant

**Platform Studio** (apps, versions, entities, workflows, permissions, reports): shared application metadata.

**Tenant Studio** (`/studio/system` and existing `/api/v1/tenant*`): branding, navigation, terminology, enabled apps, locale, currency, timezone. Tenant A cannot read or write Tenant B’s drafts, branding, or data.

The entity page includes Fields, Form/layout, **Views** (List / Kanban / Calendar / Detail detection plus production preview), Workflow, and Permissions. There is no second preview engine.

## HTTP

All routes require a Bearer token and Studio capabilities. Prefix: `/api/v1/studio`.

- `GET /overview`, `/apps`, `/apps/:app`, `/entities`, `/entities/:entity`
- `GET /workflows/:entity`, `/permissions/:entity`, `/operations/:entity`
- `GET /notifications`, `/webhooks`, `/public-forms`
- `GET /reports`, `/dashboards`, `/pages`, `/print-formats`
- `GET /search?q=`
- `POST /drafts`, `/validate`, `/publish`
- `GET /versions`, `POST /rollback`
- `POST /formula/preview`

See [studio-publishing.md](studio-publishing.md) for the draft → validate → preview → publish flow.
