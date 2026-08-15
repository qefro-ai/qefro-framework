import { FormEvent, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, ApiError, formVisible, type UiEntity, type UiField } from "../api";
import { FormLayout } from "../components/forms/FormLayout";

export default function EntityForm({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(Boolean(id));
  const [saving, setSaving] = useState(false);

  const fields = useMemo(
    () => (meta?.fields.filter(formVisible) ?? []).filter((f) => f.relation_kind !== "one_to_many"),
    [meta],
  );

  useEffect(() => {
    if (id && slug) {
      setLoading(true);
      api
        .get(slug, id)
        .then((row) => {
          const next: Record<string, unknown> = {};
          for (const field of fields) {
            next[field.name] = row[field.name] ?? "";
          }
          setValues(next);
        })
        .catch((e) => setError(e.message))
        .finally(() => setLoading(false));
    } else {
      setValues({});
      setLoading(false);
    }
  }, [id, slug, fields]);

  if (!meta || !slug) return <p>Unknown entity.</p>;
  if (loading) return <p className="muted">Loading {meta.label.toLowerCase()}…</p>;
  const entitySlug = slug;

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const body: Record<string, unknown> = {};
    for (const field of fields) {
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
        await api.update(entitySlug, id, body);
        navigate(`/${entitySlug}/${id}`);
      } else {
        const created = await api.create(entitySlug, body);
        navigate(`/${entitySlug}/${created.id}`);
      }
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
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
      <div className="badge">{meta.entity}</div>
      <h2>
        {id ? "Edit" : "New"} {meta.label}
      </h2>
      <form className="form form-wide" onSubmit={onSubmit}>
        <FormLayout
          fields={fields}
          values={values}
          entities={entities}
          fieldErrors={fieldErrors}
          onChange={(name, value) => setValues((prev) => ({ ...prev, [name]: value }))}
        />
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <button type="submit" disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </button>
      </form>
    </div>
  );
}

function coerce(field: UiField, raw: unknown): unknown {
  if (raw === "" || raw == null) return null;
  if (field.type === "integer" || field.type === "decimal") return Number(raw);
  if (field.type === "boolean") return Boolean(raw);
  return raw;
}
