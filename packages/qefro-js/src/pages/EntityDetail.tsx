import { useEffect, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
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
import AutomationRuns from "../components/automation/AutomationRuns";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { SectionHeader } from "../components/ui/SectionHeader";
import { ActionMenu } from "../components/ui/ActionMenu";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { showSnackbar } from "../components/ui/Snackbar";
import { FieldValue } from "../components/fields/FieldValue";
import { StatusBadge } from "../components/ui/StatusBadge";
import { friendlyError } from "../friendlyError";
import { relativeTime } from "../format";
import { useTenantTheme } from "../metadata/context";
import { canDeleteRecord, canUpdateRecord, displayValue } from "../metadata/views";
import { resolveLayout } from "../metadata/layout";
import { t } from "../i18n";
import { resolveEntity } from "../metadata/navigation";
import { emitUiEvent } from "../core/events";
import { useBreadcrumbRecord } from "../components/shell/breadcrumbContext";
import { useRealtime } from "../realtime";

export default function EntityDetail({ entities, entity }: { entities: UiEntity[]; entity?: string }) {
  const { slug: routeSlug, id } = useParams();
  const meta = resolveEntity(entities, entity ?? routeSlug);
  const slug = meta?.slug ?? routeSlug;
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const [row, setRow] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [activity, setActivity] = useState<Array<Record<string, unknown>>>([]);
  const [attachments, setAttachments] = useState<Array<Record<string, unknown>>>([]);
  const [comment, setComment] = useState("");
  const [commentFile, setCommentFile] = useState<File | null>(null);
  const [commentBusy, setCommentBusy] = useState(false);
  const [pending, setPending] = useState<"delete" | "archive" | "restore" | null>(null);
  const [preview, setPreview] = useState(false);
  const [previewHtml, setPreviewHtml] = useState("");
  const [previewBusy, setPreviewBusy] = useState(false);
  const [sendOpen, setSendOpen] = useState(false);
  const [sendTemplate, setSendTemplate] = useState("");
  const [sendChannel, setSendChannel] = useState("");
  const [comms, setComms] = useState<Array<Record<string, unknown>>>([]);
  const [busy, setBusy] = useState(false);
  const theme = useTenantTheme();
  const [tab, setTab] = useState(params.get("tab") || "overview");
  const { setRecord } = useBreadcrumbRecord();

  async function load() {
    if (!slug || !id) return;
    const data = await api.get(slug, id);
    setRow(data);
    if (meta) {
      setRecord(recordCrumb(meta, entities, data));
    }
  }

  useEffect(() => {
    load().catch((e) => setError(friendlyError(e)));
    return () => setRecord(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, id]);

  useEffect(() => {
    if (!slug || !id || !row) return;
    if (tab === "activity") {
      api
        .activity(slug, id)
        .then((d) => setActivity(d.items ?? []))
        .catch(() => setActivity([]));
    }
    if (tab === "files" && meta?.attachments) {
      api
        .attachments(slug, id)
        .then((d) => setAttachments(d.items ?? []))
        .catch(() => setAttachments([]));
    }
    if (tab === "communication" && meta?.capabilities?.communication) {
      api
        .communications(slug, id)
        .then((d) => setComms(d.items ?? []))
        .catch(() => setComms([]));
    }
  }, [tab, slug, id, row, meta?.attachments]);

  useRealtime({ entity: meta?.entity, recordId: id }, () => {
    load().catch(() => undefined);
  });

  if (!meta || !slug || !id) return <ErrorState message="Unknown entity." />;
  if (!row && !error) return <Skeleton rows={6} variant="detail" />;
  if (!row) return <ErrorState message={error || "Unable to load record."} />;

  const workflow = row._workflow as
    | { current?: string; transitions?: WorkflowAction[] }
    | undefined;
  const actions = ((row._actions as EntityAction[] | undefined) ?? []).filter(
    (a) =>
      a.name !== "generate_document" &&
      a.name !== "generate-document" &&
      a.name !== "send_communication" &&
      a.name !== "send-communication",
  );
  const related = (row._related ?? {}) as Record<
    string,
    {
      slug: string;
      entity: string;
      label?: string;
      items: Record<string, unknown>[];
      total: number;
      filters?: Array<{ field: string; value: string }>;
    }
  >;
  const links = (
    (row._links as Array<{
      label: string;
      entity: string;
      slug: string;
      relation: string;
      total: number;
      filters?: Array<{ field: string; value: string }>;
    }>) ?? []
  ).filter((l) => !related[l.relation]);
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
  const allowArchive = Boolean(caps?.archive) && canUpdateRecord(meta, row);
  const allowPrint = Boolean(caps?.print);
  const allowCommunication = Boolean(caps?.communication);
  const templates = meta.communications ?? [];
  const archived = Boolean(row.archived_at);

  const chrome = [
    { id: "overview", label: "Details" },
    ...childTables.map((c) => ({ id: `items:${c.name}`, label: c.label })),
    { id: "related", label: "Related records", hide: !showRelated },
    { id: "files", label: "Attachments", hide: !showAttachments },
    { id: "communication", label: "Communication", hide: !allowCommunication },
    { id: "activity", label: "Activity", hide: !showActivity },
  ].filter((t) => !t.hide);

  async function runAction(name: string, action?: EntityAction, input: Record<string, unknown> = {}) {
    if (!slug || !id) return;
    try {
      const result = await api.action(slug, id, name, input);
      setError("");
      const nav = result._operation as
        | { navigate?: { slug?: string; id?: string }; message?: string; status?: string }
        | undefined;
      if (nav?.navigate?.slug && nav.navigate.id) {
        showSnackbar(nav.message || (action?.label ? `${action.label} done` : "Done"));
        navigate(`/${nav.navigate.slug}/${nav.navigate.id}`);
        return;
      }
      await load();
      showSnackbar(nav?.message || (action?.label ? `${action.label} done` : "Done"));
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  async function runLifecycle(action: "delete" | "archive" | "restore") {
    if (!slug || !id || busy) return;
    setBusy(true);
    try {
      if (action === "delete") {
        await api.remove(slug, id);
        showSnackbar(t("bulk.done.delete", { count: meta.label.toLowerCase() }));
        emitUiEvent("entity:deleted", { entity: meta.entity, slug, id });
        navigate(`/${slug}`);
        return;
      }
      await api.bulk(slug, { action, ids: [id] });
      showSnackbar(
        t(action === "archive" ? "bulk.done.archive" : "bulk.done.restore", {
          count: meta.label.toLowerCase(),
        }),
      );
      setPending(null);
      if (action === "archive") navigate(`/${slug}`);
      else await load();
    } catch (e) {
      setError(friendlyError(e));
      setPending(null);
    } finally {
      setBusy(false);
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
            {allowPrint ? (
              <>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => {
                    setPreviewBusy(true);
                    setPreview(true);
                    api
                      .printHtml(slug, id)
                      .then((html) => setPreviewHtml(html))
                      .catch((e) => setError(friendlyError(e)))
                      .finally(() => setPreviewBusy(false));
                  }}
                >
                  Print
                </button>
                <button
                  type="button"
                  className="ghost"
                  onClick={() =>
                    void api.downloadPdf(slug, id).catch((e) => setError(friendlyError(e)))
                  }
                >
                  Download PDF
                </button>
              </>
            ) : null}
            {allowCommunication ? (
              <button
                type="button"
                className="ghost"
                onClick={() => {
                  setSendTemplate(templates[0]?.name ?? "");
                  setSendChannel(templates[0]?.channels?.[0] ?? "");
                  setSendOpen(true);
                }}
              >
                Send message
              </button>
            ) : null}
            <ActionBar
              actions={actions}
              transitions={workflow?.transitions}
              onAction={(name, action, input) => void runAction(name, action, input)}
              onTransition={async (name) => {
                try {
                  const next = await api.transition(slug, id, name);
                  setRow(next);
                  setError("");
                  await load();
                  showSnackbar("Updated");
                } catch (e) {
                  setError(friendlyError(e));
                }
              }}
            />
            <ActionMenu
              items={[
                {
                  key: "print",
                  label: "Print",
                  hidden: !allowPrint,
                  onSelect: () => {
                    setPreviewBusy(true);
                    setPreview(true);
                    api
                      .printHtml(slug, id)
                      .then((html) => setPreviewHtml(html))
                      .catch((e) => setError(friendlyError(e)))
                      .finally(() => setPreviewBusy(false));
                  },
                },
                {
                  key: "pdf",
                  label: "Download PDF",
                  hidden: !allowPrint,
                  onSelect: () => {
                    void api.downloadPdf(slug, id).catch((e) => setError(friendlyError(e)));
                  },
                },
                {
                  key: "send-communication",
                  label: "Send message",
                  hidden: !allowCommunication,
                  onSelect: () => {
                    setSendTemplate(templates[0]?.name ?? "");
                    setSendChannel(templates[0]?.channels?.[0] ?? "");
                    setSendOpen(true);
                  },
                },
                {
                  key: "attach-pdf",
                  label: "Attach PDF",
                  hidden: !allowPrint || !showAttachments,
                  onSelect: () => {
                    void runAction("generate_document");
                  },
                },
                {
                  key: "archive",
                  label: archived ? "Restore" : "Archive",
                  hidden: !allowArchive,
                  onSelect: () => setPending(archived ? "restore" : "archive"),
                },
                {
                  key: "delete",
                  label: "Delete",
                  danger: true,
                  hidden: !canDeleteRecord(meta, row),
                  onSelect: () => setPending("delete"),
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
            onClick={() => {
              setTab(t.id);
              const next = new URLSearchParams(params);
              if (t.id === "overview") next.delete("tab");
              else next.set("tab", t.id);
              setParams(next, { replace: true });
            }}
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
          sections={meta.views?.detail?.sections ?? meta.views?.form?.sections}
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
        <div className="panel detail-panel">
          <SectionHeader title="Attachments" />
          <AttachmentsPanel slug={slug} id={id} items={attachments} onChanged={() => void load()} />
        </div>
      ) : null}
      {tab === "communication" && allowCommunication ? (
        <div className="panel detail-panel">
          <SectionHeader title="Communication" />
          {comms.length === 0 ? (
            <p className="muted">No messages yet.</p>
          ) : (
            <ul className="stack">
              {comms.map((item) => (
                <li key={String(item.id)} className="row-line">
                  <strong>{String(item.channel ?? "")}</strong>
                  <StatusBadge value={item.status} />
                  <span className="muted">{String(item.template ?? "")}</span>
                  <span className="muted">{String(item.recipient ?? "")}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
      {tab === "activity" && showActivity ? (
        <div className="panel detail-panel">
          <SectionHeader title="Activity" />
          {showComments ? (
            <form
              className="comment-form"
              onSubmit={async (e) => {
                e.preventDefault();
                if ((!comment.trim() && !commentFile) || commentBusy) return;
                setCommentBusy(true);
                try {
                  let attachmentId: string | undefined;
                  if (commentFile && slug && id) {
                    const uploaded = await api.uploadAttachment(slug, id, commentFile);
                    attachmentId = String(uploaded.id ?? "");
                  }
                  await api.addComment(slug, id, comment.trim() || `Attached ${commentFile?.name ?? "file"}`, attachmentId);
                  setComment("");
                  setCommentFile(null);
                  await load();
                  const activityData = await api.activity(slug, id);
                  setActivity(activityData.items ?? []);
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
                  placeholder={t("comment.placeholder")}
                />
              </label>
              <button type="submit" disabled={commentBusy || (!comment.trim() && !commentFile)}>
                Send comment
              </button>
              <label className="comment-attach">
                <span className="sr-only">Attach file</span>
                <input
                  type="file"
                  onChange={(e) => setCommentFile(e.target.files?.[0] ?? null)}
                />
                {commentFile ? <span className="muted">📎 {commentFile.name}</span> : <span className="muted">📎 Attach file</span>}
              </label>
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
              filename: (() => {
                const meta = item.metadata as Record<string, unknown> | undefined;
                return meta?.filename ? String(meta.filename) : undefined;
              })(),
            }))}
            timezone={theme.timezone}
            locale={theme.locale}
          />
          <SectionHeader title="Automation Runs" />
          <AutomationRuns entity={meta.entity} recordId={id} />
        </div>
      ) : null}
      <ConfirmDialog
        open={preview}
        title={`${meta.label} preview`}
        confirmLabel="Print"
        cancelLabel="Close"
        confirmDisabled={previewBusy || !previewHtml}
        className="print-preview-dialog"
        onCancel={() => {
          setPreview(false);
          setPreviewHtml("");
        }}
        onConfirm={() => {
          const frame = document.querySelector<HTMLIFrameElement>("iframe.print-preview");
          frame?.contentWindow?.focus();
          frame?.contentWindow?.print();
        }}
      >
        {previewBusy ? <p>Loading document…</p> : null}
        {previewHtml ? (
          <iframe title="Document preview" className="print-preview" srcDoc={previewHtml} />
        ) : null}
      </ConfirmDialog>
      <ConfirmDialog
        open={sendOpen}
        title="Send message"
        confirmLabel="Queue"
        cancelLabel="Cancel"
        confirmDisabled={!sendTemplate}
        onCancel={() => setSendOpen(false)}
        onConfirm={() => {
          const body: { template: string; channel?: string } = { template: sendTemplate };
          if (sendChannel) body.channel = sendChannel;
          void api
            .sendCommunication(slug, id, body)
            .then((res) => {
              setSendOpen(false);
              showSnackbar(res.message || "Communication queued");
              if (tab === "communication") {
                return api.communications(slug, id).then((d) => setComms(d.items ?? []));
              }
            })
            .catch((e) => setError(friendlyError(e)));
        }}
      >
        <label>
          Template
          <select
            value={sendTemplate}
            onChange={(e) => {
              const name = e.target.value;
              setSendTemplate(name);
              const next = templates.find((t) => t.name === name);
              setSendChannel(next?.channels?.[0] ?? "");
            }}
          >
            {templates.map((t) => (
              <option key={t.name} value={t.name}>
                {t.name.replaceAll("_", " ")}
              </option>
            ))}
          </select>
        </label>
        <label>
          Channel
          <select value={sendChannel} onChange={(e) => setSendChannel(e.target.value)}>
            {(templates.find((t) => t.name === sendTemplate)?.channels ?? ["email"]).map((ch) => (
              <option key={ch} value={ch}>
                {ch}
              </option>
            ))}
          </select>
        </label>
      </ConfirmDialog>
      <ConfirmDialog
        open={pending === "delete"}
        title={t("record.deleteTitle", { entity: meta.label })}
        message={t("record.deleteConfirm", { entity: meta.label.toLowerCase() })}
        confirmLabel="Delete"
        danger
        confirmDisabled={busy}
        onCancel={() => setPending(null)}
        onConfirm={() => void runLifecycle("delete")}
      />
      <ConfirmDialog
        open={pending === "archive"}
        title={t("record.archiveTitle", { entity: meta.label })}
        message={t("record.archiveConfirm", { entity: meta.label.toLowerCase() })}
        confirmLabel="Archive"
        confirmDisabled={busy}
        onCancel={() => setPending(null)}
        onConfirm={() => void runLifecycle("archive")}
      />
      <ConfirmDialog
        open={pending === "restore"}
        title={t("record.restoreTitle", { entity: meta.label })}
        message={t("record.restoreConfirm", { entity: meta.label.toLowerCase() })}
        confirmLabel="Restore"
        confirmDisabled={busy}
        onCancel={() => setPending(null)}
        onConfirm={() => void runLifecycle("restore")}
      />
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
  sections?: Array<{
    title: string;
    fields?: string[];
    columns?: Array<{ fields?: string[] }>;
    visible_when?: { field: string; equals: unknown };
    tab?: string;
  }>;
}) {
  const layout = resolveLayout(fields, spec, row);
  if (fields.length === 0 && layout.sections.length === 0) return <EmptyState title="Nothing to show" />;
  return (
    <>
      {layout.sections.map((section) => (
        <section key={`${section.tab}-${section.title || "default"}`} className="section-block">
          {section.title ? <SectionHeader title={section.title} /> : <SectionHeader title="Details" />}
          <div className={section.columns.length > 1 ? "form-columns" : undefined}>
            {section.columns.map((col, i) => (
              <table key={i} className="dl">
                <tbody>
                  {col.fields.map((f) => (
                    <tr key={f.name}>
                      <th>{f.label}</th>
                      <td>
                        <FieldValue row={row} field={f} entities={entities} linkRelations relativeDates={false} compact={false} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ))}
          </div>
        </section>
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
  const named = field.widget_options?.column_fields;
  const allCols = (child?.fields ?? []).filter(
    (f) => !f.hidden && f.list !== false && f.relation_kind !== "one_to_many",
  );
  const cols = named?.length
    ? named.map((n) => allCols.find((c) => c.name === n)).filter((c): c is NonNullable<typeof c> => Boolean(c))
    : allCols;
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

function relatedCreateHref(
  slug: string,
  id: string,
  relation?: string,
  filters?: Array<{ field: string; value: string }>,
) {
  const params = new URLSearchParams();
  if (relation) params.set(relation, id);
  for (const filter of filters ?? []) {
    if (filter.field && filter.value) params.set(filter.field, filter.value);
  }
  const qs = params.toString();
  return qs ? `/${slug}/new?${qs}` : `/${slug}/new`;
}

function RelatedPanel({
  links,
  related,
  id,
  meta,
  entities,
}: {
  links: Array<{
    label: string;
    slug: string;
    relation: string;
    total: number;
    columns?: string[];
    filters?: Array<{ field: string; value: string }>;
  }>;
  related: Record<
    string,
    {
      slug: string;
      items: Record<string, unknown>[];
      total: number;
      label?: string;
      columns?: string[];
      filters?: Array<{ field: string; value: string }>;
    }
  >;
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
                <Link to={relatedCreateHref(link.slug, id, link.relation, link.filters)}>Add</Link>
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
        const filters = rel.filters;
        return (
          <div key={name} className="related panel">
            <div className="related-head">
              <h3 className="panel-title">{title}</h3>
              <p className="muted related-meta">
                {rel.total} related
                {inverse ? (
                  <>
                    {" · "}
                    <Link to={relatedCreateHref(rel.slug, id, inverse, filters)}>Add</Link>
                  </>
                ) : null}
                {" · "}
                <Link to={`/${rel.slug}?${encodeURIComponent(inverse || name)}=${id}`}>View all</Link>
              </p>
            </div>
            {rel.items.length === 0 ? (
              <EmptyState title="No related records." />
            ) : rel.columns && rel.columns.length > 0 ? (
              <div className="table-wrap">
                <table className="data">
                  <thead>
                    <tr>
                      {rel.columns.map((col) => (
                        <th key={col}>{col.replace(/_/g, " ")}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {rel.items.map((item) => (
                      <tr key={String(item.id)}>
                        {rel.columns!.map((col, colIdx) => (
                          <td key={col}>
                            {colIdx === 0 ? (
                              <Link to={`/${rel.slug}/${item.id}`}>
                                {String(item[col] ?? item.name ?? item.title ?? item.code ?? item.doc_no ?? item.id)}
                              </Link>
                            ) : col === "status" ? (
                              <StatusBadge value={item.status} />
                            ) : (
                              String(item[col] ?? "")
                            )}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="table-wrap">
                <table className="data">
                  <tbody>
                    {rel.items.map((item) => (
                      <tr key={String(item.id)}>
                        <td>
                          <Link to={`/${rel.slug}/${item.id}`}>
                            {String(item.name ?? item.title ?? item.code ?? item.doc_no ?? item.id)}
                          </Link>
                        </td>
                        {item.status != null && item.status !== "" ? (
                          <td>
                            <StatusBadge value={item.status} />
                          </td>
                        ) : null}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
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
