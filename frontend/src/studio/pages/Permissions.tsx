import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload, useStudioEntities } from "../StudioApp";

const ACTIONS = ["create", "read", "update", "delete", "list"];
const ROLES = ["Staff", "Manager", "Admin"];

export default function Permissions({ caps }: { caps: string[] }) {
  const { entity } = useParams();
  const entities = useStudioEntities();
  const [grants, setGrants] = useState<Array<{ role: string; entity: string; actions: string[] }>>([]);
  const [ops, setOps] = useState<Array<Record<string, unknown>>>([]);
  const [fieldLevels, setFieldLevels] = useState<
    Array<{ role: string; entity: string; level: number; read: boolean; write: boolean }>
  >([]);
  const [fields, setFields] = useState<
    Array<{ name: string; label?: string; permission_level: number; allow_on_submit: boolean }>
  >([]);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!entity) return;
    api.studioPermissions(entity).then((d) => {
      setGrants(d.grants);
      setFieldLevels(d.field_levels ?? []);
      setFields(d.fields ?? []);
    });
    api.studioOperations(entity).then((d) => setOps(d.operations));
  }, [entity]);

  if (!entity) {
    return (
      <div className="page">
        <h2>Permissions</h2>
        <ul>
          {entities.map((e) => (
            <li key={String(e.name)}>
              <Link to={`/studio/permissions/${e.name}`}>{String(e.label || e.name)}</Link>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  function allowed(role: string, action: string) {
    return grants.some((g) => g.role === role && g.actions.includes(action));
  }

  function toggle(role: string, action: string) {
    const existing = grants.find((g) => g.role === role);
    const next = grants.filter((g) => g.role !== role);
    const actions = new Set(existing?.actions ?? []);
    if (actions.has(action)) actions.delete(action);
    else actions.add(action);
    next.push({ role, entity: entity!, actions: [...actions] });
    setGrants(next);
  }

  return (
    <div className="page">
      <h2>{entity} permissions</h2>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Role</th>
              {ACTIONS.map((a) => (
                <th key={a}>{a}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {ROLES.map((role) => (
              <tr key={role}>
                <td>{role}</td>
                {ACTIONS.map((action) => (
                  <td key={action}>
                    <input
                      type="checkbox"
                      checked={role === "Admin" || allowed(role, action)}
                      disabled={role === "Admin" || !can(caps, "studio.manage_permissions")}
                      onChange={() => toggle(role, action)}
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {can(caps, "studio.manage_permissions") ? (
        <button
          type="button"
          onClick={() =>
            publishAndReload({ kind: "permissions", target: entity, payload: grants.filter((g) => g.role !== "Admin") })
              .catch((e) => setError(e.message))
          }
        >
          Publish
        </button>
      ) : null}
      <h3>Operations</h3>
      <ul>
        {ops.map((op) => (
          <li key={String(op.name)}>
            {String(op.label || op.name)} · {(op.roles as string[] | undefined)?.join(", ") || "any role"}
            {op.source_managed ? <span className="muted"> · Custom Rust operation · Source-managed</span> : null}
          </li>
        ))}
      </ul>
      {error ? <p className="error">{error}</p> : null}
      <h3>Field Permissions</h3>
      <p className="muted">Level 0 is normal. Higher levels require a matching role grant. Enforcement is server-side.</p>
      <div className="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Field</th>
              <th>Level</th>
              {["Staff", "Manager", "HR", "Admin"].map((role) => (
                <th key={role}>{role}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {fields
              .filter((f) => f.permission_level > 0)
              .map((f) => (
                <tr key={f.name}>
                  <td>{f.label || f.name}</td>
                  <td>{f.permission_level}</td>
                  {["Staff", "Manager", "HR", "Admin"].map((role) => {
                    const grant = fieldLevels.find((g) => g.role === role && g.level >= f.permission_level);
                    const admin = role === "Admin";
                    return (
                      <td key={role}>
                        Read {admin || grant?.read ? "✓" : "✗"} Write {admin || grant?.write ? "✓" : "✗"}
                      </td>
                    );
                  })}
                </tr>
              ))}
            {fields.filter((f) => f.permission_level > 0).length === 0 ? (
              <tr>
                <td colSpan={6} className="muted">
                  No restricted fields.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </div>
  );
}
