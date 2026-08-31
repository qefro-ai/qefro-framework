import { Link } from "react-router-dom";
import { api, type EntityAction, type WorkflowAction } from "../../sdk/client";
import { ActionBar } from "../actions/ActionBar";
import { EmptyState, Skeleton } from "../ui/EmptyState";
import { EntityCard } from "./EntityCard";
import { canCreate } from "../../metadata/views";
import { friendlyError } from "../../friendlyError";
import type { CollectionViewProps } from "../../views/registry";

export default function CardView({
  meta,
  slug,
  rows,
  loading,
  queryActive,
  onClearQuery,
  onReload,
  onError,
}: CollectionViewProps) {
  const spec = meta.views?.card;
  const allowCreate = canCreate(meta);
  const showActions = Boolean(meta.workflow || meta.capabilities?.workflow || meta.capabilities?.actions);

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
      {rows.map((row) => {
        const actions = ((row._actions as EntityAction[] | undefined) ?? []).slice(0, 2);
        const transitions =
          ((row._workflow as { transitions?: WorkflowAction[] } | undefined)?.transitions ?? []).slice(0, 2);
        return (
          <div key={String(row.id)} role="listitem">
            <EntityCard
              meta={meta}
              slug={slug}
              row={row}
              spec={spec}
              footer={
                showActions && (actions.length > 0 || transitions.length > 0) ? (
                  <ActionBar
                    compact
                    actions={actions}
                    transitions={transitions}
                    onAction={async (name, _action, input) => {
                      try {
                        await api.action(slug, String(row.id), name, input ?? {});
                        onReload();
                      } catch (err) {
                        onError(friendlyError(err));
                      }
                    }}
                    onTransition={async (name) => {
                      try {
                        await api.transition(slug, String(row.id), name);
                        onReload();
                      } catch (err) {
                        onError(friendlyError(err));
                      }
                    }}
                  />
                ) : undefined
              }
            />
            <div className="entity-card-actions">
              <Link to={`/${slug}/${row.id}`}>View</Link>
            </div>
          </div>
        );
      })}
    </div>
  );
}
