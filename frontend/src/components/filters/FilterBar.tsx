import { useEffect, useId, useMemo, useRef, useState } from "react";
import { api, type UiField } from "../../api";
import { datePresetRange } from "../../format";
import { renderWidget } from "../../metadata/registry";
import type { UiEntity } from "../../metadata/types";

const OPS: Record<string, string[]> = {
  string: ["eq", "neq", "contains", "starts_with", "in", "not_in", "empty", "not_empty"],
  text: ["contains", "starts_with", "empty", "not_empty"],
  integer: ["eq", "neq", "gt", "lt", "between", "empty", "not_empty"],
  decimal: ["eq", "neq", "gt", "lt", "between", "empty", "not_empty"],
  date: ["eq", "gt", "lt", "between", "empty", "not_empty"],
  time: ["eq", "gt", "lt"],
  datetime: ["eq", "gt", "lt", "between"],
  boolean: ["eq"],
  enum: ["eq", "neq", "in", "not_in"],
  relation: ["eq", "empty", "not_empty"],
  uuid: ["eq"],
  json: ["empty", "not_empty"],
};

const OP_LABEL: Record<string, string> = {
  eq: "equals",
  neq: "not equals",
  contains: "contains",
  starts_with: "starts with",
  between: "between",
  gt: "greater than",
  lt: "less than",
  in: "in",
  not_in: "not in",
  empty: "empty",
  not_empty: "not empty",
};

const DATE_PRESETS = [
  ["today", "Today"],
  ["yesterday", "Yesterday"],
  ["this_week", "This week"],
  ["this_month", "This month"],
  ["last_7_days", "Last 7 days"],
  ["last_30_days", "Last 30 days"],
];

type Draft = { field: string; op: string; value: string; value2?: string; preset?: string };

function readDrafts(fields: UiField[], params: URLSearchParams): Draft[] {
  const drafts: Draft[] = [];
  for (const field of fields) {
    const ops = OPS[field.type] ?? ["eq", "contains"];
    const op = params.get(`${field.name}.op`) ?? (params.get(`${field.name}.between`) ? "between" : ops[0]);
    const value =
      params.get(op === "eq" ? field.name : `${field.name}.${op}`) ?? params.get(field.name) ?? "";
    const preset = params.get(`${field.name}.preset`) ?? "";
    if (value || op === "empty" || op === "not_empty" || preset) {
      const [a, b] = value.split(",");
      drafts.push({ field: field.name, op, value: a ?? value, value2: b, preset });
    }
  }
  return drafts;
}

function draftIsComplete(draft: Draft) {
  return Boolean(draft.value || draft.op === "empty" || draft.op === "not_empty" || draft.preset);
}

function writeFilters(
  params: URLSearchParams,
  fields: UiField[],
  drafts: Draft[],
): URLSearchParams {
  const next = new URLSearchParams(params);
  for (const field of fields) {
    next.delete(field.name);
    next.delete(`${field.name}.op`);
    next.delete(`${field.name}.preset`);
    for (const op of OPS[field.type] ?? []) next.delete(`${field.name}.${op}`);
  }
  for (const draft of drafts) {
    const field = fields.find((f) => f.name === draft.field);
    if (!field) continue;
    if (draft.preset && (field.type === "date" || field.type === "datetime")) {
      const range = datePresetRange(draft.preset);
      if (range) {
        next.set(`${field.name}.between`, `${range.from},${range.to}`);
        next.set(`${field.name}.preset`, draft.preset);
      }
      continue;
    }
    if (draft.op === "empty" || draft.op === "not_empty") {
      next.set(`${field.name}.${draft.op}`, "1");
      next.set(`${field.name}.op`, draft.op);
      continue;
    }
    const value = draft.op === "between" ? `${draft.value},${draft.value2 ?? ""}` : draft.value;
    if (!value) continue;
    next.set(draft.op === "eq" ? field.name : `${field.name}.${draft.op}`, value);
    next.set(`${field.name}.op`, draft.op);
  }
  next.set("page", "1");
  return next;
}

function chipLabel(field: UiField, draft: Draft) {
  if (draft.preset) {
    return `${field.label}: ${DATE_PRESETS.find(([id]) => id === draft.preset)?.[1] ?? draft.preset}`;
  }
  if (draft.op === "empty" || draft.op === "not_empty") {
    return `${field.label}: ${OP_LABEL[draft.op]}`;
  }
  return `${field.label}${draft.value ? `: ${draft.value}` : ""}`;
}

export function FilterBar({
  fields,
  entities,
  params,
  onChange,
  onReplace,
}: {
  entity: string;
  fields: UiField[];
  entities: UiEntity[];
  params: URLSearchParams;
  onChange: (key: string, value: string) => void;
  onReplace?: (next: URLSearchParams) => void;
}) {
  const [drafts, setDrafts] = useState<Draft[]>(() => readDrafts(fields, params));
  const [pickerOpen, setPickerOpen] = useState(false);
  const [editing, setEditing] = useState<Draft | null>(null);
  const popover = useRef<HTMLDivElement>(null);
  const popoverId = useId();
  const applied = drafts.filter(draftIsComplete);
  const popoverOpen = pickerOpen || Boolean(editing);

  useEffect(() => {
    setDrafts(readDrafts(fields, params));
  }, [fields, params]);

  useEffect(() => {
    if (!popoverOpen) return;
    function onPointer(event: MouseEvent) {
      if (!popover.current?.contains(event.target as Node)) closePopover();
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") closePopover();
    }
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [popoverOpen]);

  const enumFields = useMemo(
    () => fields.filter((f) => f.enum_values && f.enum_values.length > 0),
    [fields],
  );

  function closePopover() {
    setPickerOpen(false);
    setEditing(null);
  }

  function apply(nextDrafts = drafts) {
    if (onReplace) {
      onReplace(writeFilters(params, fields, nextDrafts));
      return;
    }
    for (const draft of nextDrafts) {
      const field = fields.find((f) => f.name === draft.field);
      if (!field) continue;
      if (draft.preset) {
        const range = datePresetRange(draft.preset);
        if (range) onChange(`${field.name}.between`, `${range.from},${range.to}`);
        continue;
      }
      const value = draft.op === "between" ? `${draft.value},${draft.value2 ?? ""}` : draft.value;
      onChange(draft.op === "eq" ? field.name : `${field.name}.${draft.op}`, value);
    }
  }

  function commit(draft: Draft, close = true) {
    const next = drafts.filter((d) => d.field !== draft.field);
    if (draftIsComplete(draft)) next.push(draft);
    setDrafts(next);
    apply(next);
    if (close) closePopover();
  }

  function reset() {
    setDrafts([]);
    closePopover();
    if (onReplace) {
      onReplace(writeFilters(params, fields, []));
    } else {
      for (const field of fields) {
        onChange(field.name, "");
        onChange(`${field.name}.op`, "");
        onChange(`${field.name}.preset`, "");
        for (const op of OPS[field.type] ?? []) onChange(`${field.name}.${op}`, "");
      }
    }
  }

  function removeApplied(fieldName: string) {
    const next = drafts.filter((d) => d.field !== fieldName);
    setDrafts(next);
    apply(next);
  }

  function startField(field: UiField) {
    const existing = drafts.find((d) => d.field === field.name);
    setEditing(existing ?? { field: field.name, op: (OPS[field.type] ?? ["eq"])[0], value: "" });
    setPickerOpen(false);
  }

  return (
    <div className={`filters list-filters${applied.length ? " has-filters" : ""}`}>
      {applied.length > 0 ? (
        <div className="chip-row filter-chips" aria-label="Active filters">
          {applied.map((draft) => {
            const field = fields.find((f) => f.name === draft.field);
            if (!field) return null;
            return (
              <span key={`${draft.field}-${draft.op}-${draft.value}-${draft.preset ?? ""}`} className="chip is-active">
                <button
                  type="button"
                  className="chip-action"
                  onClick={() => startField(field)}
                >
                  {chipLabel(field, draft)}
                </button>
                <button
                  type="button"
                  className="chip-remove"
                  aria-label={`Clear ${field.label} filter`}
                  onClick={() => removeApplied(draft.field)}
                >
                  ×
                </button>
              </span>
            );
          })}
          <button type="button" className="ghost filter-reset" onClick={reset}>
            Reset
          </button>
        </div>
      ) : null}
      <div className="filter-popover add-filter" ref={popover}>
        <button
          type="button"
          className={applied.length || popoverOpen ? "tonal is-active" : "ghost"}
          aria-expanded={popoverOpen}
          aria-controls={popoverId}
          aria-haspopup="dialog"
          onClick={() => {
            if (popoverOpen) closePopover();
            else {
              setEditing(null);
              setPickerOpen(true);
            }
          }}
        >
          + Add filter
        </button>
        {popoverOpen ? (
          <div id={popoverId} className="filter-popover-panel" role="dialog" aria-label="Add filter">
            {editing ? (
              <FilterEditor
                fields={fields}
                entities={entities}
                draft={editing}
                onChange={setEditing}
                onApply={() => commit(editing)}
                onBack={() => {
                  setEditing(null);
                  setPickerOpen(true);
                }}
                onImplicit={(next) => {
                  setEditing(next);
                  if (draftIsComplete(next)) commit(next);
                }}
              />
            ) : (
              <div className="filter-picker">
                {enumFields.length > 0 ? (
                  <div className="quick-filters" aria-label="Quick filters">
                    {enumFields.slice(0, 2).map((field) => (
                      <div key={field.name} className="chip-row">
                        <span className="muted">{field.label}</span>
                        {(field.enum_values ?? []).map((v) => {
                          const active = params.get(field.name) === v;
                          return (
                            <button
                              key={v}
                              type="button"
                              className={active ? "chip is-active" : "chip chip-quiet"}
                              aria-pressed={active}
                              onClick={() => {
                                onChange(field.name, active ? "" : v);
                                closePopover();
                              }}
                            >
                              {v}
                            </button>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                ) : null}
                <ul className="option-list" role="listbox">
                  {fields.map((field) => (
                    <li key={field.name}>
                      <button type="button" className="ghost" onClick={() => startField(field)}>
                        {field.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function FilterEditor({
  fields,
  entities,
  draft,
  onChange,
  onApply,
  onBack,
  onImplicit,
}: {
  fields: UiField[];
  entities: UiEntity[];
  draft: Draft;
  onChange: (next: Draft) => void;
  onApply: () => void;
  onBack: () => void;
  onImplicit: (next: Draft) => void;
}) {
  const field = fields.find((f) => f.name === draft.field);
  if (!field) return null;
  const ops = OPS[field.type] ?? ["eq", "contains"];
  const isDate = field.type === "date" || field.type === "datetime";
  const skipValue = draft.op === "empty" || draft.op === "not_empty" || Boolean(draft.preset);

  function patch(partial: Partial<Draft>, implicit = false) {
    const next = { ...draft, ...partial };
    if (implicit) onImplicit(next);
    else onChange(next);
  }

  return (
    <div className="filter-editor">
      <div className="filter-editor-head">
        <strong>{field.label}</strong>
        <button type="button" className="ghost filter-editor-back" onClick={onBack}>
          Back
        </button>
      </div>
      <label>
        Condition
        <select
          value={draft.op}
          aria-label={`${field.label} operator`}
          onChange={(e) => {
            const op = e.target.value;
            const next = { ...draft, op, preset: op === "empty" || op === "not_empty" ? "" : draft.preset };
            if (op === "empty" || op === "not_empty") onImplicit(next);
            else onChange(next);
          }}
        >
          {ops.map((o) => (
            <option key={o} value={o}>
              {OP_LABEL[o] ?? o}
            </option>
          ))}
        </select>
      </label>
      {isDate ? (
        <label>
          Preset
          <select
            aria-label={`${field.label} preset`}
            value={draft.preset ?? ""}
            onChange={(e) => {
              const preset = e.target.value;
              onImplicit({ ...draft, preset, op: preset ? "between" : draft.op === "between" ? "eq" : draft.op });
            }}
          >
            <option value="">Custom</option>
            {DATE_PRESETS.map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {skipValue ? null : field.enum_values ? (
        <label>
          Value
          <select
            value={draft.value}
            onChange={(e) => patch({ value: e.target.value }, Boolean(e.target.value))}
          >
            <option value="">Any</option>
            {field.enum_values.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
        </label>
      ) : field.relation ? (
        renderWidget({
          field: { ...field, widget: "relation", required: false },
          value: draft.value,
          entities,
          onChange: (v) => patch({ value: v == null ? "" : String(v) }, v != null && v !== ""),
        })
      ) : (
        <span className="filter-values">
          <input
            type={
              field.type === "date"
                ? "date"
                : field.type === "integer" || field.type === "decimal"
                  ? "number"
                  : "text"
            }
            value={draft.value}
            aria-label={`${field.label} value`}
            onChange={(e) => onChange({ ...draft, value: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                onApply();
              }
            }}
          />
          {draft.op === "between" ? (
            <input
              type={field.type === "date" ? "date" : field.type === "integer" || field.type === "decimal" ? "number" : "text"}
              value={draft.value2 ?? ""}
              aria-label={`${field.label} value to`}
              onChange={(e) => onChange({ ...draft, value2: e.target.value })}
            />
          ) : null}
        </span>
      )}
      <div className="filter-editor-ops">
        <button type="button" onClick={onApply}>
          Apply
        </button>
      </div>
    </div>
  );
}

export function SavedViewsMenu({
  entity,
  params,
  canSave,
  onChange,
  onReplace,
}: {
  entity: string;
  params: URLSearchParams;
  canSave: boolean;
  onChange: (key: string, value: string) => void;
  onReplace?: (next: URLSearchParams) => void;
}) {
  const [saved, setSaved] = useState<Array<{ id: string; name: string; query?: Record<string, unknown> }>>([]);
  const [saveName, setSaveName] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.savedFilters(entity).then((d) => setSaved(d.items)).catch(() => setSaved([]));
  }, [entity]);

  async function save() {
    if (!saveName.trim()) return;
    const query: Record<string, string> = {};
    params.forEach((v, k) => {
      if (k !== "page" && !k.endsWith(".op") && !k.endsWith(".preset")) query[k] = v;
    });
    setBusy(true);
    try {
      await api.saveFilter(entity, saveName.trim(), query);
      setSaveName("");
      const next = await api.savedFilters(entity);
      setSaved(next.items);
    } finally {
      setBusy(false);
    }
  }

  function load(id: string) {
    const item = saved.find((s) => s.id === id);
    if (!item) return;
    api.savedFilters(entity).then((d) => {
      const full = d.items.find((s) => s.id === item.id);
      const query = (full?.query ?? item.query ?? {}) as Record<string, string>;
      if (onReplace) {
        const next = new URLSearchParams();
        for (const [k, v] of Object.entries(query)) next.set(k, String(v ?? ""));
        next.set("page", "1");
        onReplace(next);
        return;
      }
      for (const [k, v] of Object.entries(query)) onChange(k, String(v ?? ""));
    });
  }

  return (
    <div className="saved-views-menu">
      <div className="palette-heading">Saved views</div>
      {saved.length === 0 ? (
        <p className="muted saved-views-empty">No saved views</p>
      ) : (
        <ul className="saved-views-list">
          {saved.map((s) => (
            <li key={s.id}>
              <button type="button" className="ghost" onClick={() => load(s.id)}>
                {s.name}
              </button>
            </li>
          ))}
        </ul>
      )}
      {canSave ? (
        <div className="saved-views-save">
          <input
            placeholder="Save as…"
            value={saveName}
            aria-label="Save view as"
            onChange={(e) => setSaveName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void save();
              }
            }}
          />
          <button type="button" className="ghost" disabled={busy || !saveName.trim()} onClick={() => void save()}>
            Save
          </button>
        </div>
      ) : (
        <p className="muted saved-views-hint">Apply a search or filter to save a view.</p>
      )}
    </div>
  );
}
