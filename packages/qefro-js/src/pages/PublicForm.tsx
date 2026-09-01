import { FormEvent, useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { api, ApiError, type UiEntity, type UiField } from "../api";
import { FormLayout } from "../components/forms/FormLayout";
import { TenantThemeContext } from "../metadata/context";

export default function PublicForm() {
  const { tenant, form } = useParams();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [success, setSuccess] = useState("");
  const [fields, setFields] = useState<UiField[]>([]);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [error, setError] = useState("");
  const [done, setDone] = useState<{ message: string; reference: unknown } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!tenant || !form) return;
    api
      .publicForm(tenant, form)
      .then((d) => {
        setTitle(d.title);
        setDescription(d.description ?? "");
        setSuccess(d.success_message ?? "Received");
        setFields(d.fields);
      })
      .catch((e) => setError(e.message));
  }, [tenant, form]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!tenant || !form) return;
    const body: Record<string, unknown> = {};
    for (const field of fields) {
      const raw = values[field.name];
      if (raw === "" || raw == null) continue;
      body[field.name] = raw;
    }
    try {
      setSaving(true);
      setError("");
      const result = await api.submitPublicForm(tenant, form, body);
      setDone({ message: result.message || success, reference: result.reference });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Unable to submit.");
    } finally {
      setSaving(false);
    }
  }

  const entity: UiEntity = {
    entity: "Public",
    label: title || "Form",
    label_plural: title || "Form",
    slug: form ?? "form",
    searchable: false,
    fields,
  };

  return (
    <TenantThemeContext.Provider value={{ timezone: "UTC", locale: "en-US", currency: "USD" }}>
      <div className="public-form">
        <h1>{title || "Form"}</h1>
        {description ? <p className="muted">{description}</p> : null}
        {done ? (
          <div className="panel">
            <h2>{done.message}</h2>
            {done.reference != null ? (
              <p>
                Reference: <strong>{String(done.reference)}</strong>
              </p>
            ) : null}
          </div>
        ) : (
          <form className="form form-wide" onSubmit={onSubmit}>
            <FormLayout
              fields={fields}
              values={values}
              entities={[entity]}
              fieldErrors={{}}
              onChange={(name, value) => setValues((prev) => ({ ...prev, [name]: value }))}
            />
            {error ? (
              <p className="error" role="alert">
                {error}
              </p>
            ) : null}
            <button type="submit" disabled={saving}>
              {saving ? "Sending…" : "Submit"}
            </button>
          </form>
        )}
      </div>
    </TenantThemeContext.Provider>
  );
}
