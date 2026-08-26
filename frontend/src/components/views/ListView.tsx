import { Fragment } from "react";
import { Link } from "react-router-dom";
import { EmptyState, Skeleton } from "../ui/EmptyState";
import { FieldValue } from "../fields/FieldValue";
import { listGroupField } from "../../metadata/views";
import type { CollectionViewProps } from "../../views/registry";

export default function ListView({
  meta,
  slug,
  rows,
  loading,
  queryActive,
  onClearQuery,
}: CollectionViewProps) {
  const cols = meta.fields.filter((f) => f.list !== false && !f.hidden).slice(0, 8);
  const groupBy = listGroupField(meta);
  const groups = groupBy
    ? [...new Map(rows.map((r) => [String(r[groupBy] ?? "(none)"), true])).keys()].map((key) => [
        key,
        rows.filter((r) => String(r[groupBy] ?? "(none)") === key),
      ])
    : [["", rows]];

  if (loading && rows.length === 0) return <Skeleton rows={6} />;
  if (rows.length === 0) {
    return (
      <EmptyState
        title={
          queryActive
            ? `No matching ${meta.label_plural.toLowerCase()}`
            : `No ${meta.label_plural.toLowerCase()} yet`
        }
        description={queryActive ? "Try a different search or clear filters." : undefined}
        action={
          queryActive && onClearQuery ? (
            <button type="button" className="ghost" onClick={onClearQuery}>
              Clear filters
            </button>
          ) : undefined
        }
      />
    );
  }

  return (
    <div className="panel table-wrap" aria-busy={loading || undefined}>
      <table className="data freeze">
        <thead>
          <tr>
            {cols.map((c) => (
              <th key={c.name}>{c.label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {(groups as Array<[string, Record<string, unknown>[]]>).map(([group, groupRows]) => (
            <Fragment key={group || "all"}>
              {group ? (
                <tr key={`g-${group}`} className="group-row">
                  <td colSpan={cols.length}>
                    {group} ({groupRows.length})
                  </td>
                </tr>
              ) : null}
              {groupRows.map((row) => (
                <tr key={String(row.id)}>
                  {cols.map((c, i) => (
                    <td
                      key={c.name}
                      data-label={c.label}
                      className={c.widget === "currency" || c.type === "decimal" ? "num" : undefined}
                    >
                      {i === 0 ? (
                        <Link to={`/${slug}/${row.id}`}>
                          <FieldValue row={row} field={c} />
                        </Link>
                      ) : (
                        <FieldValue row={row} field={c} />
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
