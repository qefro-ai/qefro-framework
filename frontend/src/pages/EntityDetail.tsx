import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  api,
  detailVisible,
  expandedLabel,
  type EntityAction,
  type UiEntity,
  type WorkflowAction,
} from "../api";
import { ActionBar } from "../components/actions/ActionBar";
import { AttachmentsPanel } from "../components/attachments/AttachmentsPanel";
import { Timeline } from "../components/timeline/Timeline";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { StatusBadge } from "../components/ui/StatusBadge";
import { friendlyError } from "../friendlyError";
import { relativeTime } from "../format";
import { formatMoney, utcToDatetimeLocal } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";
import { useRealtime } from "../realtime";

export default function EntityDetail({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [row, setRow] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [activity, setActivity] = useState<Array<Record<string, unknown>>>([]);
  const [attachments, setAttachments] = useState<Array<Record<string, unknown>>>([]);
  const theme = useTenantTheme();
  const [tab, setTab] = useState("overview");

  async function load() {
    if (!slug || !id) return;
    const data = await api.get(slug, id);
    setRow(data);
    if (meta) {
      api
        .audit(meta.entity, id)
        .then((d) => setActivity(d.items ?? []))
        .catch(() => setActivity([]));
      if (meta.attachments) {
        api
          .attachments(slug, id)
          .then((d) => setAttachments(d.items ?? []))
          .catch(() => setAttachments([]));
      }
    }
  }

  useEffect(() => {
    load().catch((e) => setError(friendlyError(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, id]);

  useRealtime({ entity: meta?.entity, recordId: id }, () => {
    load().catch(() => undefined);
  });

  if (!meta || !slug || !id) return <ErrorState message="Unknown entity." />;
  if (!row && !error) return <Skeleton rows={6} />;
  if (!row) return <ErrorState message={error || "Unable to load record."} />;

  const workflow = row._workflow as
    | { current?: string; transitions?: WorkflowAction[] }
    | undefined;
  const actions = (row._actions as EntityAction[] | undefined) ?? [];
  const related = (row._related ?? {}) as Record<
    string,
    { slug: string; entity: string; items: Record<string, unknown>[]; total: number }
  >;
  const links = ((row._links as Array<{
    label: string;
    entity: string;
    slug: string;
    relation: string;
    total: number;
  }>) ?? []).filter((l) => !related[l.relation]);
  const visible = meta.fields.filter(
    (f) =>
      detailVisible(f) &&
      f.relation_kind !== "one_to_many" &&
      f.relation_kind !== "child_table" &&
      f.type !== "child_table",
  );
  const childTables = meta.fields.filter(
    (f) => f.relation_kind === "child_table" || f.type === "child_table",
  );
  const number =
    row[meta.naming?.field || ""] ??
    row.name ??
    row.code ??
    row.title ??
    id;
  const owner = row.owner ?? row.created_by ?? row.modified_by;
  const created = row.created_at;
  const statusField = meta.fields.find((f) => f.widget === "status" || f.name === "status");
  const status = workflow?.current ?? row.status;

  const chrome = [
    { id: "overview", label: "Overview" },
    ...childTables.map((c) => ({ id: `items:${c.name}`, label: c.label })),
    { id: "related", label: "Related", hide: links.length === 0 && Object.keys(related).length === 0 },
    { id: "files", label: "Attachments", hide: !meta.attachments },
    { id: "activity", label: "Activity" },
  ].filter((t) => !t.hide);

  async function runAction(name: string, action?: EntityAction) {
    if (!slug || !id) return;
    const message = action?.confirmation_message || `${action?.label || name}?`;
    if ((action?.requires_confirmation || action?.confirmation_message) && !confirm(message)) return;
    try {
      const next = await api.action(slug, id, name);
      setRow(next);
      setError("");
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  return (
    <div className="page">
      <header className="doc-header">
        <div>
          <div className="badge">{meta.entity}</div>
          <h2>
            {meta.label} {String(number)}
          </h2>
          <p className="muted doc-meta">
            {status ? <StatusBadge value={status} indicators={statusField?.widget_options?.indicators} /> : null}
            {owner ? <span> · {String(owner)}</span> : null}
            {created ? <span> · {relativeTime(created, theme.locale)}</span> : null}
          </p>
        </div>
        <div className="actions">
          <Link to={`/${slug}/${id}/edit`}>
            <button>Edit</button>
          </Link>
          <ActionBar
            actions={actions}
            transitions={workflow?.transitions}
            onAction={(name, action) => void runAction(name, action)}
            onTransition={async (name) => {
              try {
                const next = await api.transition(slug, id, name);
                setRow(next);
                setError("");
              } catch (e) {
                setError(friendlyError(e));
              }
            }}
          />
          <div className="more-menu">
            <details>
              <summary className="ghost btn-like">More</summary>
              <div className="menu-list">
                <a href={`/api/v1/${slug}/${id}/print`} target="_blank" rel="noreferrer">
                  Print
                </a>
                <a href={`/api/v1/${slug}/${id}/print.pdf`} target="_blank" rel="noreferrer">
                  Download PDF
                </a>
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    if (!confirm("Delete this record?")) return;
                    try {
                      await api.remove(slug, id);
                      navigate(`/${slug}`);
                    } catch (e) {
                      setError(friendlyError(e));
                    }
                  }}
                >
                  Delete
                </button>
              </div>
            </details>
          </div>
        </div>
      </header>
      {error && <ErrorState message={error} />}
      {statusField?.enum_values && statusField.enum_values.length > 1 ? (
        <ol className="wf-strip" aria-label="Status">
          {statusField.enum_values.map((step) => (
            <li key={step} className={String(status) === step ? "is-current" : undefined}>
              {step}
            </li>
          ))}
        </ol>
      ) : null}
      <div className="tabs" role="tablist">
        {chrome.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "" : "ghost"}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === "overview" ? (
        <Overview
          row={row}
          fields={visible}
          entities={entities}
          theme={theme}
          sections={meta.views?.detail?.sections}
        />
      ) : null}
      {childTables.map((field) =>
        tab === `items:${field.name}` ? (
          <ChildPanel key={field.name} field={field} row={row} entities={entities} />
        ) : null,
      )}
      {tab === "related" ? (
        <RelatedPanel links={links} related={related} id={id} />
      ) : null}
      {tab === "files" && meta.attachments ? (
        <div className="panel" style={{ padding: "0.85rem" }}>
          <h3>Attachments</h3>
          <AttachmentsPanel slug={slug} id={id} items={attachments} onChanged={() => void load()} />
        </div>
      ) : null}
      {tab === "activity" ? (
        <div className="panel" style={{ padding: "0.85rem" }}>
          <h3>Activity</h3>
          <Timeline
            items={activity.map((item) => ({
              id: String(item.id ?? ""),
              action: String(item.action ?? item.event ?? "updated"),
              actor: item.actor ? String(item.actor) : item.user ? String(item.user) : undefined,
              created_at: item.created_at,
              summary: String(item.summary ?? item.action ?? item.event ?? "updated"),
            }))}
            timezone={theme.timezone}
            locale={theme.locale}
          />
        </div>
      ) : null}
    </div>
  );
}

function Overview({
  row,
  fields,
  entities,
  theme,
  sections: spec,
}: {
  row: Record<string, unknown>;
  fields: UiEntity["fields"];
  entities: UiEntity[];
  theme: { currency: string; locale: string; timezone: string };
  sections?: Array<{ title: string; fields?: string[] }>;
}) {
  const sections =
    spec && spec.length
      ? spec
          .map((s) => [s.title, fields.filter((f) => (s.fields ?? []).includes(f.name))] as const)
          .filter(([, fs]) => fs.length)
      : group(fields);
  if (fields.length === 0) return <EmptyState title="Nothing to show" />;
  return (
    <>
      {sections.map(([section, sectionFields]) => (
        <div key={section || "default"} className="panel">
          {section ? <h3 style={{ padding: "0.85rem 0.85rem 0" }}>{section}</h3> : null}
          <table className="dl">
            <tbody>
              {sectionFields.map((f) => (
                <tr key={f.name}>
                  <th>{f.label}</th>
                  <td>
                    {f.widget === "status" || f.name === "status" ? (
                      <StatusBadge value={row[f.name]} indicators={f.widget_options?.indicators} />
                    ) : f.relation ? (
                      relationLink(row, f.name, entities)
                    ) : (
                      formatValue(row[f.name], f.widget, theme, f.widget_options?.currency)
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </>
  );
}

function ChildPanel({
  field,
  row,
  entities,
}: {
  field: UiEntity["fields"][number];
  row: Record<string, unknown>;
  entities: UiEntity[];
}) {
  const items = (Array.isArray(row[field.name]) ? row[field.name] : []) as Record<string, unknown>[];
  const child = entities.find((e) => e.entity === (field.child_entity || field.relation));
  const cols = (child?.fields ?? []).filter(
    (f) => !f.hidden && f.list !== false && f.relation_kind !== "one_to_many",
  );
  return (
    <div className="panel">
      <h3 style={{ padding: "0.85rem 0.85rem 0" }}>{field.label}</h3>
      {items.length === 0 ? (
        <EmptyState title={`No ${field.label.toLowerCase()} yet`} />
      ) : (
        <table className="data">
          <thead>
            <tr>
              {cols.map((c) => (
                <th key={c.name}>{c.label}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {items.map((item, i) => (
              <tr key={String(item.id ?? i)}>
                {cols.map((c) => (
                  <td key={c.name} data-label={c.label}>
                    {String(item[c.name] ?? "")}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function RelatedPanel({
  links,
  related,
  id,
}: {
  links: Array<{ label: string; slug: string; relation: string; total: number }>;
  related: Record<string, { slug: string; items: Record<string, unknown>[]; total: number }>;
  id: string;
}) {
  return (
    <>
      {links.length > 0 ? (
        <div className="panel related">
          <h3 style={{ padding: "0.85rem 0.85rem 0" }}>Related</h3>
          <ul className="related-tree">
            {links.map((link) => (
              <li key={`${link.slug}-${link.relation}`}>
                <Link to={`/${link.slug}?${encodeURIComponent(link.relation)}=${id}`}>
                  {link.label} ({link.total})
                </Link>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {Object.entries(related).map(([name, rel]) => (
        <div key={name} className="related panel">
          <h3 style={{ padding: "0.85rem 0.85rem 0" }}>{name}</h3>
          <p className="muted" style={{ padding: "0 0.85rem" }}>
            {rel.total} related
          </p>
          {rel.items.length === 0 ? (
            <EmptyState title="No related records." />
          ) : (
            <ul>
              {rel.items.map((item) => (
                <li key={String(item.id)}>
                  <Link to={`/${rel.slug}/${item.id}`}>
                    {String(item.name ?? item.title ?? item.code ?? item.id)}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}
      {links.length === 0 && Object.keys(related).length === 0 ? (
        <EmptyState title="No related records" />
      ) : null}
    </>
  );
}

function group(fields: UiEntity["fields"]) {
  const map = new Map<string, typeof fields>();
  for (const f of fields) {
    const key = f.section ?? "";
    const list = map.get(key) ?? [];
    list.push(f);
    map.set(key, list);
  }
  return Array.from(map.entries());
}

function relationLink(row: Record<string, unknown>, field: string, entities: UiEntity[]) {
  const expanded = row._expanded as
    | Record<string, { id: string; label: string; slug: string }>
    | undefined;
  const rel = expanded?.[field];
  if (!rel) return expandedLabel(row, field);
  const target = entities.find((e) => e.slug === rel.slug);
  if (!target) return rel.label;
  return <Link to={`/${rel.slug}/${rel.id}`}>{rel.label}</Link>;
}

function formatValue(
  value: unknown,
  widget: string,
  theme: { currency: string; locale: string; timezone: string },
  currency?: string,
) {
  if (value == null) return "";
  if (widget === "currency") return formatMoney(value, currency || theme.currency, theme.locale);
  if (widget === "percentage") return `${value}%`;
  if (widget === "datetime") return utcToDatetimeLocal(value, theme.timezone).replace("T", " ");
  if (widget === "rich_text") {
    return <div className="rich-surface" dangerouslySetInnerHTML={{ __html: String(value) }} />;
  }
  if (widget === "color") {
    return (
      <span className="swatch">
        <i style={{ background: String(value) }} /> {String(value)}
      </span>
    );
  }
  if (widget === "image") {
    return <img src={`/api/v1/files/${encodeURIComponent(String(value))}`} alt="" className="image-preview" />;
  }
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}
