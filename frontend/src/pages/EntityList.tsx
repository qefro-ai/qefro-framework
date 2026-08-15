import { useEffect, useMemo, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { api, expandedLabel, listVisible, type UiEntity, type UiField } from "../api";

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

  const filterable = useMemo(
    () => meta?.fields.filter((f) => f.filterable || f.filter) ?? [],
    [meta],
  );

  useEffect(() => {
    if (!slug || !meta) return;
    const q = new URLSearchParams();
    if (search) q.set("search", search);
    q.set("sort", sort);
    q.set("page", String(page));
    q.set("page_size", "25");
    for (const field of filterable) {
      const value = params.get(field.name);
      if (value) q.set(field.name, value);
    }
    api
      .list(slug, q)
      .then((result) => {
        setRows(result.items);
        setTotal(result.total);
        setPageSize(result.page_size);
        setError("");
      })
      .catch((e) => setError(e.message));
  }, [slug, search, sort, page, params, meta, filterable]);

  if (!meta) return <p>Unknown entity.</p>;
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

  return (
    <div>
      <div className="row">
        <div>
          <div className="badge">{meta.entity}</div>
          <h2>{meta.label_plural}</h2>
        </div>
        <Link to={`/${meta.slug}/new`}>
          <button>New {meta.label}</button>
        </Link>
      </div>
      {meta.searchable && (
        <p>
          <input
            placeholder="Search"
            value={search}
            onChange={(e) => setParam("search", e.target.value)}
          />
        </p>
      )}
      {filterable.length > 0 && (
        <div className="filters">
          {filterable.map((field) =>
            field.enum_values ? (
              <label key={field.name}>
                {field.label}
                <select
                  value={params.get(field.name) ?? ""}
                  onChange={(e) => setParam(field.name, e.target.value)}
                >
                  <option value="">Any</option>
                  {field.enum_values.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <label key={field.name}>
                {field.label}
                <input
                  value={params.get(field.name) ?? ""}
                  onChange={(e) => setParam(field.name, e.target.value)}
                />
              </label>
            ),
          )}
        </div>
      )}
      {error && <p className="error">{error}</p>}
      <table>
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.name} onClick={() => toggleSort(c)} style={{ cursor: "pointer" }}>
                {c.label}
                {sort === c.name ? " ↑" : sort === `-${c.name}` ? " ↓" : ""}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={String(row.id)}>
              {cols.map((c, i) => (
                <td key={c.name}>
                  {i === 0 ? (
                    <Link to={`/${meta.slug}/${row.id}`}>{fmtCell(row, c)}</Link>
                  ) : (
                    fmtCell(row, c)
                  )}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="row">
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

function fmtCell(row: Record<string, unknown>, field: UiField) {
  if (field.relation) {
    return expandedLabel(row, field.name) ?? "";
  }
  const value = row[field.name];
  if (value == null) return "";
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}
