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
    return (
      <img
        src={`/api/v1/files/${encodeURIComponent(String(value))}`}
        alt=""
        className={compact ? "avatar" : "image-preview"}
      />
    );
  }
  if (widget === "color") {
    return (
      <span className="swatch">
        <i style={{ background: String(value) }} /> {String(value)}
      </span>
    );
  }
  if (widget === "rich_text") {
    return <div className="rich-surface" dangerouslySetInnerHTML={{ __html: String(value) }} />;
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
