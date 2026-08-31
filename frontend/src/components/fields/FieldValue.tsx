import { Link } from "react-router-dom";
import type { UiEntity, UiField } from "../../metadata/types";
import { expandedLabel, type Expanded } from "../../sdk/client";
import { relativeTime, dueChip } from "../../format";
import { formatMoney, utcToDatetimeLocal } from "../../metadata/timezone";
import { useTenantTheme } from "../../metadata/context";
import { displayValue } from "../../metadata/views";
import { StatusBadge } from "../ui/StatusBadge";

export function FieldValue({
  row,
  field,
  fieldName,
  entities,
  linkRelations = false,
  relativeDates = true,
  compact = true,
}: {
  row: Record<string, unknown>;
  field?: UiField;
  fieldName?: string;
  entities?: UiEntity[];
  linkRelations?: boolean;
  relativeDates?: boolean;
  compact?: boolean;
}) {
  const theme = useTenantTheme();
  const name = field?.name ?? fieldName ?? "";
  const widget = field?.widget ?? "";

  if (field?.relation || widget === "relation") {
    return relationValue(row, name, linkRelations, entities);
  }

  const value = name ? row[name] : undefined;
  if (value == null || value === "") return null;

  if (widget === "status" || name === "status") {
    return <StatusBadge value={value} indicators={field?.widget_options?.indicators} />;
  }
  if (widget === "currency") {
    return formatMoney(value, field?.widget_options?.currency || theme.currency, theme.locale);
  }
  if (widget === "percentage") return `${value}%`;
  if (widget === "image" && value) {
    const src = `/api/v1/files/${encodeURIComponent(String(value))}`;
    return (
      <img
        src={src}
        alt=""
        className={compact ? "avatar" : "image-preview"}
      />
    );
  }
  if (widget === "color") {
    const color = safeCssColor(String(value));
    return (
      <span className="swatch">
        {color ? <i style={{ background: color }} /> : null} {String(value)}
      </span>
    );
  }
  if (widget === "rich_text") {
    return <div className="rich-surface" dangerouslySetInnerHTML={{ __html: sanitizeRichHtml(String(value)) }} />;
  }
  if (widget === "datetime" || field?.type === "datetime") {
    if (name === "due_at") {
      const chip = dueChip(value, row.status);
      if (chip) {
        return (
          <span className={`status-badge tone-${chip === "Overdue" ? "danger" : "warning"}`}>
            {chip}
          </span>
        );
      }
    }
    if (relativeDates) return relativeTime(value, theme.locale);
    return utcToDatetimeLocal(value, theme.timezone).replace("T", " ");
  }
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function relationValue(
  row: Record<string, unknown>,
  name: string,
  linkRelations: boolean,
  entities?: UiEntity[],
) {
  const expanded = row._expanded as Record<string, Expanded> | undefined;
  const rel = expanded?.[name];
  const main = relationLink(rel, linkRelations, entities) ?? expandedLabel(row, name) ?? displayValue(row, name) ?? null;
  const nested = rel?._expanded;
  if (!nested || typeof nested !== "object") return main;
  const extras = Object.values(nested).filter((n) => n?.id && n.slug);
  if (!extras.length) return main;
  return (
    <span className="rel-path">
      {main}
      {extras.map((n) => (
        <span key={n.id}>
          {" · "}
          {relationLink(n, linkRelations, entities) ?? n.label}
          {n.enabled === false ? <span className="muted"> (disabled)</span> : null}
          {n.enabled === true ? <span className="muted"> (enabled)</span> : null}
        </span>
      ))}
    </span>
  );
}

function relationLink(
  rel: Expanded | undefined,
  linkRelations: boolean,
  entities?: UiEntity[],
) {
  if (!rel) return null;
  if (linkRelations && rel.slug && rel.id && (!entities || entities.some((e) => e.slug === rel.slug))) {
    return <Link to={`/${rel.slug}/${rel.id}`}>{rel.label}</Link>;
  }
  return rel.label ?? null;
}

/** Defense in depth: server already sanitizes with ammonia. */
export function sanitizeRichHtml(html: string): string {
  if (typeof DOMParser === "undefined") return html;
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script,iframe,object,embed,link,meta").forEach((el) => el.remove());
  doc.querySelectorAll("*").forEach((el) => {
    for (const attr of Array.from(el.attributes)) {
      const name = attr.name.toLowerCase();
      const value = attr.value.trim().toLowerCase();
      if (name.startsWith("on") || name === "style" || value.startsWith("javascript:") || value.startsWith("data:text/html")) {
        el.removeAttribute(attr.name);
      }
    }
  });
  return doc.body.innerHTML;
}

export function safeCssColor(value: string): string | undefined {
  const v = value.trim();
  if (/^#[0-9a-fA-F]{3,8}$/.test(v)) return v;
  if (/^rgba?\(\s*[\d.]+%?\s*,\s*[\d.]+%?\s*,\s*[\d.]+%?(?:\s*,\s*[\d.]+)?\s*\)$/.test(v)) return v;
  if (/^[a-zA-Z]+$/.test(v)) return v;
  return undefined;
}
