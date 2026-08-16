import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api, type EntityAction, type WorkflowAction } from "../../api";
import { StatusBadge } from "../ui/StatusBadge";
import { displayValue, groupingField, isWorkflowGroup } from "../../metadata/views";
import { friendlyError } from "../../friendlyError";
import type { CollectionViewProps } from "../../views/registry";

type Wf = { field?: string; current?: string; transitions?: WorkflowAction[] };

export default function KanbanView({ meta, slug, rows, loading, onReload, onError }: CollectionViewProps) {
  const group = groupingField(meta);
  const groupName = group?.name || "status";
  const card = meta.views?.kanban?.card;
  const titleField = card?.title || meta.display_field || "name";
  const subtitleField = card?.subtitle;
  const extra = card?.fields ?? [];
  const columns = useMemo(() => {
    const fromEnum = group?.enum_values ?? [];
    const fromRows = [...new Set(rows.map((r) => String(r[groupName] ?? "")))].filter(Boolean);
    const names = fromEnum.length ? fromEnum : fromRows;
    return names.length ? names : ["(none)"];
  }, [group, groupName, rows]);

  const [dragging, setDragging] = useState<string | null>(null);

  async function drop(dest: string, id: string) {
    const row = rows.find((r) => String(r.id) === id);
    if (!row) return;
    const from = String(row[groupName] ?? "");
    if (from === dest) return;
    try {
      if (isWorkflowGroup(meta, groupName)) {
        const wf = row._workflow as Wf | undefined;
        const transition = (wf?.transitions ?? []).find((t) => t.to === dest);
        if (!transition) {
          onError(`Cannot move ${meta.label.toLowerCase()} from ${from || "current state"} to ${dest}.`);
          return;
        }
        await api.transition(slug, id, transition.name);
      } else {
        await api.update(slug, id, { [groupName]: dest });
      }
      onReload();
    } catch (err) {
      onError(friendlyError(err));
      onReload();
    }
  }

  if (loading) return <p className="muted">Loading board…</p>;

  return (
    <div className="kanban" role="list">
      {columns.map((col) => {
        const cards = rows.filter((r) => String(r[groupName] ?? "") === col);
        return (
          <section
            key={col}
            className="kanban-col"
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              e.preventDefault();
              const id = e.dataTransfer.getData("text/plain");
              if (id) void drop(col, id);
              setDragging(null);
            }}
          >
            <header>
              <StatusBadge value={col} indicators={group?.widget_options?.indicators} />
              <span className="muted">{cards.length}</span>
            </header>
            <div className="kanban-cards">
              {cards.map((row) => {
                const wf = row._workflow as Wf | undefined;
                const actions = ((row._actions as EntityAction[] | undefined) ?? []).slice(0, 2);
                const transitions = (wf?.transitions ?? []).slice(0, 2);
                return (
                  <article
                    key={String(row.id)}
                    className={`kanban-card ${dragging === String(row.id) ? "is-dragging" : ""}`}
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData("text/plain", String(row.id));
                      setDragging(String(row.id));
                    }}
                    onDragEnd={() => setDragging(null)}
                  >
                    <Link to={`/${slug}/${row.id}`}>
                      <strong>{displayValue(row, titleField)}</strong>
                    </Link>
                    {subtitleField ? <div className="muted">{displayValue(row, subtitleField)}</div> : null}
                    {extra.map((f) => (
                      <div key={f} className="muted">
                        {displayValue(row, f)}
                      </div>
                    ))}
                    <div className="kanban-actions">
                      {actions.length
                        ? actions.map((a) => (
                            <button
                              key={a.name}
                              type="button"
                              className="ghost"
                              onClick={async () => {
                                try {
                                  await api.action(slug, String(row.id), a.name);
                                  onReload();
                                } catch (err) {
                                  onError(friendlyError(err));
                                }
                              }}
                            >
                              {a.label || a.name}
                            </button>
                          ))
                        : transitions.map((t) => (
                            <button
                              key={t.name}
                              type="button"
                              className="ghost"
                              onClick={async () => {
                                try {
                                  await api.transition(slug, String(row.id), t.name);
                                  onReload();
                                } catch (err) {
                                  onError(friendlyError(err));
                                }
                              }}
                            >
                              {t.label || t.name}
                            </button>
                          ))}
                    </div>
                  </article>
                );
              })}
            </div>
          </section>
        );
      })}
    </div>
  );
}
