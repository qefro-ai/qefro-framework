# Studio reports

Report Studio edits `ReportDef`: entity, fields, group by, aggregations, sort/chart. Execution is `EntityService::run_report` / `qefro-search` filters. There is no second query engine and no arbitrary SQL.

Unknown entity or field names are rejected at validate/publish. After publish, `/api/v1/meta/reports` and the generic Reports page see the overlay.
