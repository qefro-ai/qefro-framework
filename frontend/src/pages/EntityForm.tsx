import { FormEvent, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, ApiError, formVisible, type UiEntity, type UiField } from "../api";
import { renderWidget } from "../widgets";

export default function EntityForm({ entities }: { entities: UiEntity[] }) {
  const { slug, id } = useParams();
  const meta = entities.find((e) => e.slug === slug);
  const navigate = useNavigate();
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const fields = useMemo(
    () => (meta?.fields.filter(formVisible) ?? []).filter((f) => f.relation_kind !== "one_to_many"),
    [meta],
  );

  useEffect(() => {
    if (id && slug) {
      api.get(slug, id).then((row) => {
        const next: Record<string, unknown> = {};
        for (const field of fields) {
          next[field.name] = row[field.name] ?? "";
        }
        setValues(next);
      });
    } else {
      setValues({});
    }
  }, [id, slug, fields]);

  if (!meta || !slug) return <p>Unknown entity.</p>;
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
        setError("failed");
      }
    }
  }

  const sections = groupBySection(fields);

  return (
    <div>
      <h2>
        {id ? "Edit" : "New"} {meta.label}
      </h2>
      <form className="form" onSubmit={onSubmit}>
        {sections.map(([section, sectionFields]) => (
          <fieldset key={section || "default"}>
            {section ? <legend>{section}</legend> : null}
            {sectionFields.map((field) => (
              <label key={field.name}>
                {field.label}
                {field.required ? " *" : ""}
                {renderWidget({
                  field,
                  value: values[field.name],
                  entities,
                  onChange: (value) => setValues({ ...values, [field.name]: value }),
                })}
                {field.description && <span className="muted">{field.description}</span>}
                {fieldErrors[field.name] && <span className="error">{fieldErrors[field.name]}</span>}
              </label>
            ))}
          </fieldset>
        ))}
        {error && <p className="error">{error}</p>}
        <button type="submit">Save</button>
      </form>
    </div>
  );
}

function groupBySection(fields: UiField[]): Array<[string, UiField[]]> {
  const map = new Map<string, UiField[]>();
  for (const field of fields) {
    const key = field.section ?? "";
    const list = map.get(key) ?? [];
    list.push(field);
    map.set(key, list);
  }
  return Array.from(map.entries());
}

function coerce(field: UiField, raw: unknown): unknown {
  if (raw === "" || raw == null) return null;
  if (field.type === "integer" || field.type === "decimal") return Number(raw);
  if (field.type === "boolean") return Boolean(raw);
  return raw;
}
