import { useEffect, useState } from "react";
import { api, ApiError } from "../sdk/client";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { friendlyError } from "../friendlyError";
import { relativeTime } from "../format";
import { useTenantTheme } from "../metadata/context";

type AuditRow = {
  id?: string;
  actor?: string;
  user_id?: string;
  entity?: string;
  entity_id?: string;
  action?: string;
  operation?: string;
  created_at?: string;
  changes?: Record<string, { old?: unknown; new?: unknown }>;
};

export default function AuditLog() {
  const [items, setItems] = useState<AuditRow[]>([]);
  const [error, setError] = useState("");
  const [forbidden, setForbidden] = useState(false);
  const [loading, setLoading] = useState(true);
  const theme = useTenantTheme();

  useEffect(() => {
    api
      .audit()
      .then((d) => {
        setItems((d.items as AuditRow[]) ?? []);
        setError("");
        setForbidden(false);
      })
      .catch((e) => {
        if (e instanceof ApiError && e.status === 403) {
          setForbidden(true);
          setError("");
        } else {
          setError(friendlyError(e));
        }
      })
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <Skeleton rows={8} />;
  if (forbidden) {
    return (
      <div className="page">
        <PageHeader kicker="Security" title="Audit log" />
        <EmptyState title="Not authorized" description="Audit history is available to administrators only." />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        kicker="Security"
        title="Audit log"
        description="System record of who changed which records. Not shown on the business timeline."
      />
      {error ? <ErrorState message={error} /> : null}
      {items.length === 0 && !error ? (
        <EmptyState title="No audit events" description="Mutations that write audit rows will appear here." />
      ) : (
        <div className="panel table-wrap">
          <table className="data audit-table">
            <thead>
              <tr>
                <th>When</th>
                <th>Actor</th>
                <th>Entity</th>
                <th>Record</th>
                <th>Operation</th>
                <th>Changes</th>
              </tr>
            </thead>
            <tbody>
              {items.map((row, i) => (
                <tr key={String(row.id ?? i)}>
                  <td data-label="When">{relativeTime(row.created_at, theme.locale)}</td>
                  <td data-label="Actor">{row.actor || row.user_id || "—"}</td>
                  <td data-label="Entity">{row.entity}</td>
                  <td data-label="Record" className="mono">
                    {row.entity_id ? String(row.entity_id).slice(0, 8) : "—"}
                  </td>
                  <td data-label="Operation">{row.operation || row.action}</td>
                  <td data-label="Changes">
                    {row.changes && Object.keys(row.changes).length
                      ? Object.entries(row.changes)
                          .slice(0, 3)
                          .map(([field, change]) => `${field}: ${fmt(change.old)} → ${fmt(change.new)}`)
                          .join("; ")
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function fmt(value: unknown) {
  if (value == null || value === "") return "—";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}
