import { useMemo, useState } from "react";
import { api } from "../../api";
import { publishAndReload } from "../StudioApp";
import { Button } from "../../components/ui/Button";
import { Chip } from "../../components/ui/Chip";
import { ConfirmDialog } from "../../components/ui/ConfirmDialog";
import FormPreview from "../preview/FormPreview";
import type { UiEntity } from "../../api";

type Field = Record<string, unknown>;

const TYPES = [
  { value: "string", label: "Text" },
  { value: "text", label: "Textarea" },
  { value: "integer", label: "Number" },
  { value: "decimal", label: "Decimal" },
  { value: "boolean", label: "Boolean" },
  { value: "date", label: "Date" },
  { value: "datetime", label: "Date and time" },
  { value: "select", label: "Select" },
  { value: "email", label: "Email" },
  { value: "phone", label: "Phone" },
  { value: "currency", label: "Currency" },
];

function emptyDraft() {
  return {
    name: "",
    label: "",
    type: "string",
    required: false,
    readonly: false,
    hidden: false,
    filterable: false,
    default: "",
    options: "Bronze, Silver, Gold",
    help: "",
    section: "Custom",
  };
}

export default function CustomFieldsEditor({
  entity,
  fields,
  ui,
  canEdit,
  canPublish,
  onSaved,
}: {
  entity: string;
  fields: Field[];
  ui?: UiEntity;
  canEdit: boolean;
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const custom = fields.filter((f) => f.custom);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(emptyDraft());
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);
  const [disableName, setDisableName] = useState<string | null>(null);

  const previewEntity = useMemo(() => {
    if (!ui) return null;
    const extra = draft.name.trim()
      ? [
          {
            name: draft.name.trim(),
            type: draft.type === "select" ? "enum" : draft.type,
            label: draft.label || draft.name,
            required: draft.required,
            readonly: draft.readonly,
            hidden: draft.hidden,
            filter: draft.filterable,
            filterable: draft.filterable,
            list: false,
            list_visible: false,
            form: !draft.hidden,
            form_visible: !draft.hidden,
            detail: !draft.hidden,
            detail_visible: !draft.hidden,
            searchable: false,
            widget:
              draft.type === "select"
                ? "select"
                : draft.type === "text"
                  ? "textarea"
                  : draft.type === "boolean"
                    ? "checkbox"
                    : draft.type,
            section: draft.section || "Custom",
            custom: true,
            enum_values:
              draft.type === "select"
                ? draft.options
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean)
                : undefined,
            default: draft.default || undefined,
          },
        ]
      : [];
    return {
      ...ui,
      fields: [...ui.fields.filter((f) => f.name !== draft.name.trim()), ...extra],
    } as UiEntity;
  }, [ui, draft]);

  function payload() {
    const options = draft.options
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    return {
      name: draft.name.trim(),
      label: draft.label.trim() || draft.name.trim(),
      type: draft.type,
      options: draft.type === "select" ? options : undefined,
      required: draft.required,
      readonly: draft.readonly,
      hidden: draft.hidden,
      filterable: draft.filterable,
      default: draft.default || undefined,
      help: draft.help || undefined,
      section: draft.section || "Custom",
      custom: true,
    };
  }

  async function validate() {
    setError("");
    const result = await api.studioValidate({
      kind: "entity.custom_field",
      target: entity,
      payload: payload(),
    });
    setPreview(result);
    return result;
  }

  async function publish() {
    if (!canPublish) return;
    await validate();
    await publishAndReload({
      kind: "entity.custom_field",
      target: entity,
      payload: payload(),
      summary: `Custom field added: ${entity}.${draft.name.trim()}`,
    });
    setOpen(false);
    setDraft(emptyDraft());
    await onSaved();
  }

  async function disable(name: string) {
    await publishAndReload({
      kind: "entity.custom_field",
      target: entity,
      payload: { name, status: "disabled", custom: true, type: "string" },
      summary: `Custom field disabled: ${entity}.${name}`,
    });
    setDisableName(null);
    await onSaved();
  }

  return (
    <div className="stack">
      <div className="card">
        <h3>Custom fields</h3>
        <p className="muted">
          Extend {entity} without changing framework source. Values are stored in a JSONB bag and
          flow through EntityService.
        </p>
        {custom.length === 0 ? (
          <p className="muted">No custom fields yet.</p>
        ) : (
          <ul className="studio-list">
            {custom.map((f) => (
              <li key={String(f.name)}>
                <strong>{String(f.label || f.name)}</strong>
                <span className="muted"> {String(f.name)}</span>
                <Chip>{String(f.type)}</Chip>
                {f.required ? <Chip>Required</Chip> : null}
                {String(f.custom_status || "active") !== "active" ? (
                  <Chip>{String(f.custom_status)}</Chip>
                ) : null}
                {canPublish ? (
                  <Button
                    variant="text"
                    onClick={() => setDisableName(String(f.name))}
                  >
                    Disable
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {canEdit ? (
          <Button onClick={() => setOpen(true)}>+ Add custom field</Button>
        ) : null}
      </div>

      {open ? (
        <form
          className="card form"
          onSubmit={(e) => {
            e.preventDefault();
            publish().catch((err) => setError(err.message));
          }}
        >
          <h3>Add custom field</h3>
          <label>
            Field name
            <input
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              placeholder="loyalty_tier"
              required
            />
          </label>
          <label>
            Label
            <input
              value={draft.label}
              onChange={(e) => setDraft({ ...draft, label: e.target.value })}
              placeholder="Loyalty Tier"
            />
          </label>
          <label>
            Type
            <select
              value={draft.type}
              onChange={(e) => setDraft({ ...draft, type: e.target.value })}
            >
              {TYPES.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </select>
          </label>
          {draft.type === "select" ? (
            <label>
              Options
              <input
                value={draft.options}
                onChange={(e) => setDraft({ ...draft, options: e.target.value })}
                placeholder="Bronze, Silver, Gold"
              />
            </label>
          ) : null}
          <label>
            Default
            <input
              value={draft.default}
              onChange={(e) => setDraft({ ...draft, default: e.target.value })}
            />
          </label>
          <label>
            Help
            <input
              value={draft.help}
              onChange={(e) => setDraft({ ...draft, help: e.target.value })}
            />
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={draft.required}
              onChange={(e) => setDraft({ ...draft, required: e.target.checked })}
            />
            Required
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={draft.readonly}
              onChange={(e) => setDraft({ ...draft, readonly: e.target.checked })}
            />
            Read-only
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={draft.hidden}
              onChange={(e) => setDraft({ ...draft, hidden: e.target.checked })}
            />
            Hidden
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={draft.filterable}
              onChange={(e) => setDraft({ ...draft, filterable: e.target.checked })}
            />
            Filterable
          </label>
          {preview ? (
            <div className="card">
              <p>
                Impact: {String(preview.impact)}
                {preview.migration_required ? " · ⚠ Database migration required" : " · Safe"}
              </p>
              <pre>{((preview.diff as string[]) ?? []).join("\n")}</pre>
            </div>
          ) : null}
          {error ? <p className="error">{error}</p> : null}
          <div className="actions">
            <Button
              variant="outlined"
              type="button"
              onClick={() => validate().catch((e) => setError(e.message))}
            >
              Validate
            </Button>
            <Button type="submit" disabled={!canPublish}>
              Publish
            </Button>
            <Button variant="outlined" type="button" onClick={() => setOpen(false)}>
              Cancel
            </Button>
          </div>
        </form>
      ) : null}

      {previewEntity ? <FormPreview entity={previewEntity} /> : null}

      <ConfirmDialog
        open={Boolean(disableName)}
        title="Disable custom field"
        message={
          disableName
            ? `${disableName} will be hidden from forms. Stored values are kept.`
            : undefined
        }
        confirmLabel="Disable"
        danger
        onConfirm={() => disableName && disable(disableName).catch((e) => setError(e.message))}
        onCancel={() => setDisableName(null)}
      />
    </div>
  );
}
