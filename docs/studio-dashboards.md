# Studio dashboards

Dashboard Studio configures `DashboardDef` cards (`kpi`, `metric`, `table`, `chart`, `list`, `activity`, `workflow`, `saved_view`, `report`). Each card references an existing entity, report, or saved view — not a custom React component.

Studio can:

- add a widget
- reorder widgets
- set title, source entity, kind, size
- choose a saved report or view name

Publish writes a metadata overlay. Cards are evaluated with the same `dashboard_card_value` path as the business dashboard.

```
EntityDef / ReportDef
        ↓
Studio overlay
        ↓
Dashboard metadata
        ↓
Generic Dashboard renderer
```

Print format Studio edits `PrintFormat` (header, items table, totals, footer) and previews HTML through the existing print renderer. It is not a Canva-style designer.
