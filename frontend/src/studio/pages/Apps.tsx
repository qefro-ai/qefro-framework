import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";

export default function Apps() {
  const { app } = useParams();
  const [apps, setApps] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);

  useEffect(() => {
    api.studioApps().then((d) => setApps(d.apps));
  }, []);

  useEffect(() => {
    if (app) api.studioApp(app).then(setDetail);
    else setDetail(null);
  }, [app]);

  if (app && detail) {
    const entities = (detail.entities as Array<Record<string, unknown>>) ?? [];
    const deps = (detail.dependencies as Record<string, string>) ?? {};
    const reverse = (detail.reverse_dependencies as string[]) ?? [];
    return (
      <div className="page">
        <p>
          <Link to="/studio/apps">Apps</Link> / {String(detail.label || detail.name)}
        </p>
        <h2>{String(detail.label || detail.name)}</h2>
        {detail.source_managed ? (
          <p className="muted">Rust application — Studio inspects metadata and can overlay safe runtime changes. Operation handlers stay source-controlled.</p>
        ) : (
          <p className="muted">YAML application — Studio can write validated entity files under entities/.</p>
        )}
        <dl className="meta-list">
          <dt>Installed version</dt>
          <dd>{String(detail.installed_version)}</dd>
          <dt>Framework</dt>
          <dd>
            {String(detail.framework_version || "*")} (runtime {String(detail.framework_runtime)})
          </dd>
          <dt>Status</dt>
          <dd>{String(detail.status)}</dd>
          <dt>Tenant enablement</dt>
          <dd>{detail.enabled_for_tenant ? "Enabled" : "Not in tenant entitlement set"}</dd>
        </dl>
        <h3>Dependencies</h3>
        <ul>
          {Object.entries(deps).map(([name, req]) => (
            <li key={name}>
              {name} {req}
            </li>
          ))}
          {Object.keys(deps).length === 0 && <li className="muted">None</li>}
        </ul>
        <h3>Reverse dependencies</h3>
        <ul>
          {reverse.map((name) => (
            <li key={name}>
              <Link to={`/studio/apps/${name}`}>{name}</Link>
            </li>
          ))}
          {reverse.length === 0 && <li className="muted">None</li>}
        </ul>
        <h3>Entities</h3>
        <ul>
          {entities.map((e) => (
            <li key={String(e.name)}>
              <Link to={`/studio/entities/${e.name}`}>{String(e.label || e.name)}</Link>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="page">
      <h2>Apps</h2>
      <table>
        <thead>
          <tr>
            <th>App</th>
            <th>Version</th>
            <th>Source</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {apps.map((row) => (
            <tr key={String(row.name)}>
              <td>
                <Link to={`/studio/apps/${row.name}`}>{String(row.label || row.name)}</Link>
              </td>
              <td>{String(row.version)}</td>
              <td>{String(row.source)}</td>
              <td>{row.disabled ? "Disabled" : String(row.status)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
