import type { UiEntity, UiField, ViewKind } from "./types";

const SYSTEM_DATES = new Set(["created_at", "updated_at", "deleted_at"]);

export function groupingField(entity: UiEntity): UiField | undefined {
  const named = entity.views?.kanban?.group_by;
  if (named) return entity.fields.find((f) => f.name === named);
  if (entity.workflow) {
    return (
      entity.fields.find((f) => f.name === "status") ||
      entity.fields.find((f) => f.widget === "status")
    );
  }
  return entity.fields.find(
    (f) => f.widget === "status" || (f.type === "enum" && f.name === "status"),
  );
}

export function calendarStartField(entity: UiEntity): UiField | undefined {
  const named = entity.views?.calendar?.start;
  if (named) return entity.fields.find((f) => f.name === named);
  return entity.fields.find(
    (f) =>
      (f.type === "datetime" || f.type === "date" || f.widget === "datetime" || f.widget === "date") &&
      !SYSTEM_DATES.has(f.name),
  );
}

export function calendarTimeField(entity: UiEntity): UiField | undefined {
  const named = entity.views?.calendar?.time;
  if (named) return entity.fields.find((f) => f.name === named);
  const start = calendarStartField(entity);
  if (start?.type === "date") {
    return entity.fields.find((f) => f.type === "time" || f.widget === "time");
  }
  return undefined;
}

export function calendarEndField(entity: UiEntity): UiField | undefined {
  const named = entity.views?.calendar?.end;
  if (named) return entity.fields.find((f) => f.name === named);
  return undefined;
}

export function kanbanEnabled(entity: UiEntity): boolean {
  if (entity.views?.kanban?.enabled === false) return false;
  if (entity.views?.kanban?.group_by) return Boolean(groupingField(entity));
  if (entity.views?.kanban) return Boolean(groupingField(entity));
  return Boolean(entity.workflow && groupingField(entity));
}

export function calendarEnabled(entity: UiEntity): boolean {
  if (entity.views?.calendar?.enabled === false) return false;
  if (entity.views?.calendar?.start) return true;
  return Boolean(calendarStartField(entity));
}

export function availableViews(entity: UiEntity): ViewKind[] {
  const views: ViewKind[] = ["list"];
  if (kanbanEnabled(entity)) views.push("kanban");
  if (calendarEnabled(entity)) views.push("calendar");
  return views;
}

export function listGroupField(entity: UiEntity): string | undefined {
  return entity.views?.list?.group_by || entity.list?.group_by;
}

export function isWorkflowGroup(entity: UiEntity, fieldName: string): boolean {
  return Boolean(entity.workflow) && (fieldName === "status" || fieldName === groupingField(entity)?.name);
}

export function displayValue(row: Record<string, unknown>, field?: string): string {
  if (!field) return String(row.name ?? row.title ?? row.code ?? row.id ?? "");
  const expanded = row._expanded as Record<string, { label?: string }> | undefined;
  if (expanded?.[field]?.label) return String(expanded[field].label);
  const value = row[field];
  if (value == null || value === "") return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}
