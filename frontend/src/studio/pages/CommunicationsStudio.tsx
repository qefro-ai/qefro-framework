import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload } from "../StudioApp";
import SourceView from "../components/SourceView";

const CHANNELS = ["in_app", "email", "sms", "whatsapp"];
const PURPOSES = ["transactional", "marketing"];

export default function CommunicationsStudio({ caps }: { caps: string[] }) {
  const { name } = useParams();
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [preview, setPreview] = useState<{ subject?: string; body?: string; sent?: boolean } | null>(
    null,
  );
  const [channel, setChannel] = useState("email");
  const [purpose, setPurpose] = useState("transactional");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api.studioCommunications().then((d) => setItems(d.communications)).catch(() => setItems([]));
  }, []);

  useEffect(() => {
    if (!name) {
      setDetail(null);
      setPreview(null);
      return;
    }
    api.studioCommunication(name).then((d) => {
      setDetail(d);
      const payload = (d.communication ?? d) as Record<string, unknown>;
      const channels = (payload.channels as string[] | undefined) ?? [];
      setChannel(channels[0] || "email");
      setPurpose(String(payload.purpose || "transactional"));
      setSubject(String(payload.subject || ""));
      setBody(String(payload.body || ""));
    });
    api
      .studioCommunicationPreview(name)
      .then(setPreview)
      .catch(() => setPreview(null));
  }, [name]);

  const payload = (detail?.communication ?? detail ?? {}) as Record<string, unknown>;

  return (
    <div className="page">
      <h2>Templates</h2>
      <p className="muted">Declarative messages. Preview never sends a real message.</p>
      {error ? <p className="error">{error}</p> : null}
      {!name ? (
        <ul className="stack">
          {items.length === 0 ? <p className="empty">No communication templates.</p> : null}
          {items.map((item) => (
            <li key={String(item.name)}>
              <Link to={`/studio/communications/${String(item.name)}`}>
                {String(item.name).replaceAll("_", " ")}
              </Link>
              <span className="muted">
                {" "}
                {String(item.entity ?? "")} · {String(item.event || "manual")}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <>
          <p>
            <Link to="/studio/communications">All templates</Link>
          </p>
          <div className="card">
            <h3>{String(payload.name ?? name).replaceAll("_", " ")}</h3>
            <p>Entity: {String(payload.entity ?? "")}</p>
            <p>Event: {String(payload.event || "—")}</p>
            <label>
              Channel
              <select value={channel} onChange={(e) => setChannel(e.target.value)}>
                {CHANNELS.map((ch) => (
                  <option key={ch} value={ch}>
                    {ch}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Purpose
              <select value={purpose} onChange={(e) => setPurpose(e.target.value)}>
                {PURPOSES.map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Subject
              <input value={subject} onChange={(e) => setSubject(e.target.value)} />
            </label>
            <label>
              Body
              <textarea rows={8} value={body} onChange={(e) => setBody(e.target.value)} />
            </label>
            <p className="muted">Variables: {`{{ customer.name }} {{ order.number }} {{ order.total }}`}</p>
            {can(caps, "studio.publish") ? (
              <button
                type="button"
                onClick={() =>
                  publishAndReload({
                    kind: "communication",
                    target: name,
                    payload: {
                      ...payload,
                      channels: [channel],
                      purpose,
                      subject,
                      body,
                    },
                    summary: `Update communication ${name}`,
                  }).catch((e: Error) => setError(e.message))
                }
              >
                Publish
              </button>
            ) : null}
          </div>
          {preview ? (
            <div className="card">
              <h3>Template Preview</h3>
              <p>
                <strong>{preview.subject}</strong>
              </p>
              <pre className="source">{preview.body}</pre>
              {preview.sent ? <p className="error">Preview must not send.</p> : null}
            </div>
          ) : null}
          {detail ? <SourceView jsonText={String(detail.json ?? "")} yamlText={String(detail.yaml ?? "")} /> : null}
        </>
      )}
    </div>
  );
}
