import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  api,
  detailVisible,
  expandedLabel,
  tokenHeader,
  type EntityAction,
  type UiEntity,
  type WorkflowAction,
} from "../api";
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
  const [tab, setTab] = useState("");

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
    load().catch((e) => setError(e.message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, id]);

  useRealtime({ entity: meta?.entity, recordId: id }, () => {
    load().catch(() => undefined);
  });

  if (!meta || !slug || !id) return <p>Unknown entity.</p>;
  if (!row && !error) return <p className="muted">Loading…</p>;
  if (!row) return <p className="error">{error || "Unable to load record."}</p>;

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
  const tabs = [...new Set(visible.map((f) => f.tab).filter(Boolean) as string[])];
  const activeTab = tab || tabs[0] || "";
  const shown = tabs.length ? visible.filter((f) => (f.tab ?? "") === activeTab) : visible;
  const sections = group(shown);

  return (
    <div className="page">
      <div className="row">
        <div>
          <div className="badge">{meta.entity}</div>
          <h2>{String(row[meta.display_field || "name"] ?? meta.label)}</h2>
          {workflow?.current && <p className="pill">{workflow.current}</p>}
        </div>
        <div className="actions">
          <Link to={`/${slug}/${id}/edit`}>
            <button className="ghost">Edit</button>
          </Link>
          <a href={`/api/v1/${slug}/${id}/print`} target="_blank" rel="noreferrer">
            <button className="ghost">Print</button>
          </a>
          <a href={`/api/v1/${slug}/${id}/print.pdf`} target="_blank" rel="noreferrer">
            <button className="ghost">PDF</button>
          </a>
          <button
            className="ghost"
            onClick={async () => {
              if (!confirm("Delete this record?")) return;
              await api.remove(slug, id);
              navigate(`/${slug}`);
            }}
          >
            Delete
          </button>
        </div>
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {tabs.length > 1 && (
        <div className="tabs" role="tablist">
          {tabs.map((t) => (
            <button key={t} type="button" className={activeTab === t ? "" : "ghost"} onClick={() => setTab(t)}>
              {t}
            </button>
          ))}
        </div>
      )}
      {sections.map(([section, fields]) => (
        <div key={section || "default"} className="panel">
          {section ? <h3 style={{ padding: "0.85rem 0.85rem 0" }}>{section}</h3> : null}
          <table className="dl">
            <tbody>
              {fields.map((f) => (
                <tr key={f.name}>
                  <th>{f.label}</th>
                  <td>
                    {f.relation
                      ? relationLink(row, f.name, entities)
                      : formatValue(row[f.name], f.widget, theme, f.widget_options?.currency)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
      {actions.length > 0 ? (
        <div className="actions">
          {actions.map((action) => (
            <button
              key={action.name}
              className={action.style === "danger" ? "danger" : action.style === "ghost" ? "ghost" : undefined}
              onClick={async () => {
                const message = action.confirmation_message || `${action.label || action.name}?`;
                if ((action.requires_confirmation || action.confirmation_message) && !confirm(message)) {
                  return;
                }
                try {
                  const next = await api.action(slug, id, action.name);
                  setRow(next);
                  setError("");
                } catch (e) {
                  setError(e instanceof Error ? e.message : "failed");
                }
              }}
            >
              {action.label || action.name}
            </button>
          ))}
        </div>
      ) : (
        workflow?.transitions &&
        workflow.transitions.length > 0 && (
          <div className="actions">
            {workflow.transitions.map((t) => (
              <button
                key={t.name}
                onClick={async () => {
                  try {
                    const next = await api.transition(slug, id, t.name);
                    setRow(next);
                    setError("");
                  } catch (e) {
                    setError(e instanceof Error ? e.message : "failed");
                  }
                }}
              >
                {t.label || t.name}
              </button>
            ))}
          </div>
        )
      )}
      {childTables.map((field) => {
        const items = (Array.isArray(row[field.name]) ? row[field.name] : []) as Record<
          string,
          unknown
        >[];
        const child = entities.find((e) => e.entity === (field.child_entity || field.relation));
        const cols = (child?.fields ?? []).filter(
          (f) => !f.hidden && f.list !== false && f.relation_kind !== "one_to_many",
        );
        return (
          <div key={field.name} className="panel">
            <h3 style={{ padding: "0.85rem 0.85rem 0" }}>{field.label}</h3>
            {items.length === 0 ? (
              <p className="empty">No rows.</p>
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
                        <td key={c.name}>{String(item[c.name] ?? "")}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        );
      })}
      {meta.attachments ? (
        <div className="panel">
          <h3 style={{ padding: "0.85rem 0.85rem 0" }}>Attachments</h3>
          {attachments.length === 0 ? <p className="empty">No files.</p> : null}
          <ul>
            {attachments.map((file) => (
              <li key={String(file.id)}>
                <a
                  href={`/api/v1/attachments/${file.id}`}
                  onClick={async (e) => {
                    e.preventDefault();
                    const res = await fetch(`/api/v1/attachments/${file.id}`, { headers: tokenHeader() });
                    const blob = await res.blob();
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement("a");
                    a.href = url;
                    a.download = String(file.filename ?? "file");
                    a.click();
                    URL.revokeObjectURL(url);
                  }}
                >
                  {String(file.filename ?? file.id)}
                </a>{" "}
                <button
                  type="button"
                  className="ghost"
                  onClick={async () => {
                    await api.deleteAttachment(String(file.id));
                    await load();
                  }}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <label style={{ padding: "0.85rem" }}>
            Attach file
            <input
              type="file"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                await api.uploadAttachment(slug, id, file);
                e.target.value = "";
                await load();
              }}
            />
          </label>
        </div>
      ) : null}
      {links.length > 0 ? (
        <div className="panel related">
          <h3 style={{ padding: "0.85rem 0.85rem 0" }}>Related</h3>
          <ul>
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
          <h3 style={{ padding: "0.85rem 0.85rem 0" }}>
            {meta.fields.find((f) => f.name === name)?.label ?? name}
          </h3>
          <p className="muted" style={{ padding: "0 0.85rem" }}>
            {rel.total} related
          </p>
          {rel.items.length === 0 ? (
            <p className="empty">No related records.</p>
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
      <div className="panel related">
        <h3 style={{ padding: "0.85rem 0.85rem 0" }}>Activity</h3>
        {activity.length === 0 ? (
          <p className="empty">No activity yet.</p>
        ) : (
          <ul>
            {activity.map((item, i) => (
              <li key={String(item.id ?? i)}>
                {String(item.action ?? item.event ?? "updated")} ·{" "}
                {utcToDatetimeLocal(item.created_at, theme.timezone) || ""}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
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
