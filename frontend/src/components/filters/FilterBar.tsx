import { useEffect, useMemo, useState } from "react";
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

export function FilterBar({
  entity,
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
  const [saved, setSaved] = useState<Array<{ id: string; name: string }>>([]);
  const [saveName, setSaveName] = useState("");
  const [drafts, setDrafts] = useState<Draft[]>(() => readDrafts(fields, params));
  const [open, setOpen] = useState(false);

  useEffect(() => {
    api.savedFilters(entity).then((d) => setSaved(d.items)).catch(() => setSaved([]));
  }, [entity]);

  useEffect(() => {
    setDrafts(readDrafts(fields, params));
  }, [fields, params]);

  const enumFields = useMemo(
    () => fields.filter((f) => f.enum_values && f.enum_values.length > 0),
    [fields],
  );

  function apply(nextDrafts = drafts) {
    if (onReplace) {
      const next = new URLSearchParams(params);
      for (const field of fields) {
        next.delete(field.name);
        next.delete(`${field.name}.op`);
        next.delete(`${field.name}.preset`);
        for (const op of OPS[field.type] ?? []) next.delete(`${field.name}.${op}`);
      }
      for (const draft of nextDrafts) {
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
      onReplace(next);
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

  async function save() {
    if (!saveName.trim()) return;
    const query: Record<string, string> = {};
    params.forEach((v, k) => {
      if (k !== "page" && !k.endsWith(".op") && !k.endsWith(".preset")) query[k] = v;
    });
    await api.saveFilter(entity, saveName.trim(), query);
    setSaveName("");
    const next = await api.savedFilters(entity);
    setSaved(next.items);
  }

  return (
    <div className="filters">
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
                    className={active ? "" : "ghost"}
                    onClick={() => onChange(field.name, active ? "" : v)}
                  >
                    {v}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      ) : null}
      {drafts.map((draft, i) => {
        const field = fields.find((f) => f.name === draft.field);
        if (!field) return null;
        const ops = OPS[field.type] ?? ["eq", "contains"];
        const isDate = field.type === "date" || field.type === "datetime";
        return (
          <label key={`${draft.field}-${i}`}>
            {field.label}
            <select
              value={draft.op}
              aria-label={`${field.label} operator`}
              onChange={(e) => {
                const next = drafts.slice();
                next[i] = { ...draft, op: e.target.value };
                setDrafts(next);
              }}
            >
              {ops.map((o) => (
                <option key={o} value={o}>
                  {OP_LABEL[o] ?? o}
                </option>
              ))}
            </select>
            {isDate ? (
              <select
                aria-label={`${field.label} preset`}
                value={draft.preset ?? ""}
                onChange={(e) => {
                  const next = drafts.slice();
                  next[i] = { ...draft, preset: e.target.value, op: "between" };
                  setDrafts(next);
                }}
              >
                <option value="">Custom</option>
                {DATE_PRESETS.map(([id, label]) => (
                  <option key={id} value={id}>
                    {label}
                  </option>
                ))}
              </select>
            ) : null}
            {draft.op === "empty" || draft.op === "not_empty" || draft.preset ? null : field.enum_values ? (
              <select
                value={draft.value}
                onChange={(e) => {
                  const next = drafts.slice();
                  next[i] = { ...draft, value: e.target.value };
                  setDrafts(next);
                }}
              >
                <option value="">Any</option>
                {field.enum_values.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            ) : field.relation ? (
              renderWidget({
                field: { ...field, widget: "relation", required: false },
                value: draft.value,
                entities,
                onChange: (v) => {
                  const next = drafts.slice();
                  next[i] = { ...draft, value: v == null ? "" : String(v) };
                  setDrafts(next);
                },
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
                  onChange={(e) => {
                    const next = drafts.slice();
                    next[i] = { ...draft, value: e.target.value };
                    setDrafts(next);
                  }}
                />
                {draft.op === "between" ? (
                  <input
                    type={field.type === "date" ? "date" : field.type === "integer" || field.type === "decimal" ? "number" : "text"}
                    value={draft.value2 ?? ""}
                    onChange={(e) => {
                      const next = drafts.slice();
                      next[i] = { ...draft, value2: e.target.value };
                      setDrafts(next);
                    }}
                  />
                ) : null}
              </span>
            )}
            <button
              type="button"
              className="ghost"
              aria-label={`Remove ${field.label} filter`}
              onClick={() => setDrafts(drafts.filter((_, idx) => idx !== i))}
            >
              ×
            </button>
          </label>
        );
      })}
      <div className="filter-ops">
        <div className="add-filter">
          <button type="button" className="ghost" onClick={() => setOpen((v) => !v)}>
            + Add filter
          </button>
          {open ? (
            <ul className="option-list" role="listbox">
              {fields.map((field) => (
                <li key={field.name}>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => {
                      setDrafts([...drafts, { field: field.name, op: (OPS[field.type] ?? ["eq"])[0], value: "" }]);
                      setOpen(false);
                    }}
                  >
                    {field.label}
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
        <button type="button" onClick={() => apply()}>
          Apply
        </button>
        <div className="saved-filters">
          <select
            aria-label="Saved filters"
            value=""
            onChange={(e) => {
              const item = saved.find((s) => s.id === e.target.value);
              if (!item) return;
              api.savedFilters(entity).then((d) => {
                const full = d.items.find((s) => s.id === item.id);
                const query = (full?.query ?? {}) as Record<string, string>;
                if (onReplace) {
                  const next = new URLSearchParams();
                  for (const [k, v] of Object.entries(query)) next.set(k, String(v ?? ""));
                  next.set("page", "1");
                  onReplace(next);
                  return;
                }
                for (const [k, v] of Object.entries(query)) onChange(k, String(v ?? ""));
              });
            }}
          >
            <option value="">Saved views</option>
            {saved.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <input placeholder="Save as…" value={saveName} onChange={(e) => setSaveName(e.target.value)} />
          <button type="button" className="ghost" onClick={() => void save()}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
