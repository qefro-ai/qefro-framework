import { useState } from "react";
import { api } from "../../api";
import { publishAndReload } from "../StudioApp";

type Field = Record<string, unknown>;

function whenFrom(field: unknown, equals: unknown): { field: string; equals: unknown } | undefined {
  const name = String(field ?? "").trim();
  if (!name) return undefined;
  return { field: name, equals };
}

export default function FieldEditor({
  entity,
  fields,
  canEdit,
  canPublish,
  onSaved,
}: {
  entity: string;
  fields: Field[];
  canEdit: boolean;
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const [selected, setSelected] = useState(fields[0]?.name as string | undefined);
  const field = fields.find((f) => f.name === selected);
  const [draft, setDraft] = useState<Record<string, unknown>>({});
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");

  const ui = (field?.ui as Record<string, unknown>) ?? {};
  const widget = String(draft.widget ?? ui.widget ?? "text");

  function patch(): Record<string, unknown> {
    return {
      name: field?.name,
      label: draft.label ?? field?.label,
      description: draft.description ?? ui.description,
      required: draft.required ?? field?.required,
      readonly: draft.readonly ?? ui.readonly,
      hidden: draft.hidden ?? ui.hidden,
      searchable: draft.searchable ?? field?.searchable,
      sortable: draft.sortable ?? ui.sortable,
      filterable: draft.filterable ?? ui.filter,
      widget,
      placeholder: draft.placeholder ?? ui.placeholder,
      help: draft.help ?? ui.help,
      section: draft.section ?? ui.section,
      tab: draft.tab ?? ui.tab,
      width: draft.width ?? ui.width,
      order: draft.order ?? ui.order,
      widget_options: {
        ...((ui.widget_options as object) ?? {}),
        ...((draft.widget_options as object) ?? {}),
      },
      permission_level: draft.permission_level ?? field?.permission_level ?? 0,
      allow_on_submit: draft.allow_on_submit ?? field?.allow_on_submit ?? false,
      visible_when: whenFrom(draft.visible_when_field ?? (ui.visible_when as { field?: string } | undefined)?.field, draft.visible_when_equals ?? (ui.visible_when as { equals?: unknown } | undefined)?.equals),
      readonly_when: whenFrom(draft.readonly_when_field ?? (ui.readonly_when as { field?: string } | undefined)?.field, draft.readonly_when_equals ?? (ui.readonly_when as { equals?: unknown } | undefined)?.equals),
    };
  }

  async function validate() {
    setError("");
    const result = await api.studioValidate({
      kind: "entity.field.ui",
      target: entity,
      payload: patch(),
    });
    setPreview(result);
  }

  async function publish() {
    if (!canPublish) return;
    await validate();
    await publishAndReload({
      kind: "entity.field.ui",
      target: entity,
      payload: patch(),
      confirm_migration: true,
    });
    await onSaved();
  }

  return (
    <div className="studio-split">
      <ul className="studio-list">
        {fields.map((f) => (
          <li key={String(f.name)}>
            <button
              type="button"
              className={f.name === selected ? "" : "ghost"}
              onClick={() => {
                setSelected(String(f.name));
                setDraft({});
                setPreview(null);
              }}
            >
              {String(f.label || f.name)}
              <span className="muted"> {String(f.type)}</span>
              {f.computed ? <span className="muted"> / Computed</span> : null}
            </button>
          </li>
        ))}
      </ul>
      {field && canEdit ? (
        <form
          className="form"
          onSubmit={(e) => {
            e.preventDefault();
            publish().catch((err) => setError(err.message));
          }}
        >
          <label>
            Label
            <input
              value={String(draft.label ?? field.label ?? "")}
              onChange={(e) => setDraft({ ...draft, label: e.target.value })}
            />
          </label>
          <label>
            Help
            <input
              value={String(draft.help ?? ui.help ?? "")}
              onChange={(e) => setDraft({ ...draft, help: e.target.value })}
            />
          </label>
          <label>
            Widget
            <select
              value={widget}
              onChange={(e) => setDraft({ ...draft, widget: e.target.value })}
            >
              {["text", "textarea", "select", "number", "currency", "date", "time", "datetime", "checkbox", "relation"].map(
                (w) => (
                  <option key={w}>{w}</option>
                ),
              )}
            </select>
          </label>
          {widget === "currency" ? (
            <>
              <label>
                Currency
                <input
                  value={String(
                    ((draft.widget_options as Record<string, unknown>)?.currency ??
                      (ui.widget_options as Record<string, unknown> | undefined)?.currency) ??
                      "",
                  )}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      widget_options: { ...(draft.widget_options as object), currency: e.target.value },
                    })
                  }
                />
              </label>
              <label>
                Precision
                <input
                  type="number"
                  value={String(
                    ((draft.widget_options as Record<string, unknown>)?.precision ??
                      (ui.widget_options as Record<string, unknown> | undefined)?.precision) ??
                      2,
                  )}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      widget_options: {
                        ...(draft.widget_options as object),
                        precision: Number(e.target.value),
                      },
                    })
                  }
                />
              </label>
            </>
          ) : null}
          <label>
            Section
            <input
              value={String(draft.section ?? ui.section ?? "")}
              onChange={(e) => setDraft({ ...draft, section: e.target.value })}
            />
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(draft.required ?? field.required)}
              onChange={(e) => setDraft({ ...draft, required: e.target.checked })}
            />
            Required
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(draft.readonly ?? ui.readonly)}
              onChange={(e) => setDraft({ ...draft, readonly: e.target.checked })}
            />
            Readonly
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(draft.hidden ?? ui.hidden)}
              onChange={(e) => setDraft({ ...draft, hidden: e.target.checked })}
            />
            Hidden
          </label>
          <label>
            Permission level
            <select
              value={String(draft.permission_level ?? field.permission_level ?? 0)}
              onChange={(e) => setDraft({ ...draft, permission_level: Number(e.target.value) })}
            >
              <option value="0">0 · normal</option>
              <option value="1">1 · restricted</option>
              <option value="2">2 · sensitive</option>
              <option value="3">3 · highly sensitive</option>
            </select>
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(draft.allow_on_submit ?? field.allow_on_submit)}
              onChange={(e) => setDraft({ ...draft, allow_on_submit: e.target.checked })}
            />
            Allow on submit (editable in lock states)
          </label>
          <label>
            Visible when field
            <input
              value={String(draft.visible_when_field ?? (ui.visible_when as { field?: string } | undefined)?.field ?? "")}
              onChange={(e) => setDraft({ ...draft, visible_when_field: e.target.value })}
              placeholder="party_type"
            />
          </label>
          <label>
            Visible when equals
            <input
              value={String(draft.visible_when_equals ?? (ui.visible_when as { equals?: unknown } | undefined)?.equals ?? "")}
              onChange={(e) => setDraft({ ...draft, visible_when_equals: e.target.value })}
              placeholder="Organization"
            />
          </label>
          <label>
            Read-only when field
            <input
              value={String(draft.readonly_when_field ?? (ui.readonly_when as { field?: string } | undefined)?.field ?? "")}
              onChange={(e) => setDraft({ ...draft, readonly_when_field: e.target.value })}
            />
          </label>
          <label>
            Read-only when equals
            <input
              value={String(draft.readonly_when_equals ?? (ui.readonly_when as { equals?: unknown } | undefined)?.equals ?? "")}
              onChange={(e) => setDraft({ ...draft, readonly_when_equals: e.target.value })}
            />
          </label>
          {preview ? (
            <div className="card">
              <h4>Change Preview</h4>
              <p>
                Impact: {String(preview.impact)}
                {preview.migration_required ? " · ⚠ Database migration required" : " · Safe"}
              </p>
              <pre>{((preview.diff as string[]) ?? []).join("\n")}</pre>
            </div>
          ) : null}
          {error ? <p className="error">{error}</p> : null}
          <div className="actions">
            <button type="button" className="ghost" onClick={() => validate().catch((e) => setError(e.message))}>
              Validate
            </button>
            <button type="submit" disabled={!canPublish}>
              Publish
            </button>
          </div>
        </form>
      ) : (
        <p className="muted">Select a field.</p>
      )}
    </div>
  );
}
