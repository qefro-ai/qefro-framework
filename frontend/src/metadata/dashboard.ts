import { datePresetRange } from "../format";

export type DashFilter = { field: string; value: string };

export function drilldownSearch(filters: DashFilter[] | undefined, now = new Date()): string {
  const params = new URLSearchParams();
  for (const filter of filters ?? []) {
    const range = datePresetRange(filter.value, now);
    if (range) {
      params.set(`${filter.field}.between`, `${range.from},${range.to}`);
      params.set(`${filter.field}.preset`, filter.value);
    } else {
      params.set(filter.field, filter.value);
    }
  }
  return params.toString();
}

export function drilldownPath(slug: string, filters: DashFilter[] | undefined, now = new Date()): string {
  const search = drilldownSearch(filters, now);
  return search ? `/${slug}?${search}` : `/${slug}`;
}

export function dateFieldsFromFilters(filters: DashFilter[] | undefined): string[] {
  return [...new Set((filters ?? []).map((f) => f.field.split(".")[0]).filter((name) => /date|due_at|_at$/i.test(name)))];
}
