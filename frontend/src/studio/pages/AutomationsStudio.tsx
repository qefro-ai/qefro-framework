import { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload } from "../StudioApp";
import SourceView from "../components/SourceView";
import AutomationRuns from "../../components/automation/AutomationRuns";
import { validateAutomationGraph, type EditorStep } from "../editors/automationGraph";

const ACTION_KINDS = [
  "send_communication",
  "notify",
  "create_activity",
  "create_comment",
  "create_entity",
  "update_entity",
  "transition",
  "assign",
  "print_document",
  "send_webhook",
];

type TriggerDraft = {
  type: string;
  event: string;
  schedule: string;
};

function emptyAction(): EditorStep {
  return { kind: "action", actionKind: "notify", role: "Staff" };
}

export default function AutomationsStudio({ caps }: { caps: string[] }) {
  const { name } = useParams();
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [yaml, setYaml] = useState("");
  const [jsonText, setJsonText] = useState("");
  const [description, setDescription] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [trigger, setTrigger] = useState<TriggerDraft>({ type: "event", event: "", schedule: "" });
  const [steps, setSteps] = useState<EditorStep[]>([]);
  const [error, setError] = useState("");
  const [preview, setPreview] = useState<Record<string, unknown> | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);

  useEffect(() => {
    api.studioAutomations().then((d) => setItems(d.automations)).catch(() => setItems([]));
  }, []);

  useEffect(() => {
    if (!name) {
      setDetail(null);
      return;
    }
    api.studioAutomation(name).then((d) => {
      setDetail(d.automation);
      setYaml(d.yaml);
      setJsonText(d.json);
      const auto = d.automation;
      setDescription(String(auto.description ?? ""));
      setEnabled(auto.enabled !== false);
      const t = (auto.trigger ?? {}) as Record<string, unknown>;
      setTrigger({
        type: String(t.type || (t.schedule ? "scheduled" : "event")),
        event: String(t.event ?? ""),
        schedule: String(t.schedule ?? ""),
      });
      setSteps(studioStepsToEditor((auto.steps as Array<Record<string, unknown>>) ?? []));
    });
  }, [name]);

  const errors = useMemo(
    () => validateAutomationGraph({ trigger: trigger.event || trigger.schedule, steps }),
    [trigger, steps],
  );

  function payload() {
    return {
      name,
      enabled,
      description,
      trigger:
        trigger.type === "scheduled"
          ? { type: "scheduled", schedule: trigger.schedule }
          : { type: "event", event: trigger.event },
      steps: editorToDefSteps(steps),
    };
  }

  if (!name) {
    return (
      <div className="page">
        <h2>Automations</h2>
        <p className="muted">
          Event rules that invoke EntityService, Communication, and JobQueue. Not a second workflow engine.
        </p>
        {items.length === 0 ? <p className="empty">No automations.</p> : null}
        <ul className="studio-list">
          {items.map((item) => (
            <li key={String(item.id || item.name)}>
              <Link to={`/studio/automations/${String(item.name)}`}>
                {String(item.name).replaceAll("_", " ")}
              </Link>
              <span className="muted">
                {" "}
                {String(item.status ?? "")} · v{String(item.version ?? 1)}
              </span>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="page">
      <p>
        <Link to="/studio/automations">All automations</Link>
      </p>
      {error ? <p className="error">{error}</p> : null}
      <div className="studio-split auto-editor">
        <div>
          <h2>{String(name).replaceAll("_", " ")}</h2>
          <p className="muted">{enabled ? "Published" : "Disabled"} · developer editor for existing primitives</p>
          <label>
            Description
            <input value={description} onChange={(e) => setDescription(e.target.value)} />
          </label>
          <label>
            Trigger
            <select
              value={trigger.type}
              onChange={(e) => setTrigger({ ...trigger, type: e.target.value })}
            >
              <option value="event">event</option>
              <option value="scheduled">scheduled</option>
            </select>
          </label>
          {trigger.type === "scheduled" ? (
            <label>
              Cron
              <input
                value={trigger.schedule}
                onChange={(e) => setTrigger({ ...trigger, schedule: e.target.value })}
                placeholder="0 9 * * *"
              />
            </label>
          ) : (
            <label>
              Event
              <input
                value={trigger.event}
                onChange={(e) => setTrigger({ ...trigger, event: e.target.value })}
                placeholder="order.confirmed"
              />
            </label>
          )}
          {errors.length > 0 ? (
            <div className="card">
              <h3>Validation</h3>
              <ul>
                {errors.map((err) => (
                  <li key={err} className="error">
                    {err}
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <p className="muted">Graph is valid.</p>
          )}
          {can(caps, "studio.publish") ? (
            <div className="actions">
              <button
                type="button"
                disabled={errors.length > 0}
                onClick={() =>
                  publishAndReload({
                    kind: "automation",
                    target: name,
                    payload: payload(),
                    summary: `Publish automation ${name}`,
                  }).catch((e: Error) => setError(e.message))
                }
              >
                Publish
              </button>
              <button
                type="button"
                className="ghost"
                onClick={() =>
                  (enabled ? api.studioAutomationDisable(name) : api.studioAutomationEnable(name))
                    .then(() => setEnabled(!enabled))
                    .catch((e: Error) => setError(e.message))
                }
              >
                {enabled ? "Disable" : "Enable"}
              </button>
              <button
                type="button"
                className="ghost"
                onClick={() =>
                  api
                    .studioAutomationPreview(name, {
                      event: trigger.event,
                      payload: { status: "Preparing" },
                    })
                    .then(setPreview)
                    .catch((e: Error) => setError(e.message))
                }
              >
                Test
              </button>
            </div>
          ) : null}
        </div>
        <div>
          <div className="auto-flow">
            <article className="auto-node auto-node-trigger">
              <h3>Trigger</h3>
              <p>{trigger.event || trigger.schedule || "Missing trigger"}</p>
            </article>
            {steps.map((step, index) => (
              <StepCard
                key={index}
                step={step}
                onChange={(next) => setSteps(steps.map((s, i) => (i === index ? next : s)))}
                onDelete={() => setSteps(steps.filter((_, i) => i !== index))}
                draggable
                onDragStart={() => setDragIndex(index)}
                onDrop={() => {
                  if (dragIndex === null || dragIndex === index) return;
                  const copy = [...steps];
                  const [moved] = copy.splice(dragIndex, 1);
                  copy.splice(index, 0, moved);
                  setSteps(copy);
                  setDragIndex(null);
                }}
              />
            ))}
            <article className="auto-node auto-node-end">
              <h3>End</h3>
            </article>
          </div>
          <div className="actions">
            <button type="button" className="ghost" onClick={() => setSteps([...steps, emptyAction()])}>
              Add action
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => setSteps([...steps, { kind: "wait", wait: "30m" }])}
            >
              Add wait
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() =>
                setSteps([
                  ...steps,
                  { kind: "condition", field: "status", equals: "", then: [emptyAction()], else: [] },
                ])
              }
            >
              Add condition
            </button>
          </div>
          {preview ? (
            <div className="card">
              <h3>Dry run</h3>
              <p className="muted">No side effects. Communications are not sent.</p>
              <ol>
                {(preview.would_execute as Array<Record<string, unknown>> | undefined)?.map((step, i) => (
                  <li key={i}>{String(step.label || step.kind)}</li>
                ))}
              </ol>
            </div>
          ) : null}
          <h3>Runs</h3>
          <AutomationRuns automation={name} showEmpty />
          {detail ? <SourceView jsonText={jsonText} yamlText={yaml} /> : null}
        </div>
      </div>
    </div>
  );
}

function StepCard({
  step,
  onChange,
  onDelete,
  draggable,
  onDragStart,
  onDrop,
}: {
  step: EditorStep;
  onChange: (step: EditorStep) => void;
  onDelete: () => void;
  draggable?: boolean;
  onDragStart?: () => void;
  onDrop?: () => void;
}) {
  return (
    <>
      <div className="auto-edge" />
      <article
        className={`auto-node auto-node-${step.kind}`}
        draggable={draggable}
        onDragStart={onDragStart}
        onDragOver={(e) => e.preventDefault()}
        onDrop={onDrop}
      >
        {step.kind === "wait" ? (
          <>
            <h3>Wait</h3>
            <label>
              Duration
              <input value={step.wait} onChange={(e) => onChange({ ...step, wait: e.target.value })} />
            </label>
          </>
        ) : null}
        {step.kind === "condition" ? (
          <>
            <h3>Condition</h3>
            <label>
              Field
              <input value={step.field} onChange={(e) => onChange({ ...step, field: e.target.value })} />
            </label>
            <label>
              Equals
              <input value={step.equals} onChange={(e) => onChange({ ...step, equals: e.target.value })} />
            </label>
            <div className="auto-branch">
              <div>
                <p className="muted">Yes</p>
                {step.then.map((child, i) => (
                  <StepCard
                    key={i}
                    step={child}
                    onChange={(next) =>
                      onChange({ ...step, then: step.then.map((c, idx) => (idx === i ? next : c)) })
                    }
                    onDelete={() => onChange({ ...step, then: step.then.filter((_, idx) => idx !== i) })}
                  />
                ))}
                <button
                  type="button"
                  className="ghost"
                  onClick={() => onChange({ ...step, then: [...step.then, emptyAction()] })}
                >
                  Add
                </button>
              </div>
              <div>
                <p className="muted">No</p>
                {step.else.map((child, i) => (
                  <StepCard
                    key={i}
                    step={child}
                    onChange={(next) =>
                      onChange({ ...step, else: step.else.map((c, idx) => (idx === i ? next : c)) })
                    }
                    onDelete={() => onChange({ ...step, else: step.else.filter((_, idx) => idx !== i) })}
                  />
                ))}
                <button
                  type="button"
                  className="ghost"
                  onClick={() => onChange({ ...step, else: [...step.else, emptyAction()] })}
                >
                  Add
                </button>
              </div>
            </div>
          </>
        ) : null}
        {step.kind === "action" ? (
          <>
            <h3>Action</h3>
            <label>
              Kind
              <select
                value={step.actionKind}
                onChange={(e) => onChange({ ...step, actionKind: e.target.value })}
              >
                {ACTION_KINDS.map((kind) => (
                  <option key={kind} value={kind}>
                    {kind}
                  </option>
                ))}
              </select>
            </label>
            {step.actionKind === "send_communication" ? (
              <label>
                Template
                <input
                  value={step.template ?? ""}
                  onChange={(e) => onChange({ ...step, template: e.target.value })}
                />
              </label>
            ) : null}
            {step.actionKind === "notify" ? (
              <label>
                Role
                <input value={step.role ?? ""} onChange={(e) => onChange({ ...step, role: e.target.value })} />
              </label>
            ) : null}
            {step.actionKind === "transition" ? (
              <label>
                Transition
                <input
                  value={step.transition ?? ""}
                  onChange={(e) => onChange({ ...step, transition: e.target.value })}
                />
              </label>
            ) : null}
            {step.actionKind === "create_activity" || step.actionKind === "create_comment" ? (
              <label>
                Message
                <input
                  value={step.message ?? ""}
                  onChange={(e) => onChange({ ...step, message: e.target.value })}
                />
              </label>
            ) : null}
          </>
        ) : null}
        <button type="button" className="ghost" onClick={onDelete}>
          Delete
        </button>
      </article>
    </>
  );
}

function studioStepsToEditor(steps: Array<Record<string, unknown>>): EditorStep[] {
  return steps.map((step) => {
    const kind = String(step.kind ?? "");
    if (kind === "wait") {
      const wait = step.wait;
      if (typeof wait === "string") return { kind: "wait", wait };
      const until = (wait as { until_field?: string } | undefined)?.until_field;
      return { kind: "wait", wait: until ? `until:${until}` : "30m" };
    }
    if (kind === "condition") {
      const cond = (step.condition ?? {}) as Record<string, unknown>;
      return {
        kind: "condition",
        field: String(cond.field ?? ""),
        equals: String(cond.equals ?? ""),
        then: studioStepsToEditor((step.then as Array<Record<string, unknown>>) ?? []),
        else: studioStepsToEditor((step.else as Array<Record<string, unknown>>) ?? []),
      };
    }
    if (kind === "end") return { kind: "action", actionKind: "notify", role: "Staff" };
    const action = (step.action ?? {}) as Record<string, unknown>;
    const nested = Object.values(action)[0] as Record<string, unknown> | undefined;
    return {
      kind: "action",
      actionKind: kind || "notify",
      template: nested?.template as string | undefined,
      role: nested?.role as string | undefined,
      message: nested?.message as string | undefined,
      transition: nested?.name as string | undefined,
    };
  });
}

function editorToDefSteps(steps: EditorStep[]): unknown[] {
  return steps.map((step) => {
    if (step.kind === "wait") {
      if (step.wait.startsWith("until:")) {
        return { wait: { until_field: step.wait.slice(6) } };
      }
      return { wait: step.wait };
    }
    if (step.kind === "condition") {
      return {
        condition: { field: step.field, equals: step.equals },
        then: editorToDefSteps(step.then),
        else: editorToDefSteps(step.else),
      };
    }
    switch (step.actionKind) {
      case "send_communication":
        return { send_communication: { template: step.template } };
      case "notify":
        return { notify: { role: step.role } };
      case "create_activity":
        return { create_activity: { message: step.message } };
      case "create_comment":
        return { create_comment: { message: step.message } };
      case "transition":
        return { transition: { name: step.transition } };
      default:
        return { action: step.actionKind };
    }
  });
}
