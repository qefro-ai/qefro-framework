import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { api } from "../../api";
import { useTenantTheme } from "../../metadata/context";
import { registerWidget, renderWidget, type WidgetProps } from "../../metadata/registry";
import { localToUtcIso, utcToLocalParts, formatMoney } from "../../metadata/timezone";
import { previewFormula } from "../../metadata/formula";

function opt(field: WidgetProps["field"]) {
  return field.widget_options ?? {};
}

function described(field: WidgetProps["field"]) {
  return field.help || field.help_text || field.description || undefined;
}

export function TextInput({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <input
      id={id}
      type="text"
      placeholder={field.placeholder ?? ""}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      readOnly={field.readonly}
      required={field.required}
      maxLength={undefined}
      aria-required={field.required}
      aria-describedby={described(field) ? `${field.name}-help` : undefined}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function Textarea({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <textarea
      id={id}
      rows={4}
      placeholder={field.placeholder ?? ""}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      readOnly={field.readonly}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function EmailInput({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <input
      id={id}
      type="email"
      inputMode="email"
      autoComplete="email"
      placeholder={field.placeholder ?? "name@example.com"}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function PhoneInput({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <input
      id={id}
      type="tel"
      inputMode="tel"
      autoComplete="tel"
      placeholder={field.placeholder ?? "+91 98765 43210"}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function UrlInput({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <input
      id={id}
      type="url"
      inputMode="url"
      placeholder={field.placeholder ?? "https://"}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function NumberInput({ field, value, onChange, disabled, id }: WidgetProps) {
  const options = opt(field);
  const integer = field.type === "integer";
  return (
    <input
      id={id}
      type="number"
      step={options.step ?? (integer ? 1 : 0.01)}
      min={options.min as number | undefined}
      max={options.max as number | undefined}
      value={value == null || value === "" ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
    />
  );
}

export function CurrencyInput({ field, value, onChange, disabled, id }: WidgetProps) {
  const theme = useTenantTheme();
  const options = opt(field);
  const currency = options.currency || theme.currency || "USD";
  const precision = options.precision ?? 2;
  return (
    <div className="widget-affix">
      <span className="affix" aria-hidden>
        {currency}
      </span>
      <input
        id={id}
        type="number"
        step={1 / 10 ** precision}
        min={options.min as number | undefined}
        max={options.max as number | undefined}
        value={value == null || value === "" ? "" : String(value)}
        disabled={disabled}
        required={field.required}
        aria-label={`${field.label} (${currency})`}
        onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
      />
      <span className="muted preview">
        {value === "" || value == null ? "" : formatMoney(value, currency, theme.locale, precision)}
      </span>
    </div>
  );
}

export function PercentageInput({ field, value, onChange, disabled, id }: WidgetProps) {
  const options = opt(field);
  const precision = options.precision ?? 2;
  return (
    <div className="widget-affix">
      <input
        id={id}
        type="number"
        min={0}
        max={100}
        step={1 / 10 ** precision}
        value={value == null || value === "" ? "" : String(value)}
        disabled={disabled}
        required={field.required}
        onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))}
      />
      <span className="affix" aria-hidden>
        %
      </span>
    </div>
  );
}

export function DatePicker({ field, value, onChange, disabled, id }: WidgetProps) {
  const options = opt(field);
  const min = options.min != null ? String(options.min).slice(0, 10) : undefined;
  const max = options.max != null ? String(options.max).slice(0, 10) : undefined;
  return (
    <input
      id={id}
      type="date"
      value={value == null ? "" : String(value).slice(0, 10)}
      min={min}
      max={max}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value || null)}
    />
  );
}

export function TimePicker({ field, value, onChange, disabled, id }: WidgetProps) {
  const theme = useTenantTheme();
  const options = opt(field);
  const step = (options.minute_step ?? 1) * 60;
  const raw = value == null ? "" : String(value).slice(0, 5);
  return (
    <input
      id={id}
      type="time"
      step={step}
      value={raw}
      min={options.min != null ? String(options.min) : undefined}
      max={options.max != null ? String(options.max) : undefined}
      disabled={disabled}
      required={field.required}
      aria-label={`${field.label} (${theme.hour12 ? "12-hour" : "24-hour"})`}
      onChange={(e) => onChange(e.target.value || null)}
    />
  );
}

export function DateTimePicker({ field, value, onChange, disabled, id }: WidgetProps) {
  const theme = useTenantTheme();
  const options = opt(field);
  const tz =
    options.timezone === "utc" ? "UTC" : options.timezone && options.timezone !== "tenant"
      ? options.timezone
      : theme.timezone;
  const parts = utcToLocalParts(value, tz);
  function emit(date: string, time: string) {
    onChange(localToUtcIso(date, time, tz));
  }
  return (
    <div className="widget-datetime">
      <input
        id={id}
        type="date"
        value={parts.date}
        disabled={disabled}
        required={field.required}
        aria-label={`${field.label} date`}
        onChange={(e) => emit(e.target.value, parts.time || "00:00")}
      />
      <input
        type="time"
        value={parts.time}
        disabled={disabled}
        required={field.required}
        aria-label={`${field.label} time (${tz})`}
        onChange={(e) => emit(parts.date, e.target.value)}
      />
      <span className="muted">{tz}</span>
    </div>
  );
}

export function ColorPicker({ field, value, onChange, disabled, id }: WidgetProps) {
  const hex = normalizeHex(value);
  return (
    <div className="widget-color">
      <input
        id={id}
        type="color"
        value={hex || "#2563eb"}
        disabled={disabled}
        aria-label={`${field.label} color`}
        onChange={(e) => onChange(e.target.value)}
      />
      <input
        type="text"
        placeholder="#2563eb"
        value={value == null ? "" : String(value)}
        disabled={disabled}
        required={field.required}
        pattern="^#([0-9A-Fa-f]{3}|[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$|^rgb"
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function normalizeHex(value: unknown): string {
  const s = String(value ?? "");
  if (/^#[0-9A-Fa-f]{6}$/.test(s)) return s;
  if (/^#[0-9A-Fa-f]{3}$/.test(s)) {
    return `#${s[1]}${s[1]}${s[2]}${s[2]}${s[3]}${s[3]}`;
  }
  return "";
}

export function Select({ field, value, onChange, disabled, id }: WidgetProps) {
  const options = field.enum_values ?? [];
  return (
    <select
      id={id}
      value={value == null ? "" : String(value)}
      disabled={disabled}
      required={field.required}
      onChange={(e) => onChange(e.target.value || null)}
    >
      <option value="">{field.placeholder ?? "Select"}</option>
      {options.map((v) => (
        <option key={v} value={v}>
          {v}
        </option>
      ))}
    </select>
  );
}

export function MultiSelect({ field, value, onChange, disabled, id }: WidgetProps) {
  const options = field.enum_values ?? [];
  const selected = Array.isArray(value) ? value.map(String) : [];
  const max = opt(field).max_selections;
  const [q, setQ] = useState("");
  const filtered = options.filter((o) => o.toLowerCase().includes(q.toLowerCase()));
  function toggle(v: string) {
    if (disabled) return;
    if (selected.includes(v)) onChange(selected.filter((s) => s !== v));
    else if (!max || selected.length < max) onChange([...selected, v]);
  }
  return (
    <div className="multiselect" id={id}>
      <input
        placeholder="Search…"
        value={q}
        disabled={disabled}
        onChange={(e) => setQ(e.target.value)}
        aria-label={`Search ${field.label}`}
      />
      <div className="chips">
        {selected.map((v) => (
          <button type="button" key={v} className="chip" disabled={disabled} onClick={() => toggle(v)}>
            {v} ×
          </button>
        ))}
      </div>
      <ul className="option-list" role="listbox" aria-multiselectable>
        {filtered.map((v) => (
          <li key={v}>
            <label>
              <input
                type="checkbox"
                checked={selected.includes(v)}
                disabled={disabled}
                onChange={() => toggle(v)}
              />
              {v}
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function Checkbox({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <label className="inline-check">
      <input
        id={id}
        type="checkbox"
        checked={Boolean(value)}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{field.label}</span>
    </label>
  );
}

export function Switch({ field, value, onChange, disabled, id }: WidgetProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={Boolean(value)}
      aria-label={field.label}
      className={`switch ${value ? "on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!value)}
    >
      <span className="knob" />
    </button>
  );
}

export function Radio({ field, value, onChange, disabled }: WidgetProps) {
  return (
    <div role="radiogroup" aria-label={field.label} className="radio-group">
      {(field.enum_values ?? []).map((v) => (
        <label key={v} className="inline-check">
          <input
            type="radio"
            name={field.name}
            value={v}
            checked={String(value ?? "") === v}
            disabled={disabled}
            onChange={() => onChange(v)}
          />
          {v}
        </label>
      ))}
    </div>
  );
}

export function TagsInput({ field, value, onChange, disabled, id }: WidgetProps) {
  const tags = Array.isArray(value) ? value.map(String) : [];
  const [draft, setDraft] = useState("");
  function add(raw: string) {
    const tag = raw.trim();
    setDraft("");
    if (!tag || tags.includes(tag) || disabled) return;
    onChange([...tags, tag]);
  }
  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add(draft);
    }
    if (e.key === "Backspace" && !draft && tags.length) {
      onChange(tags.slice(0, -1));
    }
  }
  return (
    <div className="tags" id={id}>
      {tags.map((t) => (
        <button
          type="button"
          key={t}
          className="chip"
          disabled={disabled}
          onClick={() => onChange(tags.filter((x) => x !== t))}
        >
          {t} ×
        </button>
      ))}
      <input
        value={draft}
        disabled={disabled}
        placeholder={field.placeholder ?? "Add tag"}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={onKey}
        onBlur={() => add(draft)}
        aria-label={field.label}
      />
    </div>
  );
}

export function JsonEditor({ field, value, onChange, disabled, id }: WidgetProps) {
  const text =
    typeof value === "string" ? value : value == null ? "" : JSON.stringify(value, null, 2);
  const [error, setError] = useState("");
  function apply(raw: string) {
    if (!raw.trim()) {
      setError("");
      onChange(null);
      return;
    }
    try {
      onChange(JSON.parse(raw));
      setError("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Invalid JSON");
      onChange(raw);
    }
  }
  return (
    <div>
      <textarea
        id={id}
        rows={8}
        className="mono"
        value={text}
        disabled={disabled}
        required={field.required}
        onChange={(e) => apply(e.target.value)}
      />
      <button
        type="button"
        className="ghost"
        disabled={disabled}
        onClick={() => {
          try {
            onChange(JSON.parse(text));
            setError("");
          } catch (err) {
            setError(err instanceof Error ? err.message : "Invalid JSON");
          }
        }}
      >
        Format
      </button>
      {error && (
        <span className="error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

export function RichText({ field, value, onChange, disabled, id }: WidgetProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (ref.current && ref.current.innerHTML !== String(value ?? "")) {
      ref.current.innerHTML = String(value ?? "");
    }
  }, [value]);
  function cmd(command: string) {
    document.execCommand(command);
    onChange(ref.current?.innerHTML ?? "");
  }
  return (
    <div className="rich-text">
      <div className="rich-toolbar" role="toolbar" aria-label="Formatting">
        <button type="button" className="ghost" disabled={disabled} onClick={() => cmd("bold")}>
          B
        </button>
        <button type="button" className="ghost" disabled={disabled} onClick={() => cmd("italic")}>
          I
        </button>
        <button type="button" className="ghost" disabled={disabled} onClick={() => cmd("underline")}>
          U
        </button>
        <button type="button" className="ghost" disabled={disabled} onClick={() => cmd("insertUnorderedList")}>
          List
        </button>
        <button type="button" className="ghost" disabled={disabled} onClick={() => {
          document.execCommand("formatBlock", false, "h3");
          onChange(ref.current?.innerHTML ?? "");
        }}>
          H
        </button>
        <button
          type="button"
          className="ghost"
          disabled={disabled}
          onClick={() => {
            const url = window.prompt("Link URL");
            if (url) document.execCommand("createLink", false, url);
            onChange(ref.current?.innerHTML ?? "");
          }}
        >
          Link
        </button>
        <button type="button" className="ghost" disabled={disabled} onClick={() => {
          document.execCommand("formatBlock", false, "blockquote");
          onChange(ref.current?.innerHTML ?? "");
        }}>
          Quote
        </button>
      </div>
      <div
        id={id}
        ref={ref}
        className="rich-surface"
        contentEditable={!disabled}
        role="textbox"
        aria-multiline
        aria-label={field.label}
        onInput={() => onChange(ref.current?.innerHTML ?? "")}
      />
    </div>
  );
}

function FileWidget({ field, value, onChange, disabled, id, image }: WidgetProps & { image?: boolean }) {
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState("");
  const key = value == null ? "" : String(value);
  const url = key ? `/api/v1/files/${encodeURIComponent(key)}` : "";
  const max = opt(field).max_size ?? 8 * 1024 * 1024;
  const accept = (opt(field).accept ?? (image ? ["image/*"] : [])).join(",");

  async function upload(file: File) {
    if (file.size > max) {
      setError(`File exceeds ${(max / 1024 / 1024).toFixed(1)}MB`);
      return;
    }
    setError("");
    setProgress(0);
    try {
      const meta = await api.upload(file, image ? "image" : "file", setProgress);
      onChange(meta.key);
    } catch (err) {
      setError(err instanceof Error ? err.message : "upload failed");
    } finally {
      setProgress(null);
    }
  }

  return (
    <div className="file-widget" id={id}>
      {image && url ? <img src={url} alt="" className="image-preview" /> : null}
      {key && !image ? <p className="muted">{key}</p> : null}
      <input
        type="file"
        accept={accept || undefined}
        disabled={disabled}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void upload(file);
        }}
      />
      {progress != null && (
        <progress value={progress} max={1}>
          {Math.round(progress * 100)}%
        </progress>
      )}
      {key && (
        <button type="button" className="ghost" disabled={disabled} onClick={() => onChange(null)}>
          Remove
        </button>
      )}
      {error && (
        <span className="error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

export function FileUpload(props: WidgetProps) {
  return <FileWidget {...props} />;
}

export function ImageUpload(props: WidgetProps) {
  return <FileWidget {...props} image />;
}

export function RelationPicker({ field, value, onChange, entities, disabled, id }: WidgetProps) {
  const targetName = field.relation || opt(field).entity;
  const target = entities.find((e) => e.entity === targetName);
  const [q, setQ] = useState("");
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<Array<{ id: string; label: string }>>([]);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const selected = value == null || value === "" ? "" : String(value);
  const displayField = opt(field).display_field || target?.display_field || "name";
  const current = options.find((o) => o.id === selected);

  useEffect(() => {
    if (!target) return;
    const handle = window.setTimeout(() => {
      const params = new URLSearchParams();
      if (q) params.set("search", q);
      params.set("page", String(page));
      params.set("page_size", "20");
      api
        .list(target.slug, params)
        .then((result) => {
          setTotal(result.total);
          setOptions((prev) => {
            const next = result.items.map((row) => ({
              id: String(row.id),
              label: String(row[displayField] ?? row.name ?? row.title ?? row.code ?? row.id),
            }));
            const keep = prev.find((o) => o.id === selected && !next.some((n) => n.id === o.id));
            return keep ? [keep, ...next] : next;
          });
        })
        .catch(() => setOptions([]));
    }, 250);
    return () => window.clearTimeout(handle);
  }, [target, q, page, displayField, selected]);

  useEffect(() => {
    if (!target || !selected) return;
    api
      .get(target.slug, selected)
      .then((row) => {
        const label = String(row[displayField] ?? row.name ?? row.title ?? row.code ?? row.id);
        setOptions((prev) => (prev.some((o) => o.id === selected) ? prev : [{ id: selected, label }, ...prev]));
      })
      .catch(() => undefined);
  }, [target, selected, displayField]);

  if (!target) {
    return (
      <input
        id={id}
        value={selected}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value || null)}
      />
    );
  }

  return (
    <div className="relation-picker">
      <div className="relation-control">
        <input
          id={id}
          role="combobox"
          aria-expanded={open}
          aria-autocomplete="list"
          placeholder={`Search ${target.label_plural}`}
          value={open ? q : current?.label ?? ""}
          disabled={disabled}
          onFocus={() => setOpen(true)}
          onChange={(e) => {
            setQ(e.target.value);
            setPage(1);
            setOpen(true);
          }}
        />
        {selected && (
          <button
            type="button"
            className="ghost"
            disabled={disabled}
            aria-label="Clear"
            onClick={() => {
              onChange(null);
              setQ("");
            }}
          >
            ×
          </button>
        )}
      </div>
      {open && (
        <ul className="option-list" role="listbox">
          {options.map((o) => (
            <li key={o.id}>
              <button
                type="button"
                className={o.id === selected ? "active" : "ghost"}
                onClick={() => {
                  onChange(o.id);
                  setOpen(false);
                  setQ("");
                }}
              >
                {o.label}
              </button>
            </li>
          ))}
          {options.length === 0 && <li className="muted">No matches</li>}
          {total > options.length && (
            <li>
              <button type="button" className="ghost" onClick={() => setPage((p) => p + 1)}>
                More
              </button>
            </li>
          )}
        </ul>
      )}
    </div>
  );
}

export function ChildTable({ field, value, onChange, entities, disabled }: WidgetProps) {
  const childName = field.child_entity || field.widget_options?.entity || field.relation;
  const child = entities.find((e) => e.entity === childName);
  const opts = opt(field);
  const rows = Array.isArray(value) ? (value as Record<string, unknown>[]) : [];
  const cols = (child?.fields ?? []).filter((f) => {
    if (f.hidden || f.form === false) return false;
    if (["id", "tenant_id"].includes(f.name)) return false;
    if (f.relation_kind === "one_to_many" || f.relation_kind === "child_table") return false;
    return true;
  });
  const addable = opts.addable !== false && !disabled;
  const deletable = opts.deletable !== false && !disabled;
  const reorderable = opts.reorderable !== false && !disabled;

  function setRow(i: number, name: string, v: unknown) {
    const next = rows.map((row, idx) => (idx === i ? { ...row, [name]: v } : row));
    const formulas = cols.filter((c) => c.computed && c.formula);
    for (const computed of formulas) {
      const preview = previewFormula(computed.formula || "", next[i] ?? {}, {});
      if (preview != null) next[i] = { ...next[i], [computed.name]: preview };
    }
    onChange(next);
  }

  return (
    <div className="child-table">
      <table>
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.name}>{c.label}</th>
            ))}
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr key={String(row.id ?? i)}>
              {cols.map((col) => (
                <td key={col.name}>
                  {renderWidget({
                    field: { ...col, readonly: Boolean(col.computed || col.readonly || disabled) },
                    value: row[col.name],
                    entities,
                    disabled: Boolean(disabled || col.computed || col.readonly),
                    onChange: (v) => setRow(i, col.name, v),
                  })}
                </td>
              ))}
              <td className="child-row-ops">
                {reorderable && (
                  <>
                    <button type="button" className="ghost" onClick={() => onChange(move(rows, i, -1))} disabled={i === 0}>
                      ↑
                    </button>
                    <button
                      type="button"
                      className="ghost"
                      onClick={() => onChange(move(rows, i, 1))}
                      disabled={i === rows.length - 1}
                    >
                      ↓
                    </button>
                  </>
                )}
                {addable && (
                  <button type="button" className="ghost" onClick={() => onChange([...rows.slice(0, i + 1), { ...row, id: undefined }, ...rows.slice(i + 1)])}>
                    Duplicate
                  </button>
                )}
                {deletable && (
                  <button type="button" className="ghost" onClick={() => onChange(rows.filter((_, idx) => idx !== i))}>
                    Delete
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {addable && (
        <button type="button" className="ghost" onClick={() => onChange([...rows, {}])}>
          + Add {child?.label || "row"}
        </button>
      )}
    </div>
  );
}

function move(rows: Record<string, unknown>[], i: number, dir: number) {
  const j = i + dir;
  if (j < 0 || j >= rows.length) return rows;
  const next = rows.slice();
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

export function registerBuiltinWidgets() {
  registerWidget("text", TextInput);
  registerWidget("textarea", Textarea);
  registerWidget("email", EmailInput);
  registerWidget("phone", PhoneInput);
  registerWidget("url", UrlInput);
  registerWidget("number", NumberInput);
  registerWidget("integer", NumberInput);
  registerWidget("decimal", NumberInput);
  registerWidget("currency", CurrencyInput);
  registerWidget("percentage", PercentageInput);
  registerWidget("date", DatePicker);
  registerWidget("time", TimePicker);
  registerWidget("datetime", DateTimePicker);
  registerWidget("color", ColorPicker);
  registerWidget("select", Select);
  registerWidget("enum", Select);
  registerWidget("multiselect", MultiSelect);
  registerWidget("relation", RelationPicker);
  registerWidget("checkbox", Checkbox);
  registerWidget("boolean", Checkbox);
  registerWidget("switch", Switch);
  registerWidget("radio", Radio);
  registerWidget("tags", TagsInput);
  registerWidget("json", JsonEditor);
  registerWidget("rich_text", RichText);
  registerWidget("file", FileUpload);
  registerWidget("image", ImageUpload);
  registerWidget("child_table", ChildTable);
  registerWidget("string", TextInput);
}

registerBuiltinWidgets();
