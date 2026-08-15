import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import { api, type UiEntity, type UiField } from "./api";

export type WidgetProps = {
  field: UiField;
  value: unknown;
  onChange: (value: unknown) => void;
  entities: UiEntity[];
  disabled?: boolean;
};

type Widget = (props: WidgetProps) => ReactNode;

const registry: Record<string, Widget> = {};

export function registerWidget(name: string, widget: Widget) {
  registry[name] = widget;
}

export function renderWidget(props: WidgetProps) {
  const key = String(props.field.widget || props.field.type || "text").toLowerCase();
  const Widget = registry[key] || registry.text;
  return Widget(props);
}

function TextWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <input
      type="text"
      placeholder={field.placeholder ?? ""}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function EmailWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <input
      type="email"
      placeholder={field.placeholder ?? "name@example.com"}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function TextareaWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <textarea
      placeholder={field.placeholder ?? ""}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function NumberWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <input
      type="number"
      step={field.type === "integer" ? "1" : "0.01"}
      value={value == null || value === "" ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
    />
  );
}

function BooleanWidget({ value, onChange, disabled }: WidgetProps) {
  return (
    <input
      type="checkbox"
      checked={Boolean(value)}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
    />
  );
}

function DateWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <input
      type="date"
      value={value == null ? "" : String(value).slice(0, 10)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function DateTimeWidget({ field, value, onChange, disabled }: WidgetProps) {
  const local = value ? String(value).slice(0, 16) : "";
  return (
    <input
      type="datetime-local"
      value={local}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

function SelectWidget({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <select
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    >
      <option value="">Select</option>
      {(field.enum_values ?? []).map((v) => (
        <option key={v} value={v}>
          {v}
        </option>
      ))}
    </select>
  );
}

function RelationWidget({ field, value, onChange, entities, disabled }: WidgetProps) {
  const target = entities.find((e) => e.entity === field.relation);
  const [q, setQ] = useState("");
  const [options, setOptions] = useState<Array<{ id: string; label: string }>>([]);
  const displayField = target?.display_field || "name";
  const selected = value == null || value === "" ? "" : String(value);

  useEffect(() => {
    if (!target) return;
    const params = new URLSearchParams();
    if (q) params.set("search", q);
    params.set("page_size", "25");
    api
      .list(target.slug, params)
      .then((page) => {
        setOptions((prev) => {
          const next = page.items.map((row) => ({
            id: String(row.id),
            label: String(row[displayField] ?? row.name ?? row.title ?? row.code ?? row.id),
          }));
          const keep = prev.find((o) => o.id === selected && !next.some((n) => n.id === o.id));
          return keep ? [keep, ...next] : next;
        });
      })
      .catch(() => setOptions([]));
  }, [target, q, displayField, selected]);

  useEffect(() => {
    if (!target || !selected) return;
    api
      .get(target.slug, selected)
      .then((row) => {
        const label = String(row[displayField] ?? row.name ?? row.title ?? row.code ?? row.id);
        setOptions((prev) =>
          prev.some((o) => o.id === selected) ? prev : [{ id: selected, label }, ...prev],
        );
      })
      .catch(() => undefined);
  }, [target, selected, displayField]);

  const current = useMemo(
    () => options.find((o) => o.id === String(value ?? ""))?.label,
    [options, value],
  );

  if (!target) {
    return (
      <input
        value={value == null ? "" : String(value)}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }

  return (
    <div>
      <input
        placeholder={`Search ${target.label_plural}`}
        value={q}
        disabled={disabled}
        onChange={(e) => setQ(e.target.value)}
      />
      <select
        value={value == null ? "" : String(value)}
        disabled={disabled}
        required={field.required}
        onChange={(e) => onChange(e.target.value || null)}
      >
        <option value="">{current ? current : `Select ${target.label}`}</option>
        {options.map((o) => (
          <option key={o.id} value={o.id}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

registerWidget("text", TextWidget);
registerWidget("textarea", TextareaWidget);
registerWidget("email", EmailWidget);
registerWidget("number", NumberWidget);
registerWidget("boolean", BooleanWidget);
registerWidget("checkbox", BooleanWidget);
registerWidget("date", DateWidget);
registerWidget("datetime", DateTimeWidget);
registerWidget("select", SelectWidget);
registerWidget("relation", RelationWidget);
registerWidget("json", TextareaWidget);
registerWidget("string", TextWidget);
registerWidget("integer", NumberWidget);
registerWidget("decimal", NumberWidget);
registerWidget("enum", SelectWidget);
