import { useEffect, useState } from "react";
import { api, type UiField } from "../../api";
import { renderWidget } from "../../metadata/registry";
import type { UiEntity } from "../../metadata/types";

const OPS: Record<string, string[]> = {
  string: ["eq", "neq", "contains", "starts_with", "empty", "not_empty"],
  text: ["contains", "starts_with", "empty", "not_empty"],
  integer: ["eq", "neq", "gt", "lt", "between", "empty", "not_empty"],
  decimal: ["eq", "neq", "gt", "lt", "between", "empty", "not_empty"],
  date: ["eq", "gt", "lt", "between", "empty"],
  time: ["eq", "gt", "lt"],
  datetime: ["eq", "gt", "lt", "between"],
  boolean: ["eq"],
  enum: ["eq", "neq", "in"],
  relation: ["eq", "empty", "not_empty"],
  uuid: ["eq"],
  json: ["empty", "not_empty"],
};

export function FilterBar({
  entity,
  fields,
  entities,
  params,
  onChange,
}: {
  entity: string;
  fields: UiField[];
  entities: UiEntity[];
  params: URLSearchParams;
  onChange: (key: string, value: string) => void;
}) {
  const [saved, setSaved] = useState<Array<{ id: string; name: string }>>([]);
  const [saveName, setSaveName] = useState("");

  useEffect(() => {
    api.savedFilters(entity).then((d) => setSaved(d.items)).catch(() => setSaved([]));
  }, [entity]);

  async function save() {
    if (!saveName.trim()) return;
    const query: Record<string, string> = {};
    params.forEach((v, k) => {
      if (k !== "page") query[k] = v;
    });
    await api.saveFilter(entity, saveName.trim(), query);
    setSaveName("");
    const next = await api.savedFilters(entity);
    setSaved(next.items);
  }

  return (
    <div className="filters">
      {fields.map((field) => {
        const ops = OPS[field.type] ?? ["eq", "contains"];
        const op = params.get(`${field.name}.op`) ?? (field.enum_values ? "eq" : ops[0]);
        const value = params.get(op === "eq" ? field.name : `${field.name}.${op}`) ?? params.get(field.name) ?? "";
        return (
          <label key={field.name}>
            {field.label}
            {ops.length > 1 && (
              <select
                value={op}
                aria-label={`${field.label} operator`}
                onChange={(e) => onChange(`${field.name}.op`, e.target.value)}
              >
                {ops.map((o) => (
                  <option key={o} value={o}>
                    {o.replace("_", " ")}
                  </option>
                ))}
              </select>
            )}
            {op === "empty" || op === "not_empty" ? (
              <input
                type="checkbox"
                checked={value === "1"}
                onChange={(e) => onChange(`${field.name}.${op}`, e.target.checked ? "1" : "")}
              />
            ) : field.enum_values ? (
              <select value={value} onChange={(e) => onChange(field.name, e.target.value)}>
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
                value,
                entities,
                onChange: (v) => onChange(field.name, v == null ? "" : String(v)),
              })
            ) : (
              <input
                type={field.type === "date" ? "date" : field.type === "integer" || field.type === "decimal" ? "number" : "text"}
                value={value}
                onChange={(e) =>
                  onChange(op === "eq" ? field.name : `${field.name}.${op}`, e.target.value)
                }
              />
            )}
          </label>
        );
      })}
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
              for (const [k, v] of Object.entries(query)) onChange(k, String(v ?? ""));
            });
          }}
        >
          <option value="">Saved filters</option>
          {saved.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <input
          placeholder="Save as…"
          value={saveName}
          onChange={(e) => setSaveName(e.target.value)}
        />
        <button type="button" className="ghost" onClick={() => void save()}>
          Save filter
        </button>
      </div>
    </div>
  );
}
