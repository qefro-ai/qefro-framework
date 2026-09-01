import { FormEvent, useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Chart } from "../components/dashboards/Chart";
import { PageHeader } from "../components/ui/PageHeader";
import { ErrorState } from "../components/ui/EmptyState";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";

type ReportMeta = {
  name: string;
  label: string;
  entity: string;
  fields?: string[];
  group_by?: string[];
  aggregations?: Record<string, string>;
  chart?: string | null;
};

type ReportResult = {
  name: string;
  label: string;
  chart?: string;
  rows: Array<Record<string, unknown>>;
  series?: Array<{ label: string; value: number }>;
};

export default function Reports() {
  const [reports, setReports] = useState<ReportMeta[]>([]);
  const [selected, setSelected] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [result, setResult] = useState<ReportResult | null>(null);
  const [error, setError] = useState("");
  const [running, setRunning] = useState(false);
  const theme = useTenantTheme();
  const current = useMemo(() => reports.find((r) => r.name === selected), [reports, selected]);

  useEffect(() => {
    api
      .reports()
      .then((d) => {
        setReports(d.reports ?? []);
        setSelected(d.reports?.[0]?.name ?? "");
      })
      .catch((e) => setError(e.message));
  }, []);

  async function onRun(e: FormEvent) {
    e.preventDefault();
    if (!selected) return;
    const filters = [];
    const groupField = current?.group_by?.[0];
    if (from && to && groupField) {
      filters.push({ op: "between", field: groupField, from, to });
    }
    try {
      setRunning(true);
      setError("");
      const data = await api.runReport(selected, { filters });
      setResult(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to run report");
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="page">
      <PageHeader
        kicker="Analytics"
        title="Reports"
        description="Run saved reports against live records."
      />
      <form className="form report-toolbar" onSubmit={onRun}>
        <label>
          Report
          <select value={selected} onChange={(e) => setSelected(e.target.value)}>
            {reports.map((r) => (
              <option key={r.name} value={r.name}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          From
          <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
        </label>
        <label>
          To
          <input type="date" value={to} onChange={(e) => setTo(e.target.value)} />
        </label>
        <button type="submit" disabled={running || !selected}>
          {running ? "Running…" : "Run Report"}
        </button>
      </form>
      {error && <ErrorState message={error} />}
      {result && (
        <div className="panel report-result">
          <h3>{result.label}</h3>
          {result.series && result.series.length > 0 && (
            <Chart kind={result.chart || "bar"} series={result.series.map((s) => ({ label: String(s.label), value: Number(s.value) }))} />
          )}
          <table className="data">
            <thead>
              <tr>
                {Object.keys(result.rows[0] ?? {}).map((k) => (
                  <th key={k}>{k}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.rows.map((row, i) => (
                <tr key={i}>
                  {Object.entries(row).map(([k, v]) => (
                    <td key={k}>
                      {typeof v === "number" ? formatMoney(v, theme.locale, theme.currency) : String(v ?? "")}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
