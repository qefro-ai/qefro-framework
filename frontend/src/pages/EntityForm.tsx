import { FormEvent, useEffect, useMemo, useState } from "react";
import { Link, useBlocker, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { api, ApiError, formVisible, type UiEntity, type UiField } from "../sdk/client";
import { FormLayout } from "../components/forms/FormLayout";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { showSnackbar } from "../components/ui/Snackbar";
import { friendlyError } from "../friendlyError";
import { t } from "../i18n";
import { previewFormula } from "../metadata/formula";
import { fieldReadonly } from "../metadata/conditions";
import { displayValue } from "../metadata/views";
import { useBreadcrumbRecord } from "../components/shell/breadcrumbContext";

const DRAFT_PREFIX = "qefro:form-draft:";

function draftKey(slug: string, id?: string) {
  return `${DRAFT_PREFIX}${slug}:${id || "new"}`;
}

function readDraft(slug: string, id?: string): Record<string, unknown> | null {
  try {
    const raw = sessionStorage.getItem(draftKey(slug, id));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

function writeDraft(slug: string, id: string | undefined, values: Record<string, unknown>) {
  sessionStorage.setItem(draftKey(slug, id), JSON.stringify(values));
}

function clearDraft(slug: string, id?: string) {
  sessionStorage.removeItem(draftKey(slug, id));
}

function safeReturnPath(raw: string | null): string | null {
  if (!raw) return null;
  if (!raw.startsWith("/") || raw.startsWith("//")) return null;
  return raw;
}

function draftTargetFromPath(pathname: string): { slug: string; id?: string } | null {
  const parts = pathname.split("/").filter(Boolean);
  if (parts.length >= 2 && parts[1] === "new") return { slug: parts[0] };
  if (parts.length >= 3 && parts[2] === "edit") return { slug: parts[0], id: parts[1] };
  return null;
}

function previewDefault(field: UiField): unknown {
  if (field.default != null && field.default !== "") return field.default;
  if (field.default_from === "current_date") return new Date().toISOString().slice(0, 10);
  if (field.default_from === "current_datetime") return new Date().toISOString();
  return "";
}

export default function EntityForm({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  return <EntityFormEditor key={`${slug}:${id ?? "new"}`} entities={entities} />;
}

function EntityFormEditor({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const [searchParams] = useSearchParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(Boolean(id));
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [focusField, setFocusField] = useState<string | null>(null);
  const [focusSeq, setFocusSeq] = useState(0);
  const { setRecord } = useBreadcrumbRecord();
  const returnTo = safeReturnPath(searchParams.get("return"));
  const returnField = searchParams.get("return_field");

  const baseFields = useMemo(
    () => (meta?.fields.filter(formVisible) ?? []).filter((f) => f.relation_kind !== "one_to_many"),
    [meta],
  );
  const fields = useMemo(() => {
    const status = String(values.status ?? "");
    const locked = Boolean(status && meta?.document?.lock_states?.includes(status));
    if (!locked) return baseFields;
    return baseFields.map((f) => (f.allow_on_submit ? f : { ...f, readonly: true }));
  }, [baseFields, meta, values.status]);

  const layout = meta?.views?.form?.sections ?? meta?.views?.detail?.sections;

  function revealField(name: string) {
    setFocusField(name);
    setFocusSeq((n) => n + 1);
  }

  useEffect(() => {
    setSaved(false);
    setError("");
    setFieldErrors({});
    if (id && slug) {
      setLoading(true);
      api
        .get(slug, id)
        .then((row) => {
          const next: Record<string, unknown> = {};
          for (const field of baseFields) {
            next[field.name] = row[field.name] ?? "";
          }
          const draft = readDraft(slug, id);
          if (draft) Object.assign(next, draft);
          for (const field of baseFields) {
            const q = searchParams.get(field.name);
            if (q) next[field.name] = q;
          }
          setValues(next);
          setUpdatedAt(typeof row.updated_at === "string" ? row.updated_at : null);
          setConflict(false);
          setDirty(Boolean(draft));
          if (meta) setRecord({ id, label: displayValue(row, meta.display_field) || id });
        })
        .catch((e) => setError(friendlyError(e)))
        .finally(() => setLoading(false));
    } else if (slug) {
      const next: Record<string, unknown> = {};
      for (const field of baseFields) {
        next[field.name] = previewDefault(field);
      }
      const draft = readDraft(slug, undefined);
      if (draft) Object.assign(next, draft);
      for (const field of baseFields) {
        const q = searchParams.get(field.name);
        if (q) next[field.name] = q;
      }
      setValues(next);
      setDirty(Boolean(draft));
      setLoading(false);
    }
  }, [id, slug, baseFields, searchParams]);

  useEffect(() => {
    const onBefore = (e: BeforeUnloadEvent) => {
      if (!dirty || saving) return;
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", onBefore);
    return () => window.removeEventListener("beforeunload", onBefore);
  }, [dirty, saving]);

  useEffect(() => {
    if (!slug || !dirty) return;
    writeDraft(slug, id, values);
  }, [slug, id, dirty, values]);

  useEffect(() => {
    const sched = meta?.scheduling;
    const duration = sched?.duration_minutes;
    if (!sched || !duration) return;
    const timeName = sched.time;
    const endName = sched.end_time;
    if (!timeName || !endName) return;
    const start = String(values[timeName] ?? "");
    if (!start) return;
    const [h, m] = start.split(":").map(Number);
    if (Number.isNaN(h)) return;
    const total = h * 60 + (Number.isNaN(m) ? 0 : m) + duration;
    const next = `${String(Math.floor(total / 60) % 24).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
    if (values[endName]) return;
    setValues((prev) => ({ ...prev, [endName]: next }));
  }, [meta, values]);

  const blocker = useBlocker(({ nextLocation }) => {
    if (!dirty || saving) return false;
    if (nextLocation.search.includes("return=")) return false;
    return true;
  });

  if (!meta || !slug) return <ErrorState message="Unknown entity." />;
  if (loading) return <Skeleton rows={6} variant="form" />;
  const entitySlug = slug;
  const isSingleton = Boolean(meta.singleton);
  const errorEntries = Object.entries(fieldErrors);
  const cancelTo = returnTo || (id ? `/${entitySlug}/${id}` : `/${entitySlug}`);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (saving) return;
    const body: Record<string, unknown> = {};
    for (const field of fields) {
      if (field.readonly || field.computed) continue;
      if (fieldReadonly(field, values)) continue;
      const raw = values[field.name];
      if (raw === "" || raw == null) {
        if (!id && !field.required) continue;
        if (id) continue;
      }
      body[field.name] = coerce(field, raw);
    }
    try {
      setError("");
      setFieldErrors({});
      setSaving(true);
      setSaved(false);
      let createdId = id;
      if (id) {
        if (updatedAt) body._expected_updated_at = updatedAt;
        if (isSingleton) await api.saveSettings(entitySlug, body);
        else await api.update(entitySlug, id, body);
      } else if (isSingleton) {
        const savedRow = await api.saveSettings(entitySlug, body);
        createdId = String(savedRow.id ?? "");
      } else {
        const created = await api.create(entitySlug, body);
        createdId = String(created.id);
      }
      setDirty(false);
      clearDraft(entitySlug, id);
      setSaved(true);
      showSnackbar(id ? "Saved" : "Created");
      if (returnTo && createdId && returnField) {
        const dest = new URL(returnTo, window.location.origin);
        dest.searchParams.set(returnField, createdId);
        const origin = draftTargetFromPath(dest.pathname);
        if (origin) {
          const draft = readDraft(origin.slug, origin.id) ?? {};
          writeDraft(origin.slug, origin.id, { ...draft, [returnField]: createdId });
        }
        navigate(`${dest.pathname}${dest.search}${dest.hash}`);
        return;
      }
      if (id) {
        navigate(`/${entitySlug}/${id}`);
      } else {
        navigate(`/${entitySlug}/${createdId}`);
      }
    } catch (err) {
      if (err instanceof ApiError && err.status === 409 && /another user/i.test(err.message)) {
        setConflict(true);
        setError(err.message);
      } else if (err instanceof ApiError) {
        setError(friendlyError(err));
        const next: Record<string, string> = {};
        for (const fe of err.fields) next[fe.field] = fe.message;
        setFieldErrors(next);
        const first = err.fields[0]?.field;
        if (first) revealField(first);
      } else {
        setError("Unable to save.");
      }
    } finally {
      setSaving(false);
    }
  }

  const saveLabel = saving ? "Saving…" : saved ? "Saved" : id ? "Save" : "Create";

  return (
    <div className="page">
      <PageHeader
        kicker={meta.entity}
        title={
          <>
            {id ? "Edit" : "New"} {meta.label}
          </>
        }
      />
      <form className="form form-wide" onSubmit={onSubmit}>
        {errorEntries.length > 0 ? (
          <div className="form-error-summary" role="alert">
            <button type="button" className="ghost" onClick={() => revealField(errorEntries[0][0])}>
              {errorEntries.length} {errorEntries.length === 1 ? "error" : "errors"}
            </button>
            <ul>
              {errorEntries.map(([name, message]) => {
                const label = fields.find((f) => f.name === name)?.label || name;
                return (
                  <li key={name}>
                    <button type="button" className="ghost" onClick={() => revealField(name)}>
                      {label}: {message}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ) : null}
        <FormLayout
          fields={fields}
          values={values}
          entities={entities}
          fieldErrors={fieldErrors}
          layout={layout}
          focusField={focusField}
          focusSeq={focusSeq}
          onChange={(name, value) => {
            setDirty(true);
            setSaved(false);
            setValues((prev) => {
              const next = { ...prev, [name]: value };
              const children: Record<string, Array<Record<string, unknown>>> = {};
              for (const f of fields) {
                if (f.relation_kind === "child_table" || f.type === "child_table") {
                  children[f.name] = Array.isArray(next[f.name])
                    ? (next[f.name] as Array<Record<string, unknown>>)
                    : [];
                }
              }
              for (const f of fields) {
                if (f.computed && f.formula) {
                  const preview = previewFormula(f.formula, next, children);
                  if (preview != null) next[f.name] = preview;
                }
              }
              if (slug) writeDraft(slug, id, next);
              return next;
            });
          }}
        />
        {meta.scheduling ? (
          <AvailabilityPicker
            slug={entitySlug}
            meta={meta}
            values={values}
            onPick={(name, value) => {
              setDirty(true);
              setValues((prev) => ({ ...prev, [name]: value }));
            }}
          />
        ) : null}
        {error && <ErrorState message={error} />}
        <div className="form-actions actions">
          <Link to={cancelTo}>
            <button type="button" className="ghost">
              Cancel
            </button>
          </Link>
          <button type="submit" disabled={saving || saved}>
            {saveLabel}
          </button>
        </div>
      </form>
      <ConfirmDialog
        open={blocker.state === "blocked"}
        title="Unsaved changes"
        message="Your changes haven't been saved."
        cancelLabel="Stay"
        confirmLabel="Discard"
        danger
        onCancel={() => blocker.reset?.()}
        onConfirm={() => {
          if (slug) clearDraft(slug, id);
          blocker.proceed?.();
        }}
      />
      <ConfirmDialog
        open={conflict}
        title={t("conflict.title")}
        message={t("conflict.message")}
        cancelLabel={t("conflict.stay")}
        confirmLabel={t("conflict.reload")}
        onCancel={() => setConflict(false)}
        onConfirm={() => {
          void (async () => {
            if (!slug || !id) return;
            clearDraft(slug, id);
            try {
              const row = await api.get(slug, id);
              const next: Record<string, unknown> = {};
              for (const field of fields) {
                next[field.name] = row[field.name] ?? "";
              }
              setValues(next);
              setUpdatedAt(typeof row.updated_at === "string" ? row.updated_at : null);
              setDirty(false);
              setConflict(false);
              setError("");
              setSaved(false);
            } catch (err) {
              setError(friendlyError(err));
            }
          })();
        }}
      />
    </div>
  );
}

function coerce(field: UiField, raw: unknown): unknown {
  if (raw === "" || raw == null) return null;
  if (field.type === "child_table" || field.relation_kind === "child_table") return raw;
  if (field.computed) return raw;
  if (field.type === "integer" || field.type === "decimal") return Number(raw);
  if (field.type === "boolean") return Boolean(raw);
  return raw;
}

function AvailabilityPicker({
  slug,
  meta,
  values,
  onPick,
}: {
  slug: string;
  meta: UiEntity;
  values: Record<string, unknown>;
  onPick: (name: string, value: string) => void;
}) {
  const sched = meta.scheduling;
  const [slots, setSlots] = useState<Array<{ start: string; end: string; available: boolean }>>([]);
  const dateField = sched?.start;
  const timeField = sched?.time;
  const resourceField = sched?.resources?.[0];
  const date = dateField ? String(values[dateField] ?? "") : "";
  const resource = resourceField ? String(values[resourceField] ?? "") : "";

  useEffect(() => {
    if (!sched || !date) {
      setSlots([]);
      return;
    }
    const q = new URLSearchParams();
    q.set("date", date.slice(0, 10));
    if (resourceField && resource) q.set(resourceField, resource);
    if (sched.duration_minutes) q.set("duration", String(sched.duration_minutes));
    let cancelled = false;
    api
      .availability(slug, q)
      .then((result) => {
        if (!cancelled) setSlots(result.slots ?? []);
      })
      .catch(() => {
        if (!cancelled) setSlots([]);
      });
    return () => {
      cancelled = true;
    };
  }, [slug, date, resource, resourceField, sched]);

  if (!sched || !date || slots.length === 0) return null;
  return (
    <div className="card availability-slots" aria-label="Available times">
      <p className="muted">Available times</p>
      <div className="chips">
        {slots.map((slot) => (
          <button
            key={slot.start}
            type="button"
            className={`chip${slot.available ? "" : " is-unavailable"}`}
            disabled={!slot.available}
            onClick={() => {
              if (timeField) onPick(timeField, slot.start);
              if (sched.end_time) onPick(sched.end_time, slot.end);
            }}
          >
            {slot.start}
          </button>
        ))}
      </div>
    </div>
  );
}
