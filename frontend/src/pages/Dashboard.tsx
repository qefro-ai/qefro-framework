import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { api, type TenantConfig, type UiEntity } from "../api";
import { Chart } from "../components/dashboards/Chart";
import { EmptyState, ErrorState, Skeleton } from "../components/ui/EmptyState";
import { datePresetRange } from "../format";
import { friendlyError } from "../friendlyError";
import { dateFieldsFromFilters, drilldownPath } from "../metadata/dashboard";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";
import { useRealtime } from "../realtime";

type Card = {
  title: string;
  entity: string;
  metric: string;
  kind?: string;
  chart?: string;
  group_by?: string;
  filters?: Array<{ field: string; value: string }>;
  value: number;
  series?: Array<{ label: string; value: number }>;
  items?: Record<string, unknown>[];
  total?: number;
  size?: string | null;
  rows?: Array<Record<string, unknown>>;
};

function cardClass(card: Card) {
  const size = card.size || "";
  const kind = card.kind || "metric";
  const wide = ["chart", "status_breakdown", "workflow", "list", "table", "activity", "saved_view", "report"].includes(kind);
  const classes = ["card"];
  if (size === "xl" || (wide && size !== "sm")) classes.push("card-wide");
  if (size === "lg") classes.push("card-lg");
  if (size === "sm") classes.push("card-sm");
  return classes.join(" ");
}

function itemLabel(item: Record<string, unknown>) {
  const expanded = item._expanded as Record<string, { label?: unknown }> | undefined;
  const related = expanded
    ? Object.values(expanded)
        .map((rel) => (rel && typeof rel.label === "string" ? rel.label : ""))
        .find((s) => s)
    : "";
  return String(
    item.name ??
      item.title ??
      item.guest_name ??
      item.code ??
      item.message ??
      item.doc_no ??
      related ??
      item.id,
  );
}

export default function Dashboard({
  entities,
  config,
}: {
  entities: UiEntity[];
  config: TenantConfig | null;
}) {
  const [label, setLabel] = useState("Dashboard");
  const [name, setName] = useState("");
  const [cards, setCards] = useState<Card[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);
  const [datePreset, setDatePreset] = useState("");
  const [status, setStatus] = useState("");
  const [branch, setBranch] = useState("");
  const [segment, setSegment] = useState<{ field: string; value: string } | null>(null);
  const theme = useTenantTheme();
  const navigate = useNavigate();

  const quick = useMemo(() => {
    const all = entities.filter((e) => e.standalone !== false && !e.singleton && !e.child_of);
    const nav = config?.ui_config.navigation ?? [];
    if (nav.length === 0) return all.slice(0, 6);
    const bySlug = new Map(all.map((e) => [e.slug, e]));
    return nav.map((slug) => bySlug.get(slug)).filter(Boolean).slice(0, 6) as UiEntity[];
  }, [entities, config]);

  const dateFieldsRef = useRef<string[]>([]);
  const extraKey = `${datePreset}|${status}|${branch}|${segment?.field ?? ""}=${segment?.value ?? ""}`;

  useEffect(() => {
    setLoading(true);
    api
      .dashboards()
      .then(async (meta) => {
        const preferred = config?.ui_config.default_dashboard;
        const dash = meta.dashboards.find((d) => d.name === preferred) ?? meta.dashboards[0];
        if (!dash) {
          setCards([]);
          return;
        }
        setName(dash.name);
        const extra = new URLSearchParams();
        if (status) extra.set("status", status);
        if (branch) extra.set("branch_id", branch);
        if (segment) extra.set(segment.field, segment.value);
        if (datePreset) {
          const range = datePresetRange(datePreset);
          if (range) {
            for (const field of dateFieldsRef.current) {
              extra.set(`${field}.between`, `${range.from},${range.to}`);
            }
          }
        }
        const data = await api.dashboard(dash.name, extra.toString() ? extra : undefined);
        setLabel(data.label);
        setCards(data.cards);
        const fields = new Set<string>();
        for (const card of data.cards) {
          for (const name of dateFieldsFromFilters(card.filters)) fields.add(name);
        }
        dateFieldsRef.current = [...fields];
        setError("");
      })
      .catch((e) => setError(friendlyError(e)))
      .finally(() => setLoading(false));
  }, [config, tick, extraKey, status, branch, datePreset]);

  useRealtime({}, () => setTick((n) => n + 1));

  function slugFor(entityName: string) {
    return entities.find((e) => e.entity === entityName)?.slug;
  }

  const statuses = useMemo(() => {
    const found = new Set<string>();
    for (const card of cards) {
      for (const row of card.series ?? []) found.add(row.label);
      for (const filter of card.filters ?? []) {
        if (filter.field === "status") found.add(filter.value);
      }
    }
    return [...found].filter(Boolean);
  }, [cards]);

  const hasDate = cards.some((c) => dateFieldsFromFilters(c.filters).length > 0);
  const hasBranch = entities.some((e) => e.fields.some((f) => f.name === "branch_id"));

  return (
    <div className="page workspace">
      <div className="badge">Overview</div>
      <h2>{label}</h2>
      {quick.length > 0 ? (
        <div className="quick-actions">
          {quick.map((e) => (
            <Link key={e.slug} to={`/${e.slug}/new`}>
              <button type="button">New {e.label}</button>
            </Link>
          ))}
        </div>
      ) : null}
      {cards.length > 0 ? (
        <div className="dash-filters">
          {hasDate ? (
            <label>
              Date range
              <select value={datePreset} onChange={(e) => setDatePreset(e.target.value)} aria-label="Date range">
                <option value="">All</option>
                <option value="today">Today</option>
                <option value="this_week">This week</option>
                <option value="this_month">This month</option>
                <option value="last_7_days">Last 7 days</option>
              </select>
            </label>
          ) : null}
          {statuses.length > 0 ? (
            <label>
              Status
              <select value={status} onChange={(e) => { setStatus(e.target.value); setSegment(e.target.value ? { field: "status", value: e.target.value } : null); }} aria-label="Status">
                <option value="">All</option>
                {statuses.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
          {hasBranch ? (
            <label>
              Branch
              <select value={branch} onChange={(e) => setBranch(e.target.value)} aria-label="Branch">
                <option value="">All</option>
              </select>
            </label>
          ) : null}
        </div>
      ) : null}
      {error && <ErrorState message={`Unable to load dashboard. ${error}`} />}
      {loading && <Skeleton rows={4} />}
      {cards.length === 0 && !error && !loading && (
        <EmptyState title="No dashboard is configured" description="Enable an application with workspace cards." />
      )}
      <div className="cards">
        {cards.map((card) => {
          const slug = slugFor(card.entity);
          const kind = card.kind || "metric";
          if (kind === "chart" || kind === "status_breakdown" || kind === "workflow" || kind === "report") {
            const series = card.series ?? (card.rows ?? []).map((row) => ({
              label: String(row.label ?? Object.values(row)[0] ?? ""),
              value: Number(row.value ?? Object.values(row)[1] ?? 0),
            }));
            return (
              <div key={card.title} className={cardClass(card)}>
                <div className="muted">{card.title}</div>
                <Chart
                  kind={
                    kind === "workflow" || kind === "status_breakdown"
                      ? "bar"
                      : card.chart || "bar"
                  }
                  series={series}
                  onSegmentClick={
                    card.group_by
                      ? (label) => {
                          if (slug && (kind === "workflow" || kind === "status_breakdown")) {
                            const params = new URLSearchParams();
                            params.set(card.group_by as string, label);
                            navigate(`/${slug}?${params.toString()}`);
                            return;
                          }
                          setSegment({ field: card.group_by as string, value: label });
                          if (card.group_by === "status") setStatus(label);
                        }
                      : undefined
                  }
                />
              </div>
            );
          }
          if (kind === "list" || kind === "table" || kind === "activity" || kind === "saved_view") {
            return (
              <div key={card.title} className={cardClass(card)}>
                <div className="muted">{card.title}</div>
                {(card.items ?? []).length === 0 ? (
                  <p className="empty">Nothing to show.</p>
                ) : (
                  <ul className="dash-list">
                    {(card.items ?? []).map((item) => {
                      const id = String(item.id ?? item.entity_id ?? "");
                      const href = slug && id ? (item.entity_id ? `/${slug}/${item.entity_id}` : `/${slug}/${item.id}`) : "";
                      return (
                        <li key={id || itemLabel(item)}>
                          {href ? <Link to={href}>{itemLabel(item)}</Link> : itemLabel(item)}
                          {item.created_at ? <span className="muted"> · {String(item.created_at).slice(11, 16)}</span> : null}
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            );
          }
          const display =
            card.metric === "sum" || card.metric === "avg"
              ? formatMoney(card.value, theme.currency, theme.locale)
              : String(card.value);
          const inner = (
            <>
              <div className="muted">{card.title}</div>
              <div className="card-value">{display}</div>
            </>
          );
          const href = slug && slug !== "_audit" ? drilldownPath(slug, card.filters) : "";
          return slug && href ? (
            <Link key={card.title} className={cardClass(card)} to={href}>
              {inner}
            </Link>
          ) : (
            <div key={card.title} className={cardClass(card)}>
              {inner}
            </div>
          );
        })}
      </div>
      {quick.length > 0 ? (
        <div className="shortcuts panel">
          <h3>Shortcuts</h3>
          <ul>
            {quick.map((e) => (
              <li key={e.slug}>
                <Link to={`/${e.slug}`}>{e.label_plural}</Link>
              </li>
            ))}
            <li>
              <Link to="/reports">Saved reports</Link>
            </li>
          </ul>
        </div>
      ) : null}
      {name ? <p className="sr-only">{name}</p> : null}
    </div>
  );
}
