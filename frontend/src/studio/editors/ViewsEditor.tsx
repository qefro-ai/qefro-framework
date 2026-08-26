import { useState } from "react";
import { api } from "../../sdk/client";
import { publishAndReload } from "../StudioApp";
import type { UiEntity } from "../../metadata/types";

export default function ViewsEditor({
  entity,
  ui,
  canPublish,
  onSaved,
}: {
  entity: string;
  ui?: UiEntity;
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const views = ui?.views ?? {};
  const [listColumns, setListColumns] = useState(
    (views.list?.columns ?? []).map((c) => c.field).join(", "),
  );
  const [cardEnabled, setCardEnabled] = useState(Boolean(views.card) && views.card?.enabled !== false);
  const [cardTitle, setCardTitle] = useState(views.card?.title ?? ui?.display_field ?? "");
  const [cardSubtitle, setCardSubtitle] = useState(views.card?.subtitle ?? "");
  const [cardImage, setCardImage] = useState(views.card?.image ?? "");
  const [cardFields, setCardFields] = useState((views.card?.fields ?? []).join(", "));
  const [kanbanGroup, setKanbanGroup] = useState(views.kanban?.group_by ?? "");
  const [kanbanTitle, setKanbanTitle] = useState(views.kanban?.card?.title ?? "");
  const [kanbanSubtitle, setKanbanSubtitle] = useState(views.kanban?.card?.subtitle ?? "");
  const [kanbanFields, setKanbanFields] = useState((views.kanban?.card?.fields ?? []).join(", "));
  const [formSections, setFormSections] = useState(JSON.stringify(views.form?.sections ?? [], null, 2));
  const [detailSections, setDetailSections] = useState(
    JSON.stringify(views.detail?.sections ?? [], null, 2),
  );
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");

  function split(value: string) {
    return value
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
  }

  function payload() {
    const listCols = split(listColumns).map((field) => ({ field }));
    let form;
    let detail;
    try {
      form = { sections: formSections.trim() ? JSON.parse(formSections) : [] };
      detail = { sections: detailSections.trim() ? JSON.parse(detailSections) : [] };
    } catch {
      throw new Error("Form/detail sections must be JSON arrays.");
    }
    return {
      list: { columns: listCols },
      card: cardEnabled
        ? {
            enabled: true,
            title: cardTitle || undefined,
            subtitle: cardSubtitle || undefined,
            image: cardImage || undefined,
            fields: split(cardFields),
          }
        : { enabled: false },
      kanban: kanbanGroup
        ? {
            enabled: true,
            group_by: kanbanGroup,
            card: {
              title: kanbanTitle || undefined,
              subtitle: kanbanSubtitle || undefined,
              fields: split(kanbanFields),
            },
          }
        : undefined,
      form,
      detail,
    };
  }

  async function validate() {
    setError("");
    const result = await api.studioValidate({
      kind: "entity.views",
      target: entity,
      payload: payload(),
    });
    setPreview(result as unknown as Record<string, unknown>);
  }

  async function publish() {
    if (!canPublish) return;
    await validate();
    await publishAndReload({
      kind: "entity.views",
      target: entity,
      payload: payload(),
      confirm_migration: true,
    });
    await onSaved();
  }

  const names = (ui?.fields ?? []).map((f) => f.name).join(", ");

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        publish().catch((err) => setError(err.message));
      }}
    >
      <p className="muted">Fields: {names || "—"}</p>
      <label>
        List columns (comma-separated)
        <input value={listColumns} onChange={(e) => setListColumns(e.target.value)} />
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={cardEnabled}
          onChange={(e) => setCardEnabled(e.target.checked)}
        />
        Enable Cards view
      </label>
      {cardEnabled ? (
        <>
          <label>
            Card title field
            <input value={cardTitle} onChange={(e) => setCardTitle(e.target.value)} />
          </label>
          <label>
            Card subtitle field
            <input value={cardSubtitle} onChange={(e) => setCardSubtitle(e.target.value)} />
          </label>
          <label>
            Card image field
            <input value={cardImage} onChange={(e) => setCardImage(e.target.value)} />
          </label>
          <label>
            Card fields
            <input value={cardFields} onChange={(e) => setCardFields(e.target.value)} />
          </label>
        </>
      ) : null}
      <label>
        Kanban group_by
        <input value={kanbanGroup} onChange={(e) => setKanbanGroup(e.target.value)} />
      </label>
      <label>
        Kanban card title
        <input value={kanbanTitle} onChange={(e) => setKanbanTitle(e.target.value)} />
      </label>
      <label>
        Kanban card subtitle
        <input value={kanbanSubtitle} onChange={(e) => setKanbanSubtitle(e.target.value)} />
      </label>
      <label>
        Kanban card fields
        <input value={kanbanFields} onChange={(e) => setKanbanFields(e.target.value)} />
      </label>
      <label>
        Form sections (JSON)
        <textarea rows={6} value={formSections} onChange={(e) => setFormSections(e.target.value)} />
      </label>
      <label>
        Detail sections (JSON)
        <textarea rows={6} value={detailSections} onChange={(e) => setDetailSections(e.target.value)} />
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
          Publish views
        </button>
      </div>
    </form>
  );
}
