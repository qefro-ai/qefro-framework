import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, type TenantConfig, type UiEntity } from "../api";

export default function Dashboard({
  entities,
  config,
}: {
  entities: UiEntity[];
  config: TenantConfig | null;
}) {
  const [label, setLabel] = useState("Dashboard");
  const [cards, setCards] = useState<Array<{ title: string; entity: string; value: number }>>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .dashboards()
      .then(async (meta) => {
        const preferred = config?.ui_config.default_dashboard;
        const dash =
          meta.dashboards.find((d) => d.name === preferred) ?? meta.dashboards[0];
        if (!dash) {
          setCards([]);
          return;
        }
        const data = await api.dashboard(dash.name);
        setLabel(data.label);
        setCards(data.cards);
      })
      .catch((e) => setError(e.message));
  }, [config]);

  function slugFor(entityName: string) {
    return entities.find((e) => e.entity === entityName)?.slug;
  }

  return (
    <div>
      <div className="badge">Overview</div>
      <h2>{label}</h2>
      {error && <p className="error">{error}</p>}
      {cards.length === 0 && !error && (
        <p className="muted">No dashboard is configured for the applications enabled on this tenant.</p>
      )}
      <div className="cards">
        {cards.map((card) => {
          const slug = slugFor(card.entity);
          const inner = (
            <>
              <div className="muted">{card.title}</div>
              <div className="card-value">{card.value}</div>
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
