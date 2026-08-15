import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload } from "../StudioApp";
import SourceView from "../components/SourceView";

export default function ReportsStudio({
  kind,
  caps,
}: {
  kind: "reports" | "dashboards" | "print";
  caps: string[];
}) {
  const { name } = useParams();
  const [items, setItems] = useState<Array<Record<string, unknown>>>([]);
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [html, setHtml] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (kind === "reports") api.studioReports().then((d) => setItems(d.reports));
    if (kind === "dashboards") api.studioDashboards().then((d) => setItems(d.dashboards));
    if (kind === "print") api.studioPrintFormats().then((d) => setItems(d.print_formats));
  }, [kind]);

  useEffect(() => {
    if (!name) {
      setDetail(null);
      return;
    }
    const load =
      kind === "reports"
        ? api.studioReport(name)
        : kind === "dashboards"
          ? api.studioDashboard(name)
          : api.studioPrintFormat(name);
    load.then(setDetail);
    if (kind === "print") api.studioPrintPreview(name).then((d) => setHtml(d.html));
  }, [kind, name]);

  const title = kind === "reports" ? "Reports" : kind === "dashboards" ? "Dashboards" : "Print Formats";
  const path = kind === "reports" ? "reports" : kind === "dashboards" ? "dashboards" : "print-formats";
  const key = kind === "print" ? "print_format" : kind === "dashboards" ? "dashboard" : "report";

  if (!name) {
    return (
      <div className="page">
        <h2>{title}</h2>
        <ul>
          {items.map((item) => (
            <li key={String(item.name)}>
              <Link to={`/studio/${path}/${item.name}`}>{String(item.label || item.name)}</Link>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  const payload = (detail?.[key] as Record<string, unknown>) ?? detail ?? {};

  return (
    <div className="page">
      <p>
        <Link to={`/studio/${path}`}>{title}</Link>
      </p>
      <h2>{String(payload.label || payload.name || name)}</h2>
      {kind === "print" && html ? (
        <iframe title="Print preview" className="print-preview" srcDoc={html} />
      ) : null}
      <SourceView jsonText={String(detail?.json ?? "")} yamlText={String(detail?.yaml ?? "")} />
      {can(caps, "studio.publish") && kind !== "print" ? (
        <button
          type="button"
          onClick={() =>
            publishAndReload({
              kind: kind === "reports" ? "report" : "dashboard",
              target: name,
              payload,
            }).catch((e) => setError(e.message))
          }
        >
          Publish
        </button>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}
