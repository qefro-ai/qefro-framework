import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api } from "../../api";
import { can, publishAndReload } from "../StudioApp";
import SourceView from "../components/SourceView";

type CardDraft = {
  title: string;
  entity: string;
  kind: string;
  metric?: string;
  chart?: string;
  group_by?: string;
  field?: string;
  size?: string;
  saved_view?: string;
  report?: string;
  limit?: number;
};

const KINDS = ["kpi", "metric", "chart", "table", "list", "activity", "workflow", "saved_view", "report"];
const SIZES = ["sm", "md", "lg", "xl"];
const CHARTS = ["bar", "line", "area", "pie", "donut"];
const SECTION_KINDS = ["header", "customer", "address", "items", "totals", "notes", "terms", "footer", "text", "image"];
const VARIANTS = ["default", "compact", "professional"];

type SectionDraft = {
  kind: string;
  title?: string;
  fields?: string[];
  text?: string;
  loop_over?: string;
  show_when?: string;
};

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
  const [cards, setCards] = useState<CardDraft[]>([]);
  const [sections, setSections] = useState<SectionDraft[]>([]);
  const [variant, setVariant] = useState("default");
  const [body, setBody] = useState("");

  useEffect(() => {
    if (kind === "reports") api.studioReports().then((d) => setItems(d.reports));
    if (kind === "dashboards") api.studioDashboards().then((d) => setItems(d.dashboards));
    if (kind === "print") api.studioPrintFormats().then((d) => setItems(d.print_formats));
  }, [kind]);

  useEffect(() => {
    if (!name) {
      setDetail(null);
      setCards([]);
      return;
    }
    const load =
      kind === "reports"
        ? api.studioReport(name)
        : kind === "dashboards"
          ? api.studioDashboard(name)
          : api.studioPrintFormat(name);
    load.then((d) => {
      setDetail(d);
      const payload = (d.dashboard ?? d.report ?? d.print_format ?? d) as Record<string, unknown>;
      const next = (payload.cards as CardDraft[] | undefined) ?? [];
      setCards(next.map((c) => ({ ...c })));
      setSections(((payload.sections as SectionDraft[] | undefined) ?? []).map((s) => ({ ...s })));
      setVariant(String(payload.variant || "default"));
      setBody(String(payload.body || ""));
    });
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

  const payload = {
    ...((detail?.[key] as Record<string, unknown>) ?? detail ?? {}),
    ...(kind === "dashboards" ? { cards } : {}),
    ...(kind === "print" ? { sections, variant, body: body || undefined } : {}),
  };

  return (
    <div className="page">
      <p>
        <Link to={`/studio/${path}`}>{title}</Link>
      </p>
      <h2>{String(payload.label || payload.name || name)}</h2>
      {kind === "print" && html ? (
        <iframe title="Print preview" className="print-preview" srcDoc={html} />
      ) : null}
      {kind === "print" ? (
        <PrintFormatEditor
          sections={sections}
          variant={variant}
          body={body}
          canEdit={can(caps, "studio.publish")}
          onVariant={setVariant}
          onBody={setBody}
          onChange={setSections}
        />
      ) : null}
      {kind === "dashboards" ? (
        <DashboardCardEditor cards={cards} onChange={setCards} canEdit={can(caps, "studio.publish")} />
      ) : null}
      <SourceView jsonText={String(detail?.json ?? "")} yamlText={String(detail?.yaml ?? "")} />
      {can(caps, "studio.publish") ? (
        <button
          type="button"
          onClick={() =>
            publishAndReload({
              kind: kind === "reports" ? "report" : kind === "dashboards" ? "dashboard" : "print_format",
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

function DashboardCardEditor({
  cards,
  onChange,
  canEdit,
}: {
  cards: CardDraft[];
  onChange: (cards: CardDraft[]) => void;
  canEdit: boolean;
}) {
  function update(i: number, patch: Partial<CardDraft>) {
    const next = cards.slice();
    next[i] = { ...next[i], ...patch };
    onChange(next);
  }
  function move(i: number, dir: number) {
    const j = i + dir;
    if (j < 0 || j >= cards.length) return;
    const next = cards.slice();
    const [item] = next.splice(i, 1);
    next.splice(j, 0, item);
    onChange(next);
  }
  return (
    <div className="panel">
      <h3>Widgets</h3>
      {cards.map((card, i) => (
        <fieldset key={`${card.title}-${i}`} className="card-editor">
          <legend>{card.title || `Widget ${i + 1}`}</legend>
          <label>
            Title
            <input value={card.title} disabled={!canEdit} onChange={(e) => update(i, { title: e.target.value })} />
          </label>
          <label>
            Source entity
            <input value={card.entity} disabled={!canEdit} onChange={(e) => update(i, { entity: e.target.value })} />
          </label>
          <label>
            Kind
            <select value={card.kind} disabled={!canEdit} onChange={(e) => update(i, { kind: e.target.value })}>
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label>
            Size
            <select value={card.size || "md"} disabled={!canEdit} onChange={(e) => update(i, { size: e.target.value })}>
              {SIZES.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </label>
          {card.kind === "chart" || card.kind === "workflow" ? (
            <>
              <label>
                Chart
                <select value={card.chart || "bar"} disabled={!canEdit} onChange={(e) => update(i, { chart: e.target.value })}>
                  {CHARTS.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Dimension
                <input
                  value={card.group_by || ""}
                  disabled={!canEdit}
                  onChange={(e) => update(i, { group_by: e.target.value })}
                />
              </label>
            </>
          ) : null}
          {card.kind === "saved_view" ? (
            <label>
              Saved view
              <input
                value={card.saved_view || ""}
                disabled={!canEdit}
                onChange={(e) => update(i, { saved_view: e.target.value })}
              />
            </label>
          ) : null}
          {card.kind === "report" ? (
            <label>
              Report
              <input value={card.report || ""} disabled={!canEdit} onChange={(e) => update(i, { report: e.target.value })} />
            </label>
          ) : null}
          <div className="filter-ops">
            <button type="button" className="ghost" disabled={!canEdit || i === 0} onClick={() => move(i, -1)}>
              Up
            </button>
            <button type="button" className="ghost" disabled={!canEdit || i === cards.length - 1} onClick={() => move(i, 1)}>
              Down
            </button>
            <button
              type="button"
              className="ghost"
              disabled={!canEdit}
              onClick={() => onChange(cards.filter((_, idx) => idx !== i))}
            >
              Remove
            </button>
          </div>
        </fieldset>
      ))}
      {canEdit ? (
        <button
          type="button"
          onClick={() => onChange([...cards, { title: "New widget", entity: "", kind: "kpi", size: "md" }])}
        >
          Add widget
        </button>
      ) : null}
    </div>
  );
}

function PrintFormatEditor({
  sections,
  variant,
  body,
  canEdit,
  onVariant,
  onBody,
  onChange,
}: {
  sections: SectionDraft[];
  variant: string;
  body: string;
  canEdit: boolean;
  onVariant: (v: string) => void;
  onBody: (v: string) => void;
  onChange: (sections: SectionDraft[]) => void;
}) {
  function update(i: number, patch: Partial<SectionDraft>) {
    const next = sections.slice();
    next[i] = { ...next[i], ...patch };
    onChange(next);
  }
  return (
    <div className="card">
      <h3>Document template</h3>
      <p className="muted">Select sections, fields, and child tables. JavaScript, SQL, and URLs are rejected.</p>
      <label>
        Variant
        <select value={variant} disabled={!canEdit} onChange={(e) => onVariant(e.target.value)}>
          {VARIANTS.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </label>
      {sections.map((section, i) => (
        <fieldset key={`${section.kind}-${i}`}>
          <legend>{section.kind || "Section"}</legend>
          <label>
            Kind
            <select
              value={section.kind}
              disabled={!canEdit}
              onChange={(e) => update(i, { kind: e.target.value })}
            >
              {SECTION_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label>
            Title
            <input
              value={section.title || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { title: e.target.value })}
            />
          </label>
          <label>
            Fields (comma-separated paths)
            <input
              value={(section.fields || []).join(", ")}
              disabled={!canEdit}
              onChange={(e) =>
                update(i, {
                  fields: e.target.value
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean),
                })
              }
            />
          </label>
          <label>
            Child table
            <input
              value={section.loop_over || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { loop_over: e.target.value })}
            />
          </label>
          <label>
            Show when
            <input
              value={section.show_when || ""}
              disabled={!canEdit}
              placeholder="discount > 0"
              onChange={(e) => update(i, { show_when: e.target.value })}
            />
          </label>
          <label>
            Text
            <textarea
              value={section.text || ""}
              disabled={!canEdit}
              onChange={(e) => update(i, { text: e.target.value })}
            />
          </label>
          {canEdit ? (
            <button type="button" className="ghost" onClick={() => onChange(sections.filter((_, idx) => idx !== i))}>
              Remove
            </button>
          ) : null}
        </fieldset>
      ))}
      {canEdit ? (
        <button type="button" onClick={() => onChange([...sections, { kind: "text" }])}>
          Add section
        </button>
      ) : null}
      <label>
        Optional body template
        <textarea value={body} disabled={!canEdit} onChange={(e) => onBody(e.target.value)} />
      </label>
    </div>
  );
}
