import type { UiWhen } from "./types";

export function valuesEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (left == null || right == null) return false;
  return String(left) === String(right);
}

export function matchesWhen(when: UiWhen | undefined, record: Record<string, unknown>): boolean {
  if (!when) return true;
  return valuesEqual(record[when.field], when.equals);
}

export function fieldVisible(
  field: { hidden?: boolean; visible_when?: UiWhen },
  record: Record<string, unknown>,
): boolean {
  if (field.hidden) return false;
  return matchesWhen(field.visible_when, record);
}

export function fieldRequired(
  field: { required?: boolean; required_when?: UiWhen },
  record: Record<string, unknown>,
): boolean {
  if (field.required) return true;
  if (!field.required_when) return false;
  return matchesWhen(field.required_when, record);
}

export function fieldReadonly(
  field: { readonly?: boolean; readonly_when?: UiWhen; read_only_when?: UiWhen; disabled?: boolean },
  record: Record<string, unknown>,
): boolean {
  if (field.readonly || field.disabled) return true;
  const when = field.readonly_when || field.read_only_when;
  if (!when) return false;
  return matchesWhen(when, record);
}
