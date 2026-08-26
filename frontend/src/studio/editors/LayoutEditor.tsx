import { useState } from "react";
import { api } from "../../sdk/client";
import { publishAndReload } from "../StudioApp";
import type { UiEntity } from "../../metadata/types";

export default function LayoutEditor({
  entity,
  ui,
  canPublish,
  onSaved,
}: {
  entity: string;
  ui: UiEntity;
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const [drafts, setDrafts] = useState(
    () =>
      ui.fields.map((f) => ({
        name: f.name,
        label: f.label,
        section: f.section ?? "",
        tab: f.tab ?? "",
        order: String(f.order ?? 0),
        width: f.width ?? "",
      })),
  );
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);

  function patch(name: string, key: string, value: string) {
    setDrafts((rows) => rows.map((row) => (row.name === name ? { ...row, [key]: value } : row)));
  }

  async function publish() {
    setError("");
    let last: Record<string, unknown> | null = null;
    for (const row of drafts) {
      const payload = {
        name: row.name,
        label: row.label,
        section: row.section || undefined,
        tab: row.tab || undefined,
        order: Number(row.order) || 0,
        width: row.width || undefined,
      };
      last = (await api.studioValidate({
        kind: "entity.field.ui",
        target: entity,
        payload,
      })) as unknown as Record<string, unknown>;
      if (!canPublish) continue;
      await publishAndReload({
        kind: "entity.field.ui",
        target: entity,
        payload,
        confirm_migration: true,
      });
    }
    setPreview(last);
    await onSaved();
  }

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        publish().catch((err) => setError(err.message));
      }}
    >
      <p className="muted">Publishes field order, section, tab, and label via entity.field.ui.</p>
      <table className="data">
        <thead>
          <tr>
            <th>Field</th>
            <th>Label</th>
            <th>Section</th>
            <th>Tab</th>
            <th>Order</th>
            <th>Width</th>
          </tr>
        </thead>
        <tbody>
          {drafts.map((row) => (
            <tr key={row.name}>
              <td>{row.name}</td>
              <td>
                <input value={row.label} onChange={(e) => patch(row.name, "label", e.target.value)} />
              </td>
              <td>
                <input value={row.section} onChange={(e) => patch(row.name, "section", e.target.value)} />
              </td>
              <td>
                <input value={row.tab} onChange={(e) => patch(row.name, "tab", e.target.value)} />
              </td>
              <td>
                <input value={row.order} onChange={(e) => patch(row.name, "order", e.target.value)} />
              </td>
              <td>
                <input value={row.width} onChange={(e) => patch(row.name, "width", e.target.value)} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {preview ? (
        <p className="muted">
          Last field impact: {String(preview.impact)}
          {preview.migration_required ? " · migration required" : " · Safe"}
        </p>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
      <button type="submit" disabled={!canPublish}>
        Publish layout
      </button>
    </form>
  );
}
