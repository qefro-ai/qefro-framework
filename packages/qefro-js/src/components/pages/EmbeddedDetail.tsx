import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, detailVisible, type EntityAction, type UiEntity } from "../../sdk/client";
import { ActionBar } from "../actions/ActionBar";
import { AttachmentsPanel } from "../attachments/AttachmentsPanel";
import { EmptyState, ErrorState, Skeleton } from "../ui/EmptyState";
import { FieldValue } from "../fields/FieldValue";
import { Timeline } from "../timeline/Timeline";
import { friendlyError } from "../../friendlyError";
import { displayValue } from "../../metadata/views";
import { useTenantTheme } from "../../metadata/context";
import { useRealtime } from "../../realtime";

export function EmbeddedDetail({
  entities,
  slug,
  id,
  showActivity,
  showAttachments,
}: {
  entities: UiEntity[];
  slug: string;
  id?: string;
  showActivity?: boolean;
  showAttachments?: boolean;
}) {
  const meta = entities.find((e) => e.slug === slug);
  const theme = useTenantTheme();
  const [row, setRow] = useState<Record<string, unknown> | null>(null);
  const [activity, setActivity] = useState<Array<Record<string, unknown>>>([]);
  const [attachments, setAttachments] = useState<Array<Record<string, unknown>>>([]);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!slug || !id || !meta) {
      setRow(null);
      return;
    }
    api
      .get(slug, id)
      .then((data) => {
        setRow(data);
        setError("");
      })
      .catch((err) => {
        setRow(null);
        setError(friendlyError(err));
      });
  }, [slug, id, meta, tick]);

  useEffect(() => {
    if (!showActivity || !slug || !id) return;
    api
      .activity(slug, id)
      .then((d) => setActivity(d.items ?? []))
      .catch(() => setActivity([]));
  }, [showActivity, slug, id, tick]);

  useEffect(() => {
    if (!showAttachments || !slug || !id) return;
    api
      .attachments(slug, id)
      .then((d) => setAttachments(d.items ?? []))
      .catch(() => setAttachments([]));
  }, [showAttachments, slug, id, tick]);

  useRealtime({ entity: meta?.entity, recordId: id }, () => setTick((n) => n + 1));

  if (!meta) return <ErrorState message="Unknown entity." />;
  if (!id) {
    return <EmptyState title={`Select a ${meta.label.toLowerCase()}`} />;
  }
  if (error) return <ErrorState message={error} onRetry={() => setTick((n) => n + 1)} />;
  if (!row) return <Skeleton variant="detail" />;

  const visible = meta.fields.filter(
    (f) =>
      detailVisible(f) &&
      f.relation_kind !== "one_to_many" &&
      f.relation_kind !== "child_table" &&
      f.type !== "child_table",
  );
  const actions = (row._actions as EntityAction[] | undefined) ?? [];
  const related = (row._related ?? {}) as Record<
    string,
    { slug: string; items: Record<string, unknown>[]; total: number }
  >;

  return (
    <div className="embedded-detail">
      <div className="embedded-detail-head">
        <Link to={`/${slug}/${id}`}>{displayValue(row, meta.display_field)}</Link>
        {actions.length ? (
          <ActionBar
            actions={actions}
            onAction={async (name) => {
              await api.action(slug, id, name, {});
              setTick((n) => n + 1);
            }}
            compact
          />
        ) : null}
      </div>
      <dl className="detail-grid compact">
        {visible.slice(0, 12).map((field) => (
          <div key={field.name}>
            <dt>{field.label}</dt>
            <dd>
              <FieldValue row={row} field={field} />
            </dd>
          </div>
        ))}
      </dl>
      {Object.entries(related).map(([name, rel]) => (
        <div key={name} className="related panel">
          <h4>{name}</h4>
          {rel.items.length === 0 ? (
            <EmptyState title="No related records." />
          ) : (
            <ul>
              {rel.items.slice(0, 8).map((item) => (
                <li key={String(item.id)}>
                  <Link to={`/${rel.slug}/${item.id}`}>{displayValue(item)}</Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}
      {showActivity ? <Timeline items={activity} timezone={theme.timezone} locale={theme.locale} /> : null}
      {showAttachments ? (
        <AttachmentsPanel
          slug={slug}
          id={id}
          items={attachments}
          onChanged={() => setTick((n) => n + 1)}
        />
      ) : null}
    </div>
  );
}
