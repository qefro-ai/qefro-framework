import { FormEvent, Fragment, useEffect, useMemo, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import {
  api,
  ApiError,
  expandedLabel,
  formVisible,
  listVisible,
  type EntityAction,
  type UiEntity,
  type UiField,
  type WorkflowAction,
} from "../sdk/client";
import { ActionBar } from "../components/actions/ActionBar";
import { FilterBar } from "../components/filters/FilterBar";
import { FormLayout } from "../components/forms/FormLayout";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { ActionMenu } from "../components/ui/ActionMenu";
import { FieldValue } from "../components/fields/FieldValue";
import { ViewSelector } from "../components/views/ViewSelector";
import { renderView } from "../views/registry";
import "../views";
import { downloadCsv, isoDate } from "../format";
import { friendlyError } from "../friendlyError";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";
import { availableViews, calendarStartField, canCreate, canDelete, listGroupField, listViewSpec } from "../metadata/views";
import type { ViewKind } from "../metadata/types";
import { usePrefsOptional } from "../prefsContext";
import { useRealtime } from "../realtime";

export default function EntityList({ entities }: { entities: UiEntity[] }) {
  const { slug } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const [params, setParams] = useSearchParams();
  const search = params.get("search") ?? "";
  const page = Number(params.get("page") ?? "1");
  const prefs = usePrefsOptional();
  const table = slug ? prefs?.tablePrefs(slug) : undefined;
  const listSpec = listViewSpec(meta ?? ({ fields: [] } as UiEntity));
  const defaultSort = listSpec?.default_sort
    ? `${listSpec.default_sort.direction === "desc" ? "-" : ""}${listSpec.default_sort.field}`
    : "-created_at";
  const sort = params.get("sort") ?? table?.sort ?? defaultSort;
  const views = useMemo(() => (meta ? availableViews(meta) : (["list"] as ViewKind[])), [meta]);
  const view = (["list", "card", "kanban", "calendar"].includes(params.get("view") || "")
    ? (params.get("view") as ViewKind)
    : ((table?.view as ViewKind) || "list")) as ViewKind;
  const currentView = views.includes(view) ? view : "list";
  const [rows, setRows] = useState<Record<string, unknown>[]>([]);
  const [total, setTotal] = useState(0);
  const [pageSize, setPageSize] = useState(table?.pageSize ?? listSpec?.page_size ?? 25);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [importOpen, setImportOpen] = useState(false);
  const [colsOpen, setColsOpen] = useState(false);
  const [searchInput, setSearchInput] = useState(search);
  const [tick, setTick] = useState(0);
  const theme = useTenantTheme();

  const filterable = useMemo(
    () => meta?.fields.filter((f) => f.filterable || f.filter) ?? [],
    [meta],
  );

  const allCols = useMemo(() => {
    if (!meta) return [];
    if (listSpec?.columns?.length) {
      return listSpec.columns
        .map((c) => {
          const field = meta.fields.find((f) => f.name === c.field);
          if (!field) return null;
          return {
            ...field,
            width: c.width != null ? String(c.width) : field.width,
            widget: c.widget || field.widget,
          };
        })
        .filter(Boolean) as UiField[];
    }
    return meta.fields.filter(listVisible);
  }, [meta, listSpec]);

  const cols = useMemo(() => {
    if (table?.columns?.length) return allCols.filter((c) => table.columns!.includes(c.name));
    return allCols;
  }, [allCols, table]);
  const numericCols = cols.filter(isNumeric);

  useEffect(() => {
    setSearchInput(search);
  }, [search]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      if (searchInput !== search) setParam("search", searchInput);
    }, 250);
    return () => window.clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchInput]);

  useEffect(() => {
    if (!slug || !meta || meta.singleton) return;
    const q = new URLSearchParams();
    if (search) q.set("search", search);
    q.set("sort", sort);
    q.set("page", String(page));
    const size = currentView === "list" ? pageSize : Math.max(pageSize, 100);
    q.set("page_size", String(size));
    if (currentView === "calendar" && meta) {
      const start = calendarStartField(meta);
      const cursor = params.get("cursor") ? new Date(params.get("cursor") as string) : new Date();
      const cal = params.get("cal") || "month";
      if (start) {
        const from = new Date(cursor);
        const to = new Date(cursor);
        if (cal === "day") {
          /* same day */
        } else if (cal === "week") {
          const day = (from.getDay() + 6) % 7;
          from.setDate(from.getDate() - day);
          to.setDate(from.getDate() + 6);
        } else {
          from.setDate(1);
          to.setMonth(to.getMonth() + 1);
          to.setDate(0);
        }
        q.set(`${start.name}.between`, `${isoDate(from)},${isoDate(to)}`);
      }
    }
    for (const [key, value] of params.entries()) {
      if (["search", "sort", "page", "page_size", "view", "cal", "cursor"].includes(key)) continue;
      if (key.endsWith(".op") || key.endsWith(".preset")) continue;
      if (value) q.set(key, value);
    }
    setLoading(true);
    api
      .list(slug, q)
      .then((result) => {
        setRows(result.items);
        setTotal(result.total);
        if (currentView === "list") setPageSize(result.page_size);
        setError("");
      })
      .catch((e) => setError(friendlyError(e)))
      .finally(() => setLoading(false));
  }, [slug, search, sort, page, params, meta, tick, pageSize, currentView]);

  useRealtime({ entity: meta?.entity, enabled: Boolean(meta && !meta.singleton) }, () => {
    setTick((n) => n + 1);
  });

  if (!meta) return <ErrorState message="Unknown entity." />;
  if (meta.singleton) return <SingletonSettings meta={meta} entities={entities} />;
  const pages = Math.max(1, Math.ceil(total / pageSize));
  const groupBy = listGroupField(meta);
  const grouped = groupBy ? groupRows(rows, groupBy) : ([["", rows]] as Array<[string, Record<string, unknown>[]]>);
  const allowCreate = canCreate(meta);
  const allowDelete = canDelete(meta);
  const showRowActions = Boolean(meta.workflow || meta.capabilities?.workflow || meta.capabilities?.actions);
  const tableColSpan = cols.length + 1 + (showRowActions ? 1 : 0);
  const queryActive = isQueryActive(params, search);
  const initialLoad = loading && rows.length === 0 && !error;

  function clearQuery() {
    const next = new URLSearchParams();
    const keepView = params.get("view");
    const keepCal = params.get("cal");
    if (keepView) next.set("view", keepView);
    if (keepCal) next.set("cal", keepCal);
    setSearchInput("");
    setParams(next);
  }

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
    if (slug) prefs?.setTablePrefs(slug, { sort: next });
  }

  function toggleAll() {
    if (selected.size === rows.length) setSelected(new Set());
    else setSelected(new Set(rows.map((r) => String(r.id))));
  }

  async function bulkDelete() {
    if (!meta || !confirm(`Delete ${selected.size} records?`)) return;
    try {
      for (const id of selected) await api.remove(meta.slug, id);
      setSelected(new Set());
      setTick((n) => n + 1);
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  async function exportCsv() {
    if (!slug || !meta) return;
    const q = new URLSearchParams(params);
    q.set("page", "1");
    q.set("page_size", "1000");
    q.set("sort", sort);
    if (search) q.set("search", search);
    for (const key of [...q.keys()]) {
      if (key.endsWith(".op") || key.endsWith(".preset")) q.delete(key);
    }
    const result = await api.list(slug, q);
    const source = selected.size
      ? result.items.filter((r) => selected.has(String(r.id)))
      : result.items;
    downloadCsv(
      `${meta.slug}.csv`,
      cols.map((c) => c.label),
      source.map((row) => cols.map((c) => cellText(row, c, theme))),
    );
  }

  return (
    <div className="page">
      <PageHeader
        kicker={meta.entity}
        title={meta.label_plural}
        actions={
          <>
            <ActionMenu
              items={[
                { key: "export", label: "Export", onSelect: () => void exportCsv() },
                {
                  key: "import",
                  label: "Import CSV",
                  hidden: !allowCreate,
                  onSelect: () => setImportOpen((v) => !v),
                },
              ]}
            />
            {allowCreate ? (
              <Link to={`/${meta.slug}/new`}>
                <button type="button">New {meta.label}</button>
              </Link>
            ) : null}
          </>
        }
      />
      <ViewSelector
        views={views}
        current={currentView}
        onChange={(next) => {
          setParam("view", next);
          if (slug) prefs?.setTablePrefs(slug, { view: next });
        }}
      />
      {importOpen ? <ImportPanel slug={meta.slug} onDone={() => setTick((n) => n + 1)} /> : null}
      <div className="view-toolbar toolbar">
        {meta.searchable && (
          <div className="search-field">
            <input
              placeholder={`Search ${meta.label_plural.toLowerCase()}`}
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearchInput("");
                  setParam("search", "");
                }
              }}
              aria-label={`Search ${meta.label_plural}`}
            />
            {searchInput ? (
              <button
                type="button"
                className="ghost icon-btn search-clear"
                aria-label="Clear search"
                onClick={() => {
                  setSearchInput("");
                  setParam("search", "");
                }}
              >
                ×
              </button>
            ) : null}
          </div>
        )}
        {currentView === "list" ? (
          <>
            <button type="button" className="ghost" onClick={() => setColsOpen((v) => !v)}>
              Columns
            </button>
            <label className="page-size">
              Page size
              <select
                value={String(pageSize)}
                onChange={(e) => {
                  const n = Number(e.target.value);
                  setPageSize(n);
                  if (slug) prefs?.setTablePrefs(slug, { pageSize: n });
                  setParam("page", "1");
                }}
              >
                {[10, 25, 50, 100].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          </>
        ) : null}
        {loading ? (
          <span className="muted toolbar-status" aria-live="polite">
            Loading…
          </span>
        ) : null}
      </div>
      {colsOpen ? (
        <div className="column-picker panel">
          {allCols.map((c) => {
            const visible = cols.some((x) => x.name === c.name);
            return (
              <label key={c.name} className="inline-check">
                <input
                  type="checkbox"
                  checked={visible}
                  onChange={() => {
                    const names = (visible ? cols.filter((x) => x.name !== c.name) : [...cols, c]).map((x) => x.name);
                    if (slug) prefs?.setTablePrefs(slug, { columns: names });
                  }}
                />
                {c.label}
              </label>
            );
          })}
        </div>
      ) : null}
      {filterable.length > 0 && (
        <FilterBar
          entity={meta.entity}
          fields={filterable}
          entities={entities}
          params={params}
          onChange={setParam}
          onReplace={setParams}
        />
      )}
      {error && (
        <ErrorState
          message={`Unable to load ${meta.label_plural.toLowerCase()}. ${error}`}
          onRetry={() => setTick((n) => n + 1)}
        />
      )}
      {selected.size > 0 && (
        <div className="actions bulk-bar">
          <span className="muted">{selected.size} selected</span>
          <button className="ghost" onClick={() => void exportCsv()}>
            Export selected
          </button>
          {allowDelete ? (
            <button className="danger" onClick={() => void bulkDelete()}>
              Delete selected
            </button>
          ) : null}
        </div>
      )}
      {currentView === "list" ? (
      <div className={`panel table-wrap${loading ? " is-loading" : ""}`} aria-busy={loading || undefined}>
        {initialLoad ? (
          <Skeleton rows={8} />
        ) : rows.length === 0 && !error ? (
          <EmptyState
            title={
              queryActive
                ? `No matching ${meta.label_plural.toLowerCase()}`
                : `No ${meta.label_plural.toLowerCase()} yet`
            }
            description={
              queryActive
                ? "Try a different search or clear filters."
                : `Create your first ${meta.label.toLowerCase()}.`
            }
            action={
              queryActive ? (
                <button type="button" className="ghost" onClick={clearQuery}>
                  Clear filters
                </button>
              ) : allowCreate ? (
                <Link to={`/${meta.slug}/new`}>
                  <button>New {meta.label}</button>
                </Link>
              ) : undefined
            }
          />
        ) : (
          <table className="data freeze">
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
                    className={isNumeric(c) ? "num" : undefined}
                    style={{ cursor: c.sortable ? "pointer" : undefined, width: c.width }}
                  >
                    {c.label}
                    {sort === c.name ? " ↑" : sort === `-${c.name}` ? " ↓" : ""}
                  </th>
                ))}
                {showRowActions ? <th className="row-actions">Actions</th> : null}
              </tr>
            </thead>
            <tbody>
              {grouped.map(([group, groupRowsList]) => (
                <Fragment key={group || "all"}>
                  {group ? (
                    <tr key={`g-${group}`} className="group-row">
                      <td colSpan={tableColSpan}>
                        {group} ({groupRowsList.length})
                      </td>
                    </tr>
                  ) : null}
                  {groupRowsList.map((row) => (
                    <tr key={String(row.id)} className={selected.has(String(row.id)) ? "is-selected" : undefined}>
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
                        <td key={c.name} data-label={c.label} className={isNumeric(c) ? "num" : undefined}>
                          {i === 0 ? (
                            <Link to={`/${meta.slug}/${row.id}`}>
                              <FieldValue row={row} field={c} />
                            </Link>
                          ) : (
                            <FieldValue row={row} field={c} />
                          )}
                        </td>
                      ))}
                      {showRowActions ? (
                        <td data-label="Actions" className="row-actions">
                          <ActionBar
                            compact
                            actions={((row._actions as EntityAction[] | undefined) ?? []).slice(0, 2)}
                            transitions={
                              ((row._workflow as { transitions?: WorkflowAction[] } | undefined)?.transitions ?? []).slice(
                                0,
                                2,
                              )
                            }
                            onAction={async (name) => {
                              try {
                                await api.action(meta.slug, String(row.id), name);
                                setTick((n) => n + 1);
                              } catch (err) {
                                setError(friendlyError(err));
                              }
                            }}
                            onTransition={async (name) => {
                              try {
                                await api.transition(meta.slug, String(row.id), name);
                                setTick((n) => n + 1);
                              } catch (err) {
                                setError(friendlyError(err));
                              }
                            }}
                          />
                        </td>
                      ) : null}
                    </tr>
                  ))}
                </Fragment>
              ))}
            </tbody>
            {numericCols.length > 0 && rows.length > 0 ? (
              <tfoot>
                <tr>
                  <td />
                  {cols.map((c) => (
                    <td key={c.name} className={isNumeric(c) ? "num" : undefined}>
                      {isNumeric(c)
                        ? c.widget === "currency"
                          ? formatMoney(
                              rows.reduce((s, r) => s + Number(r[c.name] ?? 0), 0),
                              c.widget_options?.currency || theme.currency,
                              theme.locale,
                            )
                          : rows.reduce((s, r) => s + Number(r[c.name] ?? 0), 0)
                        : ""}
                    </td>
                  ))}
                  {showRowActions ? <td /> : null}
                </tr>
              </tfoot>
            ) : null}
          </table>
        )}
      </div>
      ) : (
        renderView(currentView, {
          meta,
          entities,
          slug: meta.slug,
          rows,
          total,
          loading,
          onReload: () => setTick((n) => n + 1),
          onError: setError,
          queryActive,
          onClearQuery: clearQuery,
        })
      )}
      {currentView === "list" ? (
      <div className="row pagination" style={{ marginTop: "0.85rem" }}>
        <p className="muted">{total} records</p>
        <p>
          <button type="button" className="ghost" disabled={page <= 1} onClick={() => setParam("page", String(page - 1))}>
            Prev
          </button>{" "}
          <span className="muted">
            {page} / {pages}
          </span>{" "}
          <button type="button" className="ghost" disabled={page >= pages} onClick={() => setParam("page", String(page + 1))}>
            Next
          </button>
        </p>
      </div>
      ) : (
        <p className="muted">{total} records</p>
      )}
    </div>
  );
}

function isNumeric(field: UiField) {
  return (
    field.widget === "currency" ||
    field.widget === "percentage" ||
    field.widget === "number" ||
    field.type === "integer" ||
    field.type === "decimal"
  );
}

function groupRows(rows: Record<string, unknown>[], field: string): Array<[string, Record<string, unknown>[]]> {
  const map = new Map<string, Record<string, unknown>[]>();
  for (const row of rows) {
    const key = String(row[field] ?? "(none)");
    const list = map.get(key) ?? [];
    list.push(row);
    map.set(key, list);
  }
  return Array.from(map.entries());
}

function cellText(
  row: Record<string, unknown>,
  field: UiField,
  theme: { currency: string; locale: string },
) {
  if (field.relation) return expandedLabel(row, field.name) ?? "";
  const value = row[field.name];
  if (value == null) return "";
  if (field.widget === "currency") return formatMoney(value, field.widget_options?.currency || theme.currency, theme.locale);
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function isQueryActive(params: URLSearchParams, search: string) {
  if (search.trim()) return true;
  const skip = new Set(["search", "sort", "page", "page_size", "view", "cal", "cursor"]);
  for (const [key, value] of params.entries()) {
    if (skip.has(key) || key.endsWith(".op") || key.endsWith(".preset")) continue;
    if (value) return true;
  }
  return false;
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
      .catch((e) => setError(friendlyError(e)));
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
      setError(err instanceof ApiError ? friendlyError(err) : "Unable to save.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="page">
      <PageHeader kicker="Singleton" title={meta.label} />
      <form className="form form-wide" onSubmit={onSubmit}>
        <FormLayout
          fields={fields}
          values={values}
          entities={entities}
          fieldErrors={{}}
          onChange={(name, value) => setValues((prev) => ({ ...prev, [name]: value }))}
        />
        {error ? <ErrorState message={error} /> : null}
        <div className="form-actions actions">
          <button type="submit" disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
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
      setError(friendlyError(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="panel" style={{ padding: "0.85rem" }}>
      <h3>Import CSV</h3>
      <textarea rows={6} value={csv} onChange={(e) => setCsv(e.target.value)} placeholder="Paste CSV with a header row" />
      {error ? <ErrorState message={error} /> : null}
      {preview ? (
        <p className="muted">
          Rows: {String(preview.rows ?? preview.imported ?? 0)} · Valid: {String(preview.valid ?? "")} ·
          Invalid: {String(preview.invalid ?? preview.failed ?? "")}
        </p>
      ) : null}
      <div className="actions">
        <button type="button" className="ghost" onClick={() => runPreview().catch((e) => setError(friendlyError(e)))}>
          Preview
        </button>
        <button type="button" disabled={running} onClick={() => void runImport()}>
          {running ? "Importing…" : "Import"}
        </button>
      </div>
    </div>
  );
}
