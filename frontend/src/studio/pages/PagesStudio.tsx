import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload } from "../StudioApp";
import SourceView from "../components/SourceView";

const LAYOUTS = ["stack", "two_column", "three_column", "grid", "split"];
const KINDS = ["entity_view", "related", "report", "widget", "activity", "attachments"];
const VIEWS = ["list", "card", "kanban", "calendar", "chart"];
const PANES = ["", "master", "detail", "main"];

type SectionDraft = {
  title: string;
  kind: string;
  entity?: string;
  view?: string;
  report?: string;
  dashboard?: string;
  widget?: string;
  relation?: string;
  query?: string;
  size?: string;
  pane?: string;
  tab?: string;
};

export default function PagesStudio({ caps }: { caps: string[] }) {
  const { name } = useParams();
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [sections, setSections] = useState<SectionDraft[]>([]);
  const [layout, setLayout] = useState("stack");
  const [error, setError] = useState("");
  const [entities, setEntities] = useState<Array<{ name: string }>>([]);
  const [reports, setReports] = useState<Array<{ name: string }>>([]);
  const [dashboards, setDashboards] = useState<Array<{ name: string }>>([]);

  useEffect(() => {
    api.studioPages().then((d) => setItems(d.pages));
    api.studioEntities().then((d) => setEntities(d.entities as Array<{ name: string }>)).catch(() => undefined);
    api.studioReports().then((d) => setReports(d.reports as Array<{ name: string }>)).catch(() => undefined);
    api.studioDashboards().then((d) => setDashboards(d.dashboards as Array<{ name: string }>)).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!name) {
      setDetail(null);
      setSections([]);
      return;
    }
    api.studioPage(name).then((d) => {
      setDetail(d);
      const payload = (d.page ?? d) as Record<string, unknown>;
      setLayout(String(payload.layout || "stack"));
      setSections(((payload.sections as SectionDraft[]) ?? []).map((s) => ({ ...s })));
    });
  }, [name]);

  if (!name) {
    return (
      <div className="page">
        <h2>Pages</h2>
        <p className="muted">Compose workspaces from existing entities, views, reports, and widgets. Custom JavaScript, HTML, and SQL are rejected.</p>
        <ul>
          {items.map((item) => (
            <li key={String(item.name)}>
              <Link to={`/studio/pages/${item.name}`}>{String(item.label || item.name)}</Link>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  const payload = {
    ...((detail?.page as Record<string, unknown>) ?? detail ?? {}),
    layout,
    sections,
  };

  return (
    <div className="page">
      <p>
        <Link to="/studio/pages">Pages</Link>
      </p>
      <h2>{String(payload.label || payload.name || name)}</h2>
      <label>
        Layout
        <select value={layout} onChange={(e) => setLayout(e.target.value)} disabled={!can(caps, "studio.publish")}>
          {LAYOUTS.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </label>
      <SectionEditor
        sections={sections}
        onChange={setSections}
        canEdit={can(caps, "studio.publish")}
        entities={entities}
        reports={reports}
        dashboards={dashboards}
      />
      <SourceView jsonText={String(detail?.json ?? "")} yamlText={String(detail?.yaml ?? "")} />
      {can(caps, "studio.publish") ? (
        <button
          type="button"
          onClick={() =>
            publishAndReload({ kind: "page", target: name, payload }).catch((e) => setError(e.message))
          }
        >
          Publish
        </button>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}

function SectionEditor({
  sections,
  onChange,
  canEdit,
  entities,
  reports,
  dashboards,
}: {
  sections: SectionDraft[];
  onChange: (sections: SectionDraft[]) => void;
  canEdit: boolean;
  entities: Array<{ name: string }>;
  reports: Array<{ name: string }>;
  dashboards: Array<{ name: string }>;
}) {
  function update(i: number, patch: Partial<SectionDraft>) {
    const next = sections.slice();
    next[i] = { ...next[i], ...patch };
    onChange(next);
  }
  return (
    <div className="card">
      <h3>Components</h3>
      {sections.map((section, i) => (
        <div key={i} className="form-grid">
          <label>
            Title
            <input
              value={section.title}
              disabled={!canEdit}
              onChange={(e) => update(i, { title: e.target.value })}
            />
          </label>
          <label>
            Kind
            <select
              value={section.kind}
              disabled={!canEdit}
              onChange={(e) => update(i, { kind: e.target.value })}
            >
              {KINDS.map((kind) => (
                <option key={kind} value={kind}>
                  {kind}
                </option>
              ))}
            </select>
          </label>
          <label>
            Entity
            <select
              value={section.entity || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { entity: e.target.value })}
            >
              <option value="">—</option>
              {entities.map((entity) => (
                <option key={entity.name} value={entity.name}>
                  {entity.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            View
            <select
              value={section.view || "list"}
              disabled={!canEdit}
              onChange={(e) => update(i, { view: e.target.value })}
            >
              {VIEWS.map((view) => (
                <option key={view} value={view}>
                  {view}
                </option>
              ))}
            </select>
          </label>
          <label>
            Report
            <select
              value={section.report || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { report: e.target.value })}
            >
              <option value="">—</option>
              {reports.map((report) => (
                <option key={report.name} value={report.name}>
                  {report.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Dashboard widget
            <select
              value={section.dashboard || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { dashboard: e.target.value })}
            >
              <option value="">—</option>
              {dashboards.map((dash) => (
                <option key={dash.name} value={dash.name}>
                  {dash.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Pane
            <select
              value={section.pane || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { pane: e.target.value })}
            >
              {PANES.map((pane) => (
                <option key={pane || "none"} value={pane}>
                  {pane || "—"}
                </option>
              ))}
            </select>
          </label>
        </div>
      ))}
      {canEdit ? (
        <button
          type="button"
          className="ghost"
          onClick={() => onChange([...sections, { title: "New section", kind: "entity_view", view: "list" }])}
        >
          Add section
        </button>
      ) : null}
    </div>
  );
}
