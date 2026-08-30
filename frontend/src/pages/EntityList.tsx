import { FormEvent, Fragment, useEffect, useId, useMemo, useRef, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import {
  api,
  ApiError,
  formVisible,
  listVisible,
  type EntityAction,
  type UiEntity,
  type UiField,
  type WorkflowAction,
} from "../sdk/client";
import { ActionBar } from "../components/actions/ActionBar";
import { FilterBar, SavedViewsMenu } from "../components/filters/FilterBar";
import { FormLayout } from "../components/forms/FormLayout";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { ActionMenu } from "../components/ui/ActionMenu";
import { AssignDialog } from "../components/ui/AssignDialog";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { showSnackbar } from "../components/ui/Snackbar";
import { FieldValue } from "../components/fields/FieldValue";
import { ViewSelector } from "../components/views/ViewSelector";
import { renderView } from "../views/registry";
import "../views";
import { isoDate } from "../format";
import { friendlyError } from "../friendlyError";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";
import { availableViews, calendarStartField, canCreate, canDelete, canExport, canUpdate, defaultView, listGroupField, listViewSpec } from "../metadata/views";
import { entityCount, t } from "../i18n";
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
  const fallbackView = meta ? defaultView(meta) : "list";
  const view = (views.includes((params.get("view") || "") as ViewKind)
    ? (params.get("view") as ViewKind)
    : ((table?.view as ViewKind) || fallbackView)) as ViewKind;
  const currentView = views.includes(view) ? view : "list";
  const [rows, setRows] = useState<Record<string, unknown>[]>([]);
  const [total, setTotal] = useState(0);
  const [pageSize, setPageSize] = useState(table?.pageSize ?? listSpec?.page_size ?? 25);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [pending, setPending] = useState<"archive" | "delete" | "assign" | null>(null);
  const [busy, setBusy] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
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
    setSelected(new Set());
    setPending(null);
  }, [slug]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      if (searchInput !== search) setParam("search", searchInput);
    }, 250);
    return () => window.clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchInput]);

  useEffect(() => {
    if (!slug || !meta || meta.singleton) return;
    if (currentView === "chart") {
      setRows([]);
      setTotal(0);
      setLoading(false);
      return;
    }
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
  const allowExport = canExport(meta);
  const allowUpdate = canUpdate(meta);
  const allowArchive = Boolean(meta.capabilities?.archive) && allowUpdate;
  const allowAssign = Boolean(meta.capabilities?.assignment) && allowUpdate;
  const allowBulk = meta.capabilities?.bulk !== false;
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

  async function runBulk(action: string, fields: Record<string, unknown> = {}) {
    if (!meta || busy) return;
    const count = selected.size;
    setBusy(true);
    try {
      const result = await api.bulk(meta.slug, { action, ids: [...selected], fields });
      setSelected(new Set());
      setPending(null);
      const failed = result.failed ?? 0;
      const succeeded = result.succeeded ?? 0;
      const doneAction = action === "assign" && (fields.assigned_to == null || fields.assigned_to === "") ? "update" : action;
      showSnackbar(
        bulkResultMessage(doneAction, succeeded, failed, meta.label, meta.label_plural),
        failed ? "error" : "success",
      );
      setTick((n) => n + 1);
    } catch (e) {
      setError(friendlyError(e));
      showSnackbar(
        t("bulk.failed", { action, count: entityCount(count, meta.label, meta.label_plural) }),
        "error",
      );
    } finally {
      setBusy(false);
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
    if (selected.size) q.set("ids", [...selected].join(","));
    try {
      await api.exportCsv(slug, q);
    } catch (e) {
      setError(friendlyError(e));
    }
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
                { key: "export", label: t("export.label"), hidden: !allowExport, onSelect: () => void exportCsv() },
                {
                  key: "import",
                  label: "Import CSV",
                  hidden: !allowCreate || meta.capabilities?.import === false,
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
      {importOpen ? <ImportPanel slug={meta.slug} onDone={() => setTick((n) => n + 1)} /> : null}
      <div className="list-toolbar view-toolbar toolbar">
        {meta.searchable && (
          <div className="search-field">
            <svg className="search-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path
                fill="currentColor"
                d="M15.5 14h-.79l-.28-.27A6.47 6.47 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14"
              />
            </svg>
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
        {filterable.length > 0 ? (
          <FilterBar
            entity={meta.entity}
            fields={filterable}
            entities={entities}
            params={params}
            onChange={setParam}
            onReplace={setParams}
          />
        ) : null}
        <div className="list-toolbar-end">
          {loading ? (
            <span className="muted toolbar-status" aria-live="polite">
              Loading…
            </span>
          ) : null}
          <ViewSelector
            views={views}
            current={currentView}
            onChange={(next) => {
              setParam("view", next);
              if (slug) prefs?.setTablePrefs(slug, { view: next });
            }}
          />
          <ListOptions
            entity={meta.entity}
            currentView={currentView}
            allCols={allCols}
            cols={cols}
            pageSize={pageSize}
            canSaveView={queryActive}
            params={params}
            onChangeParam={setParam}
            onReplaceParams={setParams}
            onPageSize={(n) => {
              setPageSize(n);
              if (slug) prefs?.setTablePrefs(slug, { pageSize: n });
              setParam("page", "1");
            }}
            onColumns={(names) => {
              if (slug) prefs?.setTablePrefs(slug, { columns: names });
            }}
          />
        </div>
      </div>
      {error && (
        <ErrorState
          message={`Unable to load ${meta.label_plural.toLowerCase()}. ${error}`}
          onRetry={() => setTick((n) => n + 1)}
        />
      )}
      {selected.size > 0 && allowBulk && (
        <div className="bulk-bar" role="region" aria-label="Bulk actions">
          <span className="bulk-count">{t("bulk.selected", { count: entityCount(selected.size, meta.label, meta.label_plural) })}</span>
          <div className="bulk-actions">
            {allowExport ? (
              <button type="button" className="ghost" onClick={() => void exportCsv()}>
                {t("bulk.export")}
              </button>
            ) : null}
            {allowAssign ? (
              <button type="button" className="ghost" onClick={() => setPending("assign")}>
                {t("bulk.assign")}
              </button>
            ) : null}
            {allowArchive ? (
              <button type="button" className="ghost" onClick={() => setPending("archive")}>
                {t("bulk.archive")}
              </button>
            ) : null}
            {allowDelete ? (
              <button type="button" className="danger" onClick={() => setPending("delete")}>
                {t("bulk.delete")}
              </button>
            ) : null}
            <button type="button" className="ghost" onClick={() => setSelected(new Set())}>
              {t("bulk.clear")}
            </button>
          </div>
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
                <th className="select-cell">
                  <input
                    type="checkbox"
                    aria-label="Select all on this page"
                    checked={rows.length > 0 && selected.size === rows.length}
                    ref={(el) => {
                      if (el) el.indeterminate = selected.size > 0 && selected.size < rows.length;
                    }}
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
                      <td className="select-cell">
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
      <div className="row pagination">
        <p className="muted">{t("list.total", { count: entityCount(total, meta.label, meta.label_plural) })}</p>
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
        <p className="muted">{t("list.total", { count: entityCount(total, meta.label, meta.label_plural) })}</p>
      )}
      <ConfirmDialog
        open={pending === "archive"}
        title={t("bulk.archiveTitle", { count: entityCount(selected.size, meta.label, meta.label_plural) })}
        message={t("bulk.archiveConfirm")}
        confirmLabel="Archive"
        confirmDisabled={busy}
        onCancel={() => setPending(null)}
        onConfirm={() => void runBulk("archive")}
      />
      <ConfirmDialog
        open={pending === "delete"}
        title={t("bulk.deleteTitle", { count: entityCount(selected.size, meta.label, meta.label_plural) })}
        message={t("bulk.deleteConfirm")}
        confirmLabel="Delete"
        danger
        confirmDisabled={busy}
        onCancel={() => setPending(null)}
        onConfirm={() => void runBulk("delete")}
      />
      <AssignDialog
        open={pending === "assign"}
        title={t("bulk.assignTitle", { count: entityCount(selected.size, meta.label, meta.label_plural) })}
        usersSlug={entities.find((e) => e.entity === "User" || e.slug === "users")?.slug ?? "users"}
        onCancel={() => setPending(null)}
        onAssign={(userId) => void runBulk("assign", { assigned_to: userId })}
        onUnassign={() => void runBulk("assign", { assigned_to: null })}
      />
    </div>
  );
}

function ListOptions({
  entity,
  currentView,
  allCols,
  cols,
  pageSize,
  canSaveView,
  params,
  onChangeParam,
  onReplaceParams,
  onPageSize,
  onColumns,
}: {
  entity: string;
  currentView: ViewKind;
  allCols: UiField[];
  cols: UiField[];
  pageSize: number;
  canSaveView: boolean;
  params: URLSearchParams;
  onChangeParam: (key: string, value: string) => void;
  onReplaceParams: (next: URLSearchParams) => void;
  onPageSize: (n: number) => void;
  onColumns: (names: string[]) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    function onPointer(event: MouseEvent) {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="list-options" ref={root}>
          <button type="button" className="ghost" aria-expanded={open} aria-haspopup="dialog" aria-controls={menuId} aria-label="List options" onClick={() => setOpen((value) => !value)}>
            Options
          </button>
      {open ? (
        <div id={menuId} className="list-options-panel" role="dialog" aria-label="List options">
          {currentView === "list" ? (
            <>
              <div className="list-options-section">
                <div className="palette-heading">Columns</div>
                <div className="column-picker">
                  {allCols.map((c) => {
                    const visible = cols.some((x) => x.name === c.name);
                    return (
                      <label key={c.name} className="inline-check">
                        <input
                          type="checkbox"
                          checked={visible}
                          onChange={() => {
                            const names = (visible ? cols.filter((x) => x.name !== c.name) : [...cols, c]).map(
                              (x) => x.name,
                            );
                            onColumns(names);
                          }}
                        />
                        {c.label}
                      </label>
                    );
                  })}
                </div>
              </div>
              <label className="page-size">
                Page size
                <select
                  value={String(pageSize)}
                  onChange={(e) => onPageSize(Number(e.target.value))}
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
          <SavedViewsMenu
            entity={entity}
            params={params}
            canSave={canSaveView}
            onChange={onChangeParam}
            onReplace={onReplaceParams}
          />
        </div>
      ) : null}
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

function bulkResultMessage(
  action: string,
  succeeded: number,
  failed: number,
  label: string,
  labelPlural: string,
) {
  const past =
    action === "delete"
      ? "bulk.done.delete"
      : action === "archive"
        ? "bulk.done.archive"
        : action === "restore"
          ? "bulk.done.restore"
          : action === "assign"
            ? "bulk.done.assign"
            : "bulk.done.update";
  const verb =
    action === "delete" ? "delete" : action === "archive" ? "archive" : action === "restore" ? "restore" : "update";
  const total = succeeded + failed;
  if (failed && !succeeded) {
    return t("bulk.failed", { action: verb, count: entityCount(total, label, labelPlural) });
  }
  const done = t(past, { count: entityCount(succeeded, label, labelPlural) });
  if (failed) return t("bulk.partial", { done, failed });
  return done;
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
      showSnackbar("Saved");
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
    <div className="panel detail-panel">
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
