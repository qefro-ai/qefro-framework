import { Link } from "react-router-dom";
import { EmptyState, Skeleton } from "../ui/EmptyState";
import { EntityCard } from "./EntityCard";
import { canCreate } from "../../metadata/views";
import type { CollectionViewProps } from "../../views/registry";

export default function CardView({
  meta,
  slug,
  rows,
  loading,
  queryActive,
  onClearQuery,
}: CollectionViewProps) {
  const spec = meta.views?.card;
  const allowCreate = canCreate(meta);

  if (loading && rows.length === 0) return <Skeleton variant="cards" rows={6} />;
  if (rows.length === 0) {
    return (
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
          queryActive && onClearQuery ? (
            <button type="button" className="ghost" onClick={onClearQuery}>
              Clear filters
            </button>
          ) : allowCreate ? (
            <Link to={`/${slug}/new`}>
              <button>New {meta.label}</button>
            </Link>
          ) : undefined
        }
      />
    );
  }

  return (
    <div className={`card-grid${loading ? " is-loading" : ""}`} role="list" aria-busy={loading || undefined}>
      {rows.map((row) => (
        <div key={String(row.id)} role="listitem">
          <EntityCard meta={meta} slug={slug} row={row} spec={spec} />
          <div className="entity-card-actions">
            <Link to={`/${slug}/${row.id}`}>View</Link>
          </div>
        </div>
      ))}
    </div>
  );
}
