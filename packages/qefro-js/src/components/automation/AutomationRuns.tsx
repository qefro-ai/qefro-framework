import { useEffect, useState } from "react";
import { api } from "../../api";

function statusMark(status: string) {
  if (status === "completed" || status === "succeeded") return "✓";
  if (status === "waiting" || status === "pending" || status === "retrying" || status === "running") {
    return "↻";
  }
  if (status === "failed" || status === "cancelled") return "✕";
  return "·";
}

export default function AutomationRuns({
  automation,
  entity,
  recordId,
  showEmpty = false,
}: {
  automation?: string;
  entity?: string;
  recordId?: string;
  showEmpty?: boolean;
}) {
  const [runs, setRuns] = useState<Array<Record<string, unknown>>>([]);

  useEffect(() => {
    api
      .studioAutomationRuns(automation, { entity, recordId })
      .then((d) => setRuns(d.runs ?? []))
      .catch(() => setRuns([]));
  }, [automation, entity, recordId]);

  if (runs.length === 0) {
    return showEmpty ? <p className="empty">No automation runs.</p> : null;
  }

  return (
    <ul className="auto-runs">
      {runs.map((run) => {
        const status = String(run.status ?? "");
        const steps = Array.isArray(run.steps) ? (run.steps as Array<Record<string, unknown>>) : [];
        return (
          <li key={String(run.execution_id)} className="card">
            <h4>
              {statusMark(status)} {String(run.automation_id ?? "Automation")}
            </h4>
            <p className="muted">
              {status}
              {run.entity ? ` · ${String(run.entity)}` : ""}
              {run.error ? ` · ${String(run.error)}` : ""}
            </p>
            {steps.length > 0 ? (
              <ol className="auto-run-steps">
                {steps.map((step, i) => (
                  <li key={i}>
                    {statusMark(String(step.status ?? ""))} {String(step.message || step.kind || "step")}
                  </li>
                ))}
              </ol>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}
