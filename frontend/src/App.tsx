import { Navigate, NavLink, Route, Routes, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import { api, clearToken, hasToken, type TenantConfig, type UiEntity } from "./api";
import Login from "./pages/Login";
import EntityList from "./pages/EntityList";
import EntityForm from "./pages/EntityForm";
import EntityDetail from "./pages/EntityDetail";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";

export default function App() {
  if (!hasToken()) {
    return (
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    );
  }
  return <Shell />;
}

function Shell() {
  const [entities, setEntities] = useState<UiEntity[]>([]);
  const [config, setConfig] = useState<TenantConfig | null>(null);
  const [me, setMe] = useState("");
  const navigate = useNavigate();

  useEffect(() => {
    api
      .ui()
      .then((d) => setEntities(d.entities))
      .catch(() => {
        clearToken();
        navigate("/login");
      });
    api
      .me()
      .then((d) => setMe(`${d.user.name} · ${d.roles.join(", ")}`))
      .catch(() => undefined);
    api.tenantConfig().then(setConfig).catch(() => undefined);
  }, [navigate]);

  useEffect(() => {
    const color = config?.branding.primary_color;
    if (color) document.documentElement.style.setProperty("--accent", color);
    const name = config?.branding.app_name;
    if (name) document.title = name;
    const favicon = config?.branding.favicon;
    if (favicon) {
      let link = document.querySelector("link[rel='icon']") as HTMLLinkElement | null;
      if (!link) {
        link = document.createElement("link");
        link.rel = "icon";
        document.head.appendChild(link);
      }
      link.href = favicon;
    }
  }, [config]);

  const navEntities = useMemo(() => {
    const hidden = new Set(config?.ui_config.hidden_entities ?? []);
    const ordered = config?.ui_config.navigation ?? [];
    const visible = entities.filter((e) => !hidden.has(e.slug) && !hidden.has(e.entity));
    if (ordered.length === 0) return visible;
    const bySlug = new Map(visible.map((e) => [e.slug, e]));
    const picked = ordered.map((slug) => bySlug.get(slug)).filter(Boolean) as UiEntity[];
    const rest = visible.filter((e) => !ordered.includes(e.slug));
    return [...picked, ...rest];
  }, [entities, config]);

  const appName = config?.branding.app_name || "Qefro";

  return (
    <div className="shell">
      <aside className="nav">
        {config?.branding.logo ? <img src={config.branding.logo} alt="" className="logo" /> : null}
        <h1>{appName}</h1>
        <div className="badge">{me}</div>
        <p className="muted">Metadata UI</p>
        <NavLink to="/" className={({ isActive }) => (isActive ? "active" : "")} end>
          Dashboard
        </NavLink>
        {navEntities.map((e) => (
          <NavLink
            key={e.slug}
            to={`/${e.slug}`}
            className={({ isActive }) => (isActive ? "active" : "")}
          >
            {e.label_plural}
          </NavLink>
        ))}
        <NavLink to="/settings" className={({ isActive }) => (isActive ? "active" : "")}>
          Settings
        </NavLink>
        <p>
          <button
            className="ghost"
            onClick={() => {
              clearToken();
              navigate("/login");
            }}
          >
            Log out
          </button>
        </p>
      </aside>
      <main className="main">
        <Routes>
          <Route path="/" element={<Dashboard entities={entities} config={config} />} />
          <Route path="/login" element={<Navigate to="/" replace />} />
          <Route
            path="/settings"
            element={<Settings config={config} onSaved={setConfig} />}
          />
          <Route path="/:slug" element={<EntityList entities={entities} />} />
          <Route path="/:slug/new" element={<EntityForm entities={entities} />} />
          <Route path="/:slug/:id" element={<EntityDetail entities={entities} />} />
          <Route path="/:slug/:id/edit" element={<EntityForm entities={entities} />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
    </div>
  );
}
