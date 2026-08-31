import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../../api";

export default function Platform({
  kind,
}: {
  kind: "notifications" | "webhooks" | "public-forms";
}) {
  if (kind === "notifications") return <NotificationsPage />;
  if (kind === "webhooks") return <WebhooksPage />;
  return <PublicFormsPage />;
}

function NotificationsPage() {
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    api.studioNotifications().then((d) => setItems(d.notifications)).catch(() => setItems([]));
  }, []);
  return (
    <div className="page">
      <h2>Notifications</h2>
      <p className="muted">Rules fire after COMMIT. Secrets are never shown.</p>
      {items.length === 0 ? <p className="empty">No notification rules.</p> : null}
      {items.map((n) => (
        <div key={String(n.name)} className="card">
          <h3>{String(n.name)}</h3>
          <p>Event: {String(n.event ?? "")}</p>
          <p>Channels: {Array.isArray(n.channels) ? n.channels.join(", ") : ""}</p>
          <p>Recipients: {Array.isArray(n.recipients) ? n.recipients.join(", ") : ""}</p>
        </div>
      ))}
    </div>
  );
}

function WebhooksPage() {
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [deliveries, setDeliveries] = useState<Array<Record<string, unknown>>>([]);
  const [test, setTest] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.studioWebhooks().then((d) => setItems(d.webhooks)).catch(() => setItems([]));
  }, []);

  useEffect(() => {
    if (!selected) return;
    api.webhookDeliveries(selected).then((d) => setDeliveries(d.deliveries)).catch(() => setDeliveries([]));
  }, [selected]);

  return (
    <div className="page">
      <h2>Webhooks</h2>
      <p className="muted">Deliveries run after COMMIT. HMAC secrets are not exposed.</p>
      {error ? <p className="error">{error}</p> : null}
      {items.length === 0 ? <p className="empty">No webhooks.</p> : null}
      {items.map((w) => (
        <div key={String(w.name)} className="card">
          <h3>{String(w.name)}</h3>
          <p>Event: {String(w.event)}</p>
          <p>URL: {String(w.target)}</p>
          <p>Status: {w.enabled === false ? "Disabled" : "Enabled"}</p>
          <div className="actions">
            <button type="button" className="ghost" onClick={() => setSelected(String(w.name))}>
              Deliveries
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() =>
                api
                  .testWebhook(String(w.name))
                  .then(setTest)
                  .catch((e) => setError(e.message))
              }
            >
              Test delivery
            </button>
          </div>
        </div>
      ))}
      {selected ? (
        <div className="panel">
          <h3>Deliveries · {selected}</h3>
          <ul>
            {deliveries.map((d, i) => (
              <li key={String(d.id ?? i)}>
                {String(d.event ?? "")} · {d.ok ? "✓" : "✗"} {String(d.status_code ?? "")}{" "}
                {d.last_error ? `· ${d.last_error}` : ""}
              </li>
            ))}
            {deliveries.length === 0 ? <li className="muted">No deliveries yet.</li> : null}
          </ul>
        </div>
      ) : null}
      {test ? (
        <div className="card">
          <h4>Test payload</h4>
          <pre>{JSON.stringify(test.body ?? test, null, 2)}</pre>
        </div>
      ) : null}
    </div>
  );
}

function PublicFormsPage() {
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    api.studioPublicForms().then((d) => setItems(d.public_forms)).catch(() => setItems([]));
  }, []);
  return (
    <div className="page">
      <h2>Public Forms</h2>
      <p className="muted">Only explicitly listed fields are public. Tenant is resolved from the signed route, never from the body.</p>
      {items.length === 0 ? <p className="empty">No public forms.</p> : null}
      {items.map((f) => (
        <div key={String(f.slug)} className="card">
          <h3>
            <Link to={`/studio/entities/${f.entity}`}>{String(f.title || f.slug)}</Link>
          </h3>
          <p>Slug: {String(f.slug)}</p>
          <p>Entity: {String(f.entity)}</p>
          <p>Enabled: {f.enabled === false ? "No" : "Yes"}</p>
          <p>Fields: {Array.isArray(f.fields) ? f.fields.join(", ") : ""}</p>
        </div>
      ))}
    </div>
  );
}
