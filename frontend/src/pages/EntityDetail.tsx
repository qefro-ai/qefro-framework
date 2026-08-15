import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  api,
  expandedLabel,
  formVisible,
  listVisible,
  type EntityAction,
  type UiEntity,
  type WorkflowAction,
} from "../api";

export default function EntityDetail({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [row, setRow] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");

  async function load() {
    if (!slug || !id) return;
    const data = await api.get(slug, id);
    setRow(data);
  }

  useEffect(() => {
    load().catch((e) => setError(e.message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, id]);

  if (!meta || !slug || !id) return <p>Unknown entity.</p>;
  if (!row) return <p className="muted">Loading…</p>;

  const workflow = row._workflow as
    | { current?: string; transitions?: WorkflowAction[] }
    | undefined;
  const actions = (row._actions as EntityAction[] | undefined) ?? [];
  const related = (row._related ?? {}) as Record<
    string,
    { slug: string; entity: string; items: Record<string, unknown>[]; total: number }
  >;
  const visible = meta.fields.filter((f) => listVisible(f) || formVisible(f));

  return (
    <div>
      <div className="row">
        <div>
          <div className="badge">{meta.entity}</div>
          <h2>{String(row[meta.display_field || "name"] ?? meta.label)}</h2>
          {workflow?.current && <p className="muted">Status: {workflow.current}</p>}
        </div>
        <div>
          <Link to={`/${slug}/${id}/edit`}>
            <button className="ghost">Edit</button>
          </Link>{" "}
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
      {error && <p className="error">{error}</p>}
      <table>
        <tbody>
          {visible
            .filter((f) => f.relation_kind !== "one_to_many")
            .map((f) => (
              <tr key={f.name}>
                <th>{f.label}</th>
                <td>
                  {f.relation ? (
                    relationLink(row, f.name, entities) ?? ""
                  ) : (
                    fmt(row[f.name])
                  )}
                </td>
              </tr>
            ))}
        </tbody>
      </table>
      {actions.length > 0 ? (
        <p>
          {actions.map((action) => (
            <button
              key={action.name}
              className={action.style === "danger" ? "danger" : action.style === "ghost" ? "ghost" : undefined}
              style={{ marginRight: 8 }}
              onClick={async () => {
                if (action.requires_confirmation && !confirm(`${action.label || action.name}?`)) {
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
        </p>
      ) : (
        workflow?.transitions &&
        workflow.transitions.length > 0 && (
          <p>
            {workflow.transitions.map((t) => (
              <button
                key={t.name}
                style={{ marginRight: 8 }}
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
          </p>
        )
      )}
      {Object.entries(related).map(([name, rel]) => (
        <div key={name}>
          <h3>{meta.fields.find((f) => f.name === name)?.label ?? name}</h3>
          <p className="muted">{rel.total} related</p>
          <ul>
            {rel.items.map((item) => (
              <li key={String(item.id)}>
                <Link to={`/${rel.slug}/${item.id}`}>
                  {String(item.name ?? item.title ?? item.code ?? item.id)}
                </Link>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}

function relationLink(
  row: Record<string, unknown>,
  field: string,
  entities: UiEntity[],
) {
  const expanded = row._expanded as
    | Record<string, { id: string; label: string; slug: string }>
    | undefined;
  const rel = expanded?.[field];
  if (!rel) return expandedLabel(row, field);
  const target = entities.find((e) => e.slug === rel.slug);
  if (!target) return rel.label;
  return <Link to={`/${rel.slug}/${rel.id}`}>{rel.label}</Link>;
}

function fmt(value: unknown) {
  if (value == null) return "";
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}
