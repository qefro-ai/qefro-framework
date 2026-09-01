import type { UiField, UiWhen, ViewSection } from "./types";
import { fieldVisible, matchesWhen } from "./conditions";

export type LayoutColumn = { fields: UiField[] };

export type LayoutSection = {
  title: string;
  tab: string;
  columns: LayoutColumn[];
  visible_when?: UiWhen;
  collapsed?: boolean;
};

export type ResolvedLayout = {
  tabs: string[];
  sections: LayoutSection[];
};

/** System / inverse fields the generic form does not place. */
export function isFormField(field: UiField): boolean {
  return field.relation_kind !== "one_to_many";
}

export function layoutFieldMap(fields: UiField[]): Map<string, UiField> {
  return new Map(fields.filter(isFormField).map((f) => [f.name, f]));
}

function unique(items: string[]): string[] {
  return [...new Set(items.filter(Boolean))];
}

function columnFromNames(names: string[], byName: Map<string, UiField>, used: Set<string>): LayoutColumn {
  const fields: UiField[] = [];
  for (const name of names) {
    const field = byName.get(name);
    if (!field || used.has(name)) continue;
    used.add(name);
    fields.push(field);
  }
  return { fields };
}

/**
 * Prefer explicit form/detail section metadata. Fall back to field.section / field.tab
 * so entities without layout metadata keep the current generic grouping.
 */
export function resolveLayout(
  fields: UiField[],
  spec: ViewSection[] | undefined,
  values: Record<string, unknown>,
): ResolvedLayout {
  const byName = layoutFieldMap(fields);
  const used = new Set<string>();
  const sections: LayoutSection[] = [];

  if (spec && spec.length > 0) {
    for (const section of spec) {
      if (section.visible_when && !matchesWhen(section.visible_when, values)) continue;
      const columns: LayoutColumn[] =
        section.columns && section.columns.length > 0
          ? section.columns.map((col) => columnFromNames(col.fields ?? [], byName, used))
          : [columnFromNames(section.fields ?? [], byName, used)];
      const placed = columns.reduce((n, c) => n + c.fields.length, 0);
      if (placed === 0) continue;
      sections.push({
        title: section.title,
        tab: section.tab ?? "",
        columns: columns.filter((c) => c.fields.length > 0),
        visible_when: section.visible_when,
        collapsed: section.collapsed,
      });
    }
  }

  const rest = fields.filter((f) => isFormField(f) && fieldVisible(f, values) && !used.has(f.name));
  if (rest.length) {
    const byTabSection = new Map<string, UiField[]>();
    for (const field of rest) {
      const key = `${field.tab ?? ""}\0${field.section ?? ""}`;
      const list = byTabSection.get(key) ?? [];
      list.push(field);
      byTabSection.set(key, list);
    }
    for (const [key, group] of byTabSection) {
      const [tab, title] = key.split("\0");
      sections.push({
        title: title ?? "",
        tab: tab ?? "",
        columns: [{ fields: group }],
      });
    }
  }

  const visibleSections = sections
    .map((section) => ({
      ...section,
      columns: section.columns.map((col) => ({
        fields: col.fields.filter((f) => fieldVisible(f, values)),
      })).filter((col) => col.fields.length > 0),
    }))
    .filter((section) => section.columns.length > 0);

  const tabs = unique(visibleSections.map((s) => s.tab));
  return { tabs, sections: visibleSections };
}

export function fieldTab(layout: ResolvedLayout, name: string): string {
  for (const section of layout.sections) {
    if (section.columns.some((c) => c.fields.some((f) => f.name === name || name.startsWith(`${f.name}.`)))) {
      return section.tab;
    }
  }
  return "";
}

export function fieldSectionTitle(layout: ResolvedLayout, name: string): string {
  for (const section of layout.sections) {
    if (section.columns.some((c) => c.fields.some((f) => f.name === name || name.startsWith(`${f.name}.`)))) {
      return section.title;
    }
  }
  return "";
}

export function errorFields(fieldErrors: Record<string, string>): string[] {
  return Object.keys(fieldErrors);
}

export function tabHasError(layout: ResolvedLayout, tab: string, fieldErrors: Record<string, string>): boolean {
  return errorFields(fieldErrors).some((name) => fieldTab(layout, name) === tab);
}
