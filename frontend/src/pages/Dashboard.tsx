import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, type TenantConfig, type UiEntity } from "../api";
import { Chart } from "../components/dashboards/Chart";
import { formatMoney } from "../metadata/timezone";
import { useTenantTheme } from "../metadata/context";

type Card = {
  title: string;
  entity: string;
  metric: string;
  kind?: string;
  chart?: string;
  value: number;
  series?: Array<{ label: string; value: number }>;
  items?: Record<string, unknown>[];
  total?: number;
};

export default function Dashboard({
  entities,
  config,
}: {
  entities: UiEntity[];
  config: TenantConfig | null;
}) {
  const [label, setLabel] = useState("Dashboard");
  const [cards, setCards] = useState<Card[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const theme = useTenantTheme();

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
        const data = await api.dashboard(dash.name);
        setLabel(data.label);
        setCards(data.cards);
        setError("");
      })
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [config]);

  function slugFor(entityName: string) {
    return entities.find((e) => e.entity === entityName)?.slug;
  }

  return (
    <div className="page">
      <div className="badge">Overview</div>
      <h2>{label}</h2>
      {error && (
        <p className="error" role="alert">
          Unable to load dashboard. {error}
        </p>
      )}
      {loading && <p className="muted">Loading dashboard…</p>}
      {cards.length === 0 && !error && !loading && (
        <div className="panel empty">
          No dashboard is configured for the applications enabled on this tenant.
        </div>
      )}
      <div className="cards">
        {cards.map((card) => {
          const slug = slugFor(card.entity);
          const kind = card.kind || "metric";
          if (kind === "chart" || kind === "status_breakdown") {
            return (
              <div key={card.title} className="card card-wide">
                <div className="muted">{card.title}</div>
                <Chart kind={card.chart || "bar"} series={card.series ?? []} />
              </div>
            );
          }
          if (kind === "list" || kind === "table" || kind === "activity") {
            return (
              <div key={card.title} className="card card-wide">
                <div className="muted">{card.title}</div>
                {(card.items ?? []).length === 0 ? (
                  <p className="empty">Nothing to show.</p>
                ) : (
                  <ul className="dash-list">
                    {(card.items ?? []).map((item) => (
                      <li key={String(item.id)}>
                        {slug ? (
                          <Link to={`/${slug}/${item.id}`}>
                            {String(item.name ?? item.title ?? item.code ?? item.id)}
                          </Link>
                        ) : (
                          String(item.name ?? item.id)
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            );
          }
          const display =
            card.metric === "sum"
              ? formatMoney(card.value, theme.currency, theme.locale)
              : String(card.value);
          const inner = (
            <>
              <div className="muted">{card.title}</div>
              <div className="card-value">{display}</div>
            </>
          );
          return slug ? (
            <Link key={card.title} className="card" to={`/${slug}`}>
              {inner}
            </Link>
          ) : (
            <div key={card.title} className="card">
              {inner}
            </div>
          );
        })}
      </div>
    </div>
  );
}
