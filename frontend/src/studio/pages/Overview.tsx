import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../../api";

export default function Overview() {
  const [data, setData] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.studioOverview().then(setData).catch((e) => setError(e.message));
  }, []);

  if (error) return <p className="error">{error}</p>;
  if (!data) return <p className="muted">Loading Studio…</p>;

  const apps = (data.apps as Array<Record<string, unknown>>) ?? [];
  const warnings = (data.warnings as Array<Record<string, unknown>>) ?? [];
  const recent = (data.recent_changes as Array<Record<string, unknown>>) ?? [];

  return (
    <div className="page">
      <h2>Qefro Studio</h2>
      <div className="cards">
        <Stat label="Installed Apps" value={data.installed_apps} to="/studio/apps" />
        <Stat label="Entities" value={data.entities} to="/studio/entities" />
        <Stat label="Workflows" value={data.workflows} to="/studio/workflows" />
        <Stat label="Reports" value={data.reports} to="/studio/reports" />
        <Stat label="Dashboards" value={data.dashboards} to="/studio/dashboards" />
      </div>
      {warnings.length > 0 && (
        <section className="card">
          <h3>Warnings</h3>
          <ul>
            {warnings.map((w, i) => (
              <li key={i}>
                {String(w.kind).replaceAll("_", " ")}
                {w.app ? `: ${String(w.app)}` : ""}
              </li>
            ))}
          </ul>
        </section>
      )}
      <section className="card">
        <h3>Applications</h3>
        <table>
          <tbody>
            {apps.map((app) => (
              <tr key={String(app.name)}>
                <td>
                  <Link to={`/studio/apps/${app.name}`}>{String(app.label || app.name)}</Link>
                </td>
                <td>{String(app.version)}</td>
                <td>{app.disabled ? "Disabled" : "Installed"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
      {recent.length > 0 && (
        <section className="card">
          <h3>Recent Changes</h3>
          <ul>
            {recent.map((row, i) => (
              <li key={i}>
                {String(row.action)} · {String(row.created_at)}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function Stat({ label, value, to }: { label: string; value: unknown; to: string }) {
  return (
    <Link className="card" to={to}>
      <p className="muted">{label}</p>
      <h3>{String(value ?? 0)}</h3>
    </Link>
  );
}
