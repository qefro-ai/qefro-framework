import { useState } from "react";
import { api } from "../../sdk/client";
import { publishAndReload } from "../StudioApp";
import type { UiEntity } from "../../metadata/types";

export default function SchedulingEditor({
  entity,
  def,
  ui,
  canPublish,
  onSaved,
}: {
  entity: string;
  def?: Record<string, unknown>;
  ui?: UiEntity;
  canPublish: boolean;
  onSaved: () => Promise<void>;
}) {
  const current = (def?.scheduling as Record<string, unknown> | undefined) ?? {};
  const fields = ui?.fields ?? [];
  const dateFields = fields.filter((f) => f.type === "date" || f.type === "datetime");
  const timeFields = fields.filter((f) => f.type === "time");
  const relations = fields.filter((f) => f.relation);
  const [enabled, setEnabled] = useState(Boolean(def?.scheduling));
  const [start, setStart] = useState(String(current.start_field ?? ui?.scheduling?.start ?? dateFields[0]?.name ?? ""));
  const [end, setEnd] = useState(String(current.end_field ?? ui?.scheduling?.end ?? ""));
  const [time, setTime] = useState(String(current.time_field ?? ui?.scheduling?.time ?? ""));
  const [endTime, setEndTime] = useState(String(current.end_time_field ?? ui?.scheduling?.end_time ?? ""));
  const [resource, setResource] = useState(String((current.resources as string[] | undefined)?.[0] ?? ui?.scheduling?.resources?.[0] ?? ""));
  const [conflict, setConflict] = useState(Boolean(current.conflict ?? ui?.scheduling?.conflict ?? true));
  const [calendar, setCalendar] = useState(Boolean(current.calendar ?? ui?.scheduling?.calendar ?? true));
  const [duration, setDuration] = useState(String(current.duration_minutes ?? ui?.scheduling?.duration_minutes ?? 60));
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);

  function payload() {
    if (!enabled) return { enabled: false };
    return {
      start_field: start,
      end_field: end || null,
      time_field: time || null,
      end_time_field: endTime || null,
      resources: resource ? [resource] : [],
      conflict,
      calendar,
      duration_minutes: Number(duration) || 60,
    };
  }

  async function validate() {
    setError("");
    const result = await api.studioValidate({
      kind: "entity.scheduling",
      target: entity,
      payload: payload(),
    });
    setPreview(result);
  }

  async function publish() {
    if (!canPublish) return;
    await validate();
    await publishAndReload({
      kind: "entity.scheduling",
      target: entity,
      payload: payload(),
      summary: `Scheduling on ${entity}`,
    });
    await onSaved();
  }

  return (
    <div className="card">
      <h3>Scheduling</h3>
      <p className="muted">Opt this entity into the generic calendar and conflict checks. Fields must already exist.</p>
      <label>
        <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} /> Enabled
      </label>
      {enabled ? (
        <div className="form-grid">
          <label>
            Start field
            <select value={start} onChange={(e) => setStart(e.target.value)}>
              {dateFields.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            End field
            <select value={end} onChange={(e) => setEnd(e.target.value)}>
              <option value="">(none)</option>
              {dateFields.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Time field
            <select value={time} onChange={(e) => setTime(e.target.value)}>
              <option value="">(none)</option>
              {timeFields.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            End time
            <select value={endTime} onChange={(e) => setEndTime(e.target.value)}>
              <option value="">(none)</option>
              {timeFields.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Resource
            <select value={resource} onChange={(e) => setResource(e.target.value)}>
              <option value="">(none)</option>
              {relations.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Duration (minutes)
            <input value={duration} onChange={(e) => setDuration(e.target.value)} />
          </label>
          <label>
            <input type="checkbox" checked={conflict} onChange={(e) => setConflict(e.target.checked)} /> Conflict checking
          </label>
          <label>
            <input type="checkbox" checked={calendar} onChange={(e) => setCalendar(e.target.checked)} /> Calendar
          </label>
        </div>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
      {preview ? <p className="muted" role="status">{String(preview.ok ? "Valid" : JSON.stringify(preview))}</p> : null}
      <div className="actions">
        <button type="button" className="ghost" onClick={() => validate().catch((e) => setError(e.message))}>
          Validate
        </button>
        {canPublish ? (
          <button type="button" onClick={() => publish().catch((e) => setError(e.message))}>
            Publish
          </button>
        ) : null}
      </div>
    </div>
  );
}
