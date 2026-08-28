import { FormEvent, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { api, ApiError, formVisible, type UiEntity, type UiField } from "../sdk/client";
import { FormLayout } from "../components/forms/FormLayout";
import { ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { friendlyError } from "../friendlyError";
import { previewFormula } from "../metadata/formula";
import { displayValue } from "../metadata/views";
import { useBreadcrumbRecord } from "../components/shell/breadcrumbContext";

export default function EntityForm({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const [searchParams] = useSearchParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(Boolean(id));
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const { setRecord } = useBreadcrumbRecord();

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

  useEffect(() => {
    if (id && slug) {
      setLoading(true);
      api
        .get(slug, id)
        .then((row) => {
          const next: Record<string, unknown> = {};
          for (const field of baseFields) {
            next[field.name] = row[field.name] ?? "";
          }
          setValues(next);
          setDirty(false);
          if (meta) setRecord({ id, label: displayValue(row, meta.display_field) || id });
        })
        .catch((e) => setError(friendlyError(e)))
        .finally(() => setLoading(false));
    } else {
      const next: Record<string, unknown> = {};
      for (const field of baseFields) {
        const q = searchParams.get(field.name);
        if (q) next[field.name] = q;
      }
      setValues(next);
      setDirty(false);
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

  if (!meta || !slug) return <ErrorState message="Unknown entity." />;
  if (loading) return <Skeleton rows={6} variant="form" />;
  const entitySlug = slug;
  const isSingleton = Boolean(meta.singleton);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const body: Record<string, unknown> = {};
    for (const field of fields) {
      if (field.readonly || field.computed) continue;
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
      if (id) {
        if (isSingleton) await api.saveSettings(entitySlug, body);
        else await api.update(entitySlug, id, body);
        setDirty(false);
        navigate(`/${entitySlug}/${id}`);
      } else if (isSingleton) {
        const saved = await api.saveSettings(entitySlug, body);
        setDirty(false);
        navigate(`/${entitySlug}/${saved.id ?? ""}`);
      } else {
        const created = await api.create(entitySlug, body);
        setDirty(false);
        navigate(`/${entitySlug}/${created.id}`);
      }
    } catch (err) {
      if (err instanceof ApiError) {
        setError(friendlyError(err));
        const next: Record<string, string> = {};
        for (const fe of err.fields) next[fe.field] = fe.message;
        setFieldErrors(next);
      } else {
        setError("Unable to save.");
      }
    } finally {
      setSaving(false);
    }
  }

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
        <FormLayout
          fields={fields}
          values={values}
          entities={entities}
          fieldErrors={fieldErrors}
          sectionRules={meta.views?.form?.sections}
          onChange={(name, value) => {
            setDirty(true);
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
              return next;
            });
          }}
        />
        {error && <ErrorState message={error} />}
        <div className="form-actions actions">
          <Link to={id ? `/${entitySlug}/${id}` : `/${entitySlug}`}>
            <button type="button" className="ghost">
              Cancel
            </button>
          </Link>
          <button type="submit" disabled={saving}>
            {saving ? "Saving…" : id ? "Save" : "Create"}
          </button>
        </div>
      </form>
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
