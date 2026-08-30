import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  api,
  detailVisible,
  type EntityAction,
  type UiEntity,
  type WorkflowAction,
} from "../sdk/client";
import { ActionBar } from "../components/actions/ActionBar";
import { AttachmentsPanel } from "../components/attachments/AttachmentsPanel";
import { Timeline } from "../components/timeline/Timeline";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { ActionMenu } from "../components/ui/ActionMenu";
import { FieldValue } from "../components/fields/FieldValue";
import { StatusBadge } from "../components/ui/StatusBadge";
import { friendlyError } from "../friendlyError";
import { relativeTime } from "../format";
import { useTenantTheme } from "../metadata/context";
import { canDeleteRecord, canUpdateRecord, displayValue } from "../metadata/views";
import { useBreadcrumbRecord } from "../components/shell/breadcrumbContext";
import { useRealtime } from "../realtime";

export default function EntityDetail({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [row, setRow] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [activity, setActivity] = useState<Array<Record<string, unknown>>>([]);
  const [attachments, setAttachments] = useState<Array<Record<string, unknown>>>([]);
  const [comment, setComment] = useState("");
  const [commentBusy, setCommentBusy] = useState(false);
  const theme = useTenantTheme();
  const [tab, setTab] = useState("overview");
  const { setRecord } = useBreadcrumbRecord();

  async function load() {
    if (!slug || !id) return;
    const data = await api.get(slug, id);
    setRow(data);
    if (meta) {
      setRecord(recordCrumb(meta, entities, data));
      api
        .activity(slug, id)
        .then((d) => setActivity(d.items ?? []))
        .catch(() => setActivity([]));
      if (meta.attachments) {
        api
          .attachments(slug, id)
          .then((d) => setAttachments(d.items ?? []))
          .catch(() => setAttachments([]));
      }
    }
  }

  useEffect(() => {
    load().catch((e) => setError(friendlyError(e)));
    return () => setRecord(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, id]);

  useRealtime({ entity: meta?.entity, recordId: id }, () => {
    load().catch(() => undefined);
  });

  if (!meta || !slug || !id) return <ErrorState message="Unknown entity." />;
  if (!row && !error) return <Skeleton rows={6} />;
  if (!row) return <ErrorState message={error || "Unable to load record."} />;

  const workflow = row._workflow as
    | { current?: string; transitions?: WorkflowAction[] }
    | undefined;
  const actions = (row._actions as EntityAction[] | undefined) ?? [];
  const related = (row._related ?? {}) as Record<
    string,
    { slug: string; entity: string; label?: string; items: Record<string, unknown>[]; total: number }
  >;
  const links = ((row._links as Array<{
    label: string;
    entity: string;
    slug: string;
    relation: string;
    total: number;
  }>) ?? []).filter((l) => !related[l.relation]);
  const visible = meta.fields.filter(
    (f) =>
      detailVisible(f) &&
      f.relation_kind !== "one_to_many" &&
      f.relation_kind !== "child_table" &&
      f.type !== "child_table",
  );
  const childTables = meta.fields.filter(
    (f) => f.relation_kind === "child_table" || f.type === "child_table",
  );
  const number =
    row[meta.naming?.field || ""] ??
    row.name ??
    row.code ??
    row.title ??
    id;
  const owner = row.owner ?? row.created_by ?? row.modified_by;
  const created = row.created_at;
  const statusField = meta.fields.find((f) => f.widget === "status" || f.name === "status");
  const status = workflow?.current ?? row.status;

  const caps = meta.capabilities;
  const showActivity = caps?.activity !== false;
  const showComments = caps?.comments !== false;
  const showAttachments = Boolean(meta.attachments || caps?.attachments);
  const showRelated = links.length > 0 || Object.keys(related).length > 0;

  const chrome = [
    { id: "overview", label: "Details" },
    ...childTables.map((c) => ({ id: `items:${c.name}`, label: c.label })),
    { id: "related", label: "Related records", hide: !showRelated },
    { id: "files", label: "Attachments", hide: !showAttachments },
    { id: "activity", label: "Activity", hide: !showActivity },
  ].filter((t) => !t.hide);

  async function runAction(name: string) {
    if (!slug || !id) return;
    try {
      await api.action(slug, id, name);
      setError("");
      await load();
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  return (
    <div className="page">
      <PageHeader
        kicker={meta.entity}
        title={
          <>
            {meta.label} {String(number)}
          </>
        }
        description={
          <>
            {status ? <StatusBadge value={status} indicators={statusField?.widget_options?.indicators} /> : null}
            {owner ? <span> · {String(owner)}</span> : null}
            {created ? <span> · {relativeTime(created, theme.locale)}</span> : null}
          </>
        }
        actions={
          <>
            {canUpdateRecord(meta, row) ? (
              <Link to={`/${slug}/${id}/edit`}>
                <button type="button">Edit</button>
              </Link>
            ) : null}
            <ActionBar
              actions={actions}
              transitions={workflow?.transitions}
              onAction={(name) => void runAction(name)}
              onTransition={async (name) => {
                try {
                  const next = await api.transition(slug, id, name);
                  setRow(next);
                  setError("");
                  await load();
                } catch (e) {
                  setError(friendlyError(e));
                }
              }}
            />
            <ActionMenu
              items={[
                { key: "print", label: "Print", href: `/api/v1/${slug}/${id}/print`, target: "_blank" },
                { key: "pdf", label: "Download PDF", href: `/api/v1/${slug}/${id}/print.pdf`, target: "_blank" },
                {
                  key: "delete",
                  label: "Delete",
                  danger: true,
                  hidden: !canDeleteRecord(meta, row),
                  onSelect: async () => {
                    if (!confirm("Delete this record?")) return;
                    try {
                      await api.remove(slug, id);
                      navigate(`/${slug}`);
                    } catch (e) {
                      setError(friendlyError(e));
                    }
                  },
                },
              ]}
            />
          </>
        }
      />
      {error && <ErrorState message={error} onRetry={() => void load()} />}
      {statusField?.enum_values && statusField.enum_values.length > 1 ? (
        <ol className="wf-strip" aria-label="Status">
          {statusField.enum_values.map((step) => (
            <li key={step} className={String(status) === step ? "is-current" : undefined}>
              {step}
            </li>
          ))}
        </ol>
      ) : null}
      <div className="tabs" role="tablist">
        {chrome.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "is-active" : "ghost"}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === "overview" ? (
        <Overview
          row={row}
          fields={visible}
          entities={entities}
          sections={meta.views?.detail?.sections}
        />
      ) : null}
      {childTables.map((field) =>
        tab === `items:${field.name}` ? (
          <ChildPanel key={field.name} field={field} row={row} entities={entities} />
        ) : null,
      )}
      {tab === "related" ? (
        <RelatedPanel links={links} related={related} id={id} meta={meta} entities={entities} />
      ) : null}
      {tab === "files" && showAttachments ? (
        <div className="panel" style={{ padding: "0.85rem" }}>
          <h3>Attachments</h3>
          <AttachmentsPanel slug={slug} id={id} items={attachments} onChanged={() => void load()} />
        </div>
      ) : null}
      {tab === "activity" && showActivity ? (
        <div className="panel" style={{ padding: "0.85rem" }}>
          <h3>Activity</h3>
          {showComments ? (
            <form
              className="comment-form"
              onSubmit={async (e) => {
                e.preventDefault();
                if (!comment.trim() || commentBusy) return;
                setCommentBusy(true);
                try {
                  await api.addComment(slug, id, comment.trim());
                  setComment("");
                  await load();
                } catch (err) {
                  setError(friendlyError(err));
                } finally {
                  setCommentBusy(false);
                }
              }}
            >
              <label>
                Add comment
                <textarea
                  value={comment}
                  onChange={(e) => setComment(e.target.value)}
                  rows={3}
                  placeholder="Write a comment…"
                />
              </label>
              <button type="submit" disabled={commentBusy || !comment.trim()}>
                Comment
              </button>
            </form>
          ) : null}
          <Timeline
            items={activity.map((item) => ({
              id: String(item.id ?? ""),
              action: String(item.activity_type ?? item.action ?? item.event ?? "updated"),
              activity_type: item.activity_type ? String(item.activity_type) : undefined,
              actor: item.actor_name
                ? String(item.actor_name)
                : item.actor
                  ? String(item.actor)
                  : item.user
                    ? String(item.user)
                    : undefined,
              actor_name: item.actor_name ? String(item.actor_name) : undefined,
              created_at: item.created_at,
              summary: String(item.message ?? item.summary ?? item.action ?? item.event ?? "updated"),
              message: item.message ? String(item.message) : undefined,
            }))}
            timezone={theme.timezone}
            locale={theme.locale}
          />
        </div>
      ) : null}
    </div>
  );
}

function Overview({
  row,
  fields,
  entities,
  sections: spec,
}: {
  row: Record<string, unknown>;
  fields: UiEntity["fields"];
  entities: UiEntity[];
  sections?: Array<{ title: string; fields?: string[] }>;
}) {
  const sections =
    spec && spec.length
      ? spec
          .map((s) => [s.title, fields.filter((f) => (s.fields ?? []).includes(f.name))] as const)
          .filter(([, fs]) => fs.length)
      : group(fields);
  if (fields.length === 0) return <EmptyState title="Nothing to show" />;
  return (
    <>
      {sections.map(([section, sectionFields]) => (
        <div key={section || "default"} className="panel">
          {section ? <h3 className="panel-title">{section}</h3> : null}
          <table className="dl">
            <tbody>
              {sectionFields.map((f) => (
                <tr key={f.name}>
                  <th>{f.label}</th>
                  <td>
                    <FieldValue row={row} field={f} entities={entities} linkRelations relativeDates={false} compact={false} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </>
  );
}

function ChildPanel({
  field,
  row,
  entities,
}: {
  field: UiEntity["fields"][number];
  row: Record<string, unknown>;
  entities: UiEntity[];
}) {
  const items = (Array.isArray(row[field.name]) ? row[field.name] : []) as Record<string, unknown>[];
  const child = entities.find((e) => e.entity === (field.child_entity || field.relation));
  const cols = (child?.fields ?? []).filter(
    (f) => !f.hidden && f.list !== false && f.relation_kind !== "one_to_many",
  );
  const standalone = child?.standalone !== false;
  return (
    <div className="panel">
      <h3 className="panel-title">{field.label}</h3>
      {items.length === 0 ? (
        <EmptyState title={`No ${field.label.toLowerCase()} yet`} />
      ) : (
        <div className="child-table">
        <table className="data">
          <thead>
            <tr>
              {cols.map((c) => (
                <th key={c.name}>{c.label}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {items.map((item, i) => (
              <tr key={String(item.id ?? i)}>
                {cols.map((c, colIdx) => {
                  const canOpen = Boolean(standalone && item.id && colIdx === 0 && child);
                  return (
                    <td key={c.name} data-label={c.label}>
                      {canOpen ? (
                        <Link to={`/${child!.slug}/${item.id}`}>
                          <FieldValue row={item} field={c} compact />
                        </Link>
                      ) : (
                        <FieldValue row={item} field={c} entities={entities} linkRelations compact />
                      )}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      )}
    </div>
  );
}

function RelatedPanel({
  links,
  related,
  id,
  meta,
  entities,
}: {
  links: Array<{ label: string; slug: string; relation: string; total: number }>;
  related: Record<string, { slug: string; items: Record<string, unknown>[]; total: number; label?: string }>;
  id: string;
  meta: UiEntity;
  entities: UiEntity[];
}) {
  return (
    <>
      {links.length > 0 ? (
        <div className="panel related">
          <h3 className="panel-title">Related</h3>
          <ul className="related-tree">
            {links.map((link) => (
              <li key={`${link.slug}-${link.relation}`}>
                <Link to={`/${link.slug}?${encodeURIComponent(link.relation)}=${id}`}>
                  {link.label} ({link.total})
                </Link>
                {" · "}
                <Link to={`/${link.slug}/new?${encodeURIComponent(link.relation)}=${id}`}>Add</Link>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {Object.entries(related).map(([name, rel]) => {
        const fieldMeta = meta.fields.find((f) => f.name === name);
        const inverse =
          fieldMeta?.inverse_field ||
          entities.find((e) => e.slug === rel.slug)?.fields.find(
            (f) => f.relation === meta.entity && f.relation_kind === "many_to_one",
          )?.name;
        const title = rel.label || fieldMeta?.label || name;
        return (
          <div key={name} className="related panel">
            <h3 className="panel-title">{title}</h3>
            <p className="muted related-meta">
              {rel.total} related
              {inverse ? (
                <>
                  {" · "}
                  <Link to={`/${rel.slug}/new?${encodeURIComponent(inverse)}=${id}`}>Add</Link>
                </>
              ) : null}
            </p>
            {rel.items.length === 0 ? (
              <EmptyState title="No related records." />
            ) : (
              <ul>
                {rel.items.map((item) => (
                  <li key={String(item.id)}>
                    <Link to={`/${rel.slug}/${item.id}`}>
                      {String(item.name ?? item.title ?? item.code ?? item.id)}
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </div>
        );
      })}
      {links.length === 0 && Object.keys(related).length === 0 ? (
        <EmptyState title="No related records" />
      ) : null}
    </>
  );
}

function recordCrumb(meta: UiEntity, entities: UiEntity[], row: Record<string, unknown>) {
  const label = displayValue(row, meta.display_field);
  const parentEntity = meta.child_of
    ? entities.find((e) => e.entity === meta.child_of)
    : undefined;
  const fk = parentEntity
    ? meta.fields.find(
        (f) => f.relation === parentEntity.entity && f.relation_kind === "many_to_one",
      )
    : undefined;
  const expanded = row._expanded as
    | Record<string, { id: string; label: string; slug: string }>
    | undefined;
  const parentRel = fk ? expanded?.[fk.name] : undefined;
  const parentId = parentRel?.id ?? (fk ? String(row[fk.name] ?? "") : "");
  return {
    id: String(row.id ?? ""),
    label: label || String(row.id ?? "Record"),
    parent:
      parentEntity && parentId
        ? {
            slug: parentEntity.slug,
            id: parentId,
            label: parentRel?.label || parentId,
            entityLabel: parentEntity.label,
          }
        : undefined,
  };
}

function group(fields: UiEntity["fields"]) {
  const map = new Map<string, typeof fields>();
  for (const f of fields) {
    const key = f.section ?? "";
    const list = map.get(key) ?? [];
    list.push(f);
    map.set(key, list);
  }
  return Array.from(map.entries());
}
