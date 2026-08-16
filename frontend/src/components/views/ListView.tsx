import { Fragment } from "react";
import { Link } from "react-router-dom";
import { EmptyState } from "../ui/EmptyState";
import { StatusBadge } from "../ui/StatusBadge";
import { displayValue, listGroupField } from "../../metadata/views";
import type { CollectionViewProps } from "../../views/registry";

export default function ListView({ meta, slug, rows, loading }: CollectionViewProps) {
  const cols = meta.fields.filter((f) => f.list !== false && !f.hidden).slice(0, 8);
  const groupBy = listGroupField(meta);
  const groups = groupBy
    ? [...new Map(rows.map((r) => [String(r[groupBy] ?? "(none)"), true])).keys()].map((key) => [
        key,
        rows.filter((r) => String(r[groupBy] ?? "(none)") === key),
      ])
    : [["", rows]];

  if (loading) return <p className="muted">Loading…</p>;
  if (rows.length === 0) return <EmptyState title={`No ${meta.label_plural.toLowerCase()} yet`} />;

  return (
    <div className="panel table-wrap">
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
                    <td key={c.name} data-label={c.label} className={c.widget === "currency" || c.type === "decimal" ? "num" : undefined}>
                      {i === 0 ? (
                        <Link to={`/${slug}/${row.id}`}>{displayValue(row, c.name)}</Link>
                      ) : c.widget === "status" || c.name === "status" ? (
                        <StatusBadge value={row[c.name]} indicators={c.widget_options?.indicators} />
                      ) : (
                        displayValue(row, c.name)
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
