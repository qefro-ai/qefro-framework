import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api, type UiEntity } from "../../sdk/client";
import { EmptyState, ErrorState, Skeleton } from "../ui/EmptyState";
import { renderView } from "../../views/registry";
import "../../views";
import { friendlyError } from "../../friendlyError";
import { availableViews, canCreate, defaultView } from "../../metadata/views";
import type { ViewKind } from "../../metadata/types";
import { useRealtime } from "../../realtime";

export function EmbeddedEntityView({
  entities,
  slug,
  view,
  query,
  extra,
  compact,
  selectedId,
  onSelect,
  emptyAction,
}: {
  entities: UiEntity[];
  slug: string;
  view?: string | null;
  query?: string | null;
  extra?: URLSearchParams;
  compact?: boolean;
  selectedId?: string;
  onSelect?: (id: string) => void;
  emptyAction?: { label: string; to: string };
}) {
  const meta = entities.find((e) => e.slug === slug);
  const [rows, setRows] = useState<Record<string, unknown>[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);

  const views = useMemo(() => (meta ? availableViews(meta) : (["list"] as ViewKind[])), [meta]);
  const requested = (view || "") as ViewKind;
  const currentView = views.includes(requested) ? requested : meta ? defaultView(meta) : "list";

  useEffect(() => {
    if (!slug || !meta) return;
    const q = new URLSearchParams(query || "");
    if (extra) {
      for (const [key, value] of extra.entries()) {
        if (value && !q.has(key)) q.set(key, value);
      }
    }
    q.set("page", q.get("page") || "1");
    q.set("page_size", compact ? "15" : q.get("page_size") || "25");
    setLoading(true);
    api
      .list(slug, q)
      .then((result) => {
        setRows(result.items);
        setTotal(result.total);
        setError("");
      })
      .catch((err) => setError(friendlyError(err)))
      .finally(() => setLoading(false));
  }, [slug, query, extra?.toString(), meta, tick, compact]);

  useRealtime({ entity: meta?.entity, enabled: Boolean(meta) }, () => setTick((n) => n + 1));

  if (!meta) return <ErrorState message="Unknown entity." />;
  if (error) {
    return (
      <ErrorState message={error} onRetry={() => setTick((n) => n + 1)} />
    );
  }
  if (loading && rows.length === 0) {
    return <Skeleton variant={currentView === "kanban" ? "kanban" : currentView === "card" ? "cards" : "table"} />;
  }
  if (rows.length === 0) {
    const allowCreate = canCreate(meta);
    return (
      <EmptyState
        title={`No ${meta.label_plural.toLowerCase()}`}
        action={
          allowCreate && emptyAction ? (
            <Link to={emptyAction.to} className="btn">
              {emptyAction.label}
            </Link>
          ) : undefined
        }
      />
    );
  }

  return (
    <div className={`embedded-view${compact ? " is-compact" : ""}`}>
      {renderView(currentView, {
        meta,
        entities,
        slug,
        rows,
        total,
        loading,
        onReload: () => setTick((n) => n + 1),
        onError: setError,
        onSelect,
        selectedId,
        compact,
      })}
    </div>
  );
}
