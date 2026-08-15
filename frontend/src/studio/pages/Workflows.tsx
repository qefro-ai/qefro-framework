import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload, useStudioEntities } from "../StudioApp";
import SourceView from "../components/SourceView";

type Transition = {
  name: string;
  from: string;
  to: string;
  label?: string;
  allowed_roles?: string[];
};

type Workflow = {
  name: string;
  entity: string;
  initial: string;
  field?: string;
  states: Array<{ name: string; label?: string; terminal?: boolean }>;
  transitions: Transition[];
};

export default function Workflows({ caps }: { caps: string[] }) {
  const { entity } = useParams();
  const entities = useStudioEntities();
  const [data, setData] = useState<{ workflow?: Workflow; json?: string; yaml?: string; warnings?: string[] } | null>(
    null,
  );
  const [wf, setWf] = useState<Workflow | null>(null);
  const [error, setError] = useState("");
  const [diff, setDiff] = useState<string[]>([]);

  useEffect(() => {
    if (!entity) return;
    api
      .studioWorkflow(entity)
      .then((d) => {
        setData(d as { workflow?: Workflow; json?: string; yaml?: string; warnings?: string[] });
        setWf(d.workflow as Workflow);
      })
      .catch((e) => setError(e.message));
  }, [entity]);

  if (!entity) {
    return (
      <div className="page">
        <h2>Workflows</h2>
        <ul>
          {entities
            .filter((e) => e.workflow)
            .map((e) => (
              <li key={String(e.name)}>
                <Link to={`/studio/workflows/${e.name}`}>{String(e.label || e.name)}</Link>
              </li>
            ))}
        </ul>
      </div>
    );
  }

  if (!wf) return <p className={error ? "error" : "muted"}>{error || "Loading workflow…"}</p>;

  return (
    <div className="page">
      <p>
        <Link to={`/studio/entities/${entity}`}>Entity</Link>
      </p>
      <h2>{wf.entity} workflow</h2>
      <div className="workflow-graph">
        {wf.states.map((state) => (
          <div key={state.name} className="card">
            <strong>{state.label || state.name}</strong>
            {state.name === wf.initial ? <span className="muted"> initial</span> : null}
            <ul>
              {wf.transitions
                .filter((t) => t.from === state.name)
                .map((t) => (
                  <li key={t.name}>
                    {t.label || t.name} → {t.to}
                    {t.allowed_roles?.length ? ` (${t.allowed_roles.join(", ")})` : ""}
                  </li>
                ))}
            </ul>
          </div>
        ))}
      </div>
      {can(caps, "studio.manage_workflows") ? (
        <div className="actions">
          <button
            type="button"
            className="ghost"
            onClick={() =>
              setWf({
                ...wf,
                states: wf.states.some((s) => s.name === "Approved")
                  ? wf.states
                  : [...wf.states, { name: "Approved", label: "Approved" }],
                transitions: [
                  ...wf.transitions,
                  {
                    name: "approve",
                    from: wf.states[0]?.name === "Draft" ? "Confirmed" : wf.initial,
                    to: "Approved",
                    label: "Approve",
                    allowed_roles: ["Manager"],
                  },
                ],
              })
            }
          >
            Add Approved state
          </button>
          <button
            type="button"
            onClick={async () => {
              try {
                const preview = await api.studioValidate({
                  kind: "workflow",
                  target: entity,
                  payload: wf,
                });
                setDiff(preview.diff);
                await publishAndReload({ kind: "workflow", target: entity, payload: wf });
              } catch (e) {
                setError((e as Error).message);
              }
            }}
          >
            Validate & Publish
          </button>
        </div>
      ) : null}
      {diff.length > 0 ? <pre>{diff.join("\n")}</pre> : null}
      {error ? <p className="error">{error}</p> : null}
      <SourceView jsonText={data?.json ?? ""} yamlText={data?.yaml ?? ""} />
    </div>
  );
}
