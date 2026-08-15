import { FormEvent, useEffect, useMemo, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { api, ApiError, expandedLabel, formVisible, listVisible, type UiEntity, type UiField } from "../api";
import { FilterBar } from "../components/filters/FilterBar";
import { FormLayout } from "../components/forms/FormLayout";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";
import { useRealtime } from "../realtime";

export default function EntityList({ entities }: { entities: UiEntity[] }) {
  const { slug } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const [params, setParams] = useSearchParams();
  const search = params.get("search") ?? "";
  const page = Number(params.get("page") ?? "1");
  const sort = params.get("sort") ?? "-created_at";
  const [rows, setRows] = useState<Record<string, unknown>[]>([]);
  const [total, setTotal] = useState(0);
  const [pageSize, setPageSize] = useState(25);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [importOpen, setImportOpen] = useState(false);
  const [tick, setTick] = useState(0);
  const theme = useTenantTheme();

  const filterable = useMemo(
    () => meta?.fields.filter((f) => f.filterable || f.filter) ?? [],
    [meta],
  );

  useEffect(() => {
    if (!slug || !meta || meta.singleton) return;
    const q = new URLSearchParams();
    if (search) q.set("search", search);
    q.set("sort", sort);
    q.set("page", String(page));
    q.set("page_size", "25");
    for (const [key, value] of params.entries()) {
      if (["search", "sort", "page", "page_size"].includes(key)) continue;
      if (key.endsWith(".op")) continue;
      if (value) q.set(key, value);
    }
    setLoading(true);
    api
      .list(slug, q)
      .then((result) => {
        setRows(result.items);
        setTotal(result.total);
        setPageSize(result.page_size);
        setError("");
      })
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [slug, search, sort, page, params, meta, tick]);

  useRealtime({ entity: meta?.entity, enabled: Boolean(meta && !meta.singleton) }, () => {
    setTick((n) => n + 1);
  });

  if (!meta) return <p>Unknown entity.</p>;
  if (meta.singleton) return <SingletonSettings meta={meta} entities={entities} />;
  const cols = meta.fields.filter(listVisible);
  const pages = Math.max(1, Math.ceil(total / pageSize));

  function setParam(key: string, value: string) {
    const next = new URLSearchParams(params);
    if (value) next.set(key, value);
    else next.delete(key);
    if (key !== "page") next.set("page", "1");
    setParams(next);
  }

  function toggleSort(field: UiField) {
    if (!field.sortable && field.name !== "name") return;
    const next = sort === field.name ? `-${field.name}` : field.name;
    setParam("sort", next);
  }

  function toggleAll() {
    if (selected.size === rows.length) setSelected(new Set());
    else setSelected(new Set(rows.map((r) => String(r.id))));
  }

  async function bulkDelete() {
    if (!meta || !confirm(`Delete ${selected.size} records?`)) return;
    for (const id of selected) await api.remove(meta.slug, id);
    setSelected(new Set());
    setParam("page", String(page));
  }

  return (
    <div className="page">
      <div className="row">
        <div>
          <div className="badge">{meta.entity}</div>
          <h2>{meta.label_plural}</h2>
        </div>
        <div className="actions">
          <button type="button" className="ghost" onClick={() => setImportOpen((v) => !v)}>
            Import CSV
          </button>
          <Link to={`/${meta.slug}/new`}>
            <button>New {meta.label}</button>
          </Link>
        </div>
      </div>
      {importOpen ? <ImportPanel slug={meta.slug} onDone={() => setTick((n) => n + 1)} /> : null}
      {meta.searchable && (
        <div className="toolbar">
          <input
            placeholder={`Search ${meta.label_plural.toLowerCase()}`}
            value={search}
            onChange={(e) => setParam("search", e.target.value)}
            aria-label={`Search ${meta.label_plural}`}
          />
        </div>
      )}
      {filterable.length > 0 && (
        <FilterBar
          entity={meta.entity}
          fields={filterable}
          entities={entities}
          params={params}
          onChange={setParam}
        />
      )}
      {error && (
        <p className="error" role="alert">
          Unable to load {meta.label_plural.toLowerCase()}. {error}
        </p>
      )}
      {selected.size > 0 && (
        <div className="actions">
          <span className="muted">{selected.size} selected</span>
          <button className="danger" onClick={() => void bulkDelete()}>
            Delete selected
          </button>
        </div>
      )}
      <div className="panel table-wrap">
        {loading ? (
          <div className="empty">Loading {meta.label_plural.toLowerCase()}…</div>
        ) : rows.length === 0 && !error ? (
          <div className="empty">No {meta.label_plural.toLowerCase()} found.</div>
        ) : (
          <table>
            <thead>
              <tr>
                <th>
                  <input
                    type="checkbox"
                    aria-label="Select all"
                    checked={rows.length > 0 && selected.size === rows.length}
                    onChange={toggleAll}
                  />
                </th>
                {cols.map((c) => (
                  <th
                    key={c.name}
                    onClick={() => toggleSort(c)}
                    style={{ cursor: c.sortable ? "pointer" : undefined, width: c.width }}
                  >
                    {c.label}
                    {sort === c.name ? " ↑" : sort === `-${c.name}` ? " ↓" : ""}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={String(row.id)}>
                  <td>
                    <input
                      type="checkbox"
                      aria-label="Select row"
                      checked={selected.has(String(row.id))}
                      onChange={() => {
                        const next = new Set(selected);
                        const id = String(row.id);
                        if (next.has(id)) next.delete(id);
                        else next.add(id);
                        setSelected(next);
                      }}
                    />
                  </td>
                  {cols.map((c, i) => (
                    <td key={c.name}>
                      {i === 0 ? (
                        <Link to={`/${meta.slug}/${row.id}`}>{fmtCell(row, c, theme)}</Link>
                      ) : (
                        fmtCell(row, c, theme)
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      <div className="row" style={{ marginTop: "0.85rem" }}>
        <p className="muted">{total} records</p>
        <p>
          <button className="ghost" disabled={page <= 1} onClick={() => setParam("page", String(page - 1))}>
            Prev
          </button>{" "}
          <span className="muted">
            {page} / {pages}
          </span>{" "}
          <button
            className="ghost"
            disabled={page >= pages}
            onClick={() => setParam("page", String(page + 1))}
          >
            Next
          </button>
        </p>
      </div>
    </div>
  );
}

function fmtCell(
  row: Record<string, unknown>,
  field: UiField,
  theme: { currency: string; locale: string },
) {
  if (field.relation) return expandedLabel(row, field.name) ?? "";
  const value = row[field.name];
  if (value == null) return "";
  if (field.widget === "currency") return formatMoney(value, field.widget_options?.currency || theme.currency, theme.locale);
  if (field.widget === "percentage") return `${value}%`;
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function SingletonSettings({ meta, entities }: { meta: UiEntity; entities: UiEntity[] }) {
  const fields = meta.fields.filter(formVisible).filter((f) => f.relation_kind !== "one_to_many");
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .settings(meta.slug)
      .then((row) => {
        const next: Record<string, unknown> = {};
        for (const field of fields) next[field.name] = row[field.name] ?? "";
        setValues(next);
      })
      .catch((e) => setError(e.message));
  }, [meta.slug]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const body: Record<string, unknown> = {};
    for (const field of fields) {
      const raw = values[field.name];
      if (raw === "" || raw == null) continue;
      body[field.name] = raw;
    }
    try {
      setSaving(true);
      setError("");
      await api.saveSettings(meta.slug, body);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Unable to save.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="page">
      <div className="badge">Singleton</div>
      <h2>{meta.label}</h2>
      <form className="form form-wide" onSubmit={onSubmit}>
        <FormLayout
          fields={fields}
          values={values}
          entities={entities}
          fieldErrors={{}}
          onChange={(name, value) => setValues((prev) => ({ ...prev, [name]: value }))}
        />
        {error ? (
          <p className="error" role="alert">
            {error}
          </p>
        ) : null}
        <button type="submit" disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
      </form>
    </div>
  );
}

function ImportPanel({ slug, onDone }: { slug: string; onDone: () => void }) {
  const [csv, setCsv] = useState("");
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [running, setRunning] = useState(false);

  async function runPreview() {
    setError("");
    setPreview(await api.importPreview(slug, csv));
  }

  async function runImport() {
    setRunning(true);
    try {
      setPreview(await api.importRun(slug, csv));
      onDone();
    } catch (e) {
      setError(e instanceof Error ? e.message : "import failed");
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="panel" style={{ padding: "0.85rem" }}>
      <h3>Import CSV</h3>
      <textarea
        rows={6}
        value={csv}
        onChange={(e) => setCsv(e.target.value)}
        placeholder="Paste CSV with a header row"
      />
      {error ? <p className="error">{error}</p> : null}
      {preview ? (
        <p className="muted">
          Rows: {String(preview.rows ?? preview.imported ?? 0)} · Valid: {String(preview.valid ?? "")} ·
          Invalid: {String(preview.invalid ?? preview.failed ?? "")}
        </p>
      ) : null}
      <div className="actions">
        <button type="button" className="ghost" onClick={() => runPreview().catch((e) => setError(e.message))}>
          Preview
        </button>
        <button type="button" disabled={running} onClick={() => void runImport()}>
          {running ? "Importing…" : "Import"}
        </button>
      </div>
    </div>
  );
}

