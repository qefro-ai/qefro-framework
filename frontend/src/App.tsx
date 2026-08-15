import { Navigate, NavLink, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import StudioApp from "./studio/StudioApp";
import { api, ApiError, clearToken, hasToken, METADATA_EVENT, onAuthChange, type TenantConfig, type UiEntity } from "./api";
import { TenantThemeContext } from "./metadata/context";
import "./widgets";
import Login from "./pages/Login";
import EntityList from "./pages/EntityList";
import EntityForm from "./pages/EntityForm";
import EntityDetail from "./pages/EntityDetail";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import Reports from "./pages/Reports";
import PublicForm from "./pages/PublicForm";
import GlobalSearch from "./components/GlobalSearch";
import NotificationBell from "./components/NotificationBell";

export default function App() {
  const [authed, setAuthed] = useState(hasToken());

  useEffect(() => onAuthChange(() => setAuthed(hasToken())), []);

  if (!authed) {
    return (
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/p/:tenant/:form" element={<PublicForm />} />
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
  const [studio, setStudio] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    function loadUi() {
      api
        .ui()
        .then((d) => {
          setEntities(d.entities);
          if (d.branding) {
            setConfig((prev) =>
              prev
                ? { ...prev, branding: { ...prev.branding, ...d.branding } }
                : {
                    branding: d.branding ?? {},
                    ui_config: {
                      navigation: d.navigation ?? [],
                      hidden_entities: [],
                      default_dashboard: d.default_dashboard,
                      terminology: d.terminology,
                    },
                    enabled_apps: d.enabled_apps ?? [],
                    business: {
                      locale: d.locale,
                      timezone: d.timezone,
                      currency: d.currency,
                    },
                    features: { flags: d.features },
                  },
            );
          }
        })
        .catch((err) => {
          if (err instanceof ApiError && (err.status === 401 || err.status === 403)) {
            clearToken();
            navigate("/login");
          }
        });
    }
    loadUi();
    api
      .me()
      .then((d) => {
        setMe(`${d.user.name} · ${d.roles.join(", ")}`);
        setStudio((d.studio ?? []).includes("studio.view"));
      })
      .catch(() => undefined);
    api.tenantConfig().then(setConfig).catch(() => undefined);
    window.addEventListener(METADATA_EVENT, loadUi);
    return () => window.removeEventListener(METADATA_EVENT, loadUi);
  }, [navigate]);

  useEffect(() => {
    const root = document.documentElement;
    const primary = config?.branding.primary_color;
    const accent = config?.branding.accent_color || primary;
    if (accent) root.style.setProperty("--accent", accent);
    if (primary) root.style.setProperty("--primary", primary);
    const secondary = config?.branding.secondary_color;
    if (secondary) root.style.setProperty("--secondary", secondary);
    const name =
      config?.branding.company_name || config?.branding.app_name || "Workspace";
    document.title = name;
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
    const visible = entities.filter(
      (e) => !hidden.has(e.slug) && !hidden.has(e.entity) && e.standalone !== false,
    );
    if (ordered.length === 0) return visible;
    const bySlug = new Map(visible.map((e) => [e.slug, e]));
    const picked = ordered.map((slug) => bySlug.get(slug)).filter(Boolean) as UiEntity[];
    const rest = visible.filter((e) => !ordered.includes(e.slug));
    return [...picked, ...rest];
  }, [entities, config]);

  const appName =
    config?.branding.company_name || config?.branding.app_name || "Workspace";
  const theme = {
    timezone: config?.business?.timezone || "UTC",
    locale: config?.business?.locale || "en-US",
    currency: config?.business?.currency || "USD",
  };

  if (location.pathname.startsWith("/studio")) {
    return (
      <TenantThemeContext.Provider value={theme}>
        <Routes>
          <Route path="/studio/*" element={<StudioApp />} />
        </Routes>
      </TenantThemeContext.Provider>
    );
  }

  return (
    <TenantThemeContext.Provider value={theme}>
    <div className="shell">
      <a className="skip-link" href="#main">Skip to content</a>
      <aside className="nav" aria-label="Application">
        <div className="nav-brand">
          {config?.branding.logo ? <img src={config.branding.logo} alt="" className="logo" /> : null}
          <h1>{appName}</h1>
        </div>
        <div className="nav-links">
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
          <NavLink to="/reports" className={({ isActive }) => (isActive ? "active" : "")}>
            Reports
          </NavLink>
          <NavLink to="/settings" className={({ isActive }) => (isActive ? "active" : "")}>
            Settings
          </NavLink>
          {studio ? (
            <NavLink to="/studio" className={({ isActive }) => (isActive ? "active" : "")}>
              Studio
            </NavLink>
          ) : null}
        </div>
        <div className="nav-footer">
          <GlobalSearch />
          <NotificationBell />
          <div className="badge">{me || "Signed in"}</div>
          <button
            className="ghost"
            onClick={() => {
              clearToken();
              navigate("/login");
            }}
          >
            Log out
          </button>
        </div>
      </aside>
      <main id="main" className="main" tabIndex={-1}>
        <Routes>
          <Route path="/" element={<Dashboard entities={entities} config={config} />} />
          <Route path="/login" element={<Navigate to="/" replace />} />
          <Route
            path="/settings"
            element={<Settings config={config} onSaved={setConfig} />}
          />
          <Route path="/reports" element={<Reports />} />
          <Route path="/p/:tenant/:form" element={<PublicForm />} />
          <Route path="/:slug" element={<EntityList entities={entities} />} />
          <Route path="/:slug/new" element={<EntityForm entities={entities} />} />
          <Route path="/:slug/:id" element={<EntityDetail entities={entities} />} />
          <Route path="/:slug/:id/edit" element={<EntityForm entities={entities} />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
    </div>
    </TenantThemeContext.Provider>
  );
}
