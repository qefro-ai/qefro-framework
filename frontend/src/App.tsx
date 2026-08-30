import { Navigate, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import StudioApp from "./studio/StudioApp";
import { api, ApiError, clearToken, hasToken, METADATA_EVENT, onAuthChange, type TenantConfig, type UiEntity } from "./api";
import { TenantThemeContext } from "./metadata/context";
import { primaryNavEntities } from "./metadata/navigation";
import type { WorkspaceNavItem } from "./metadata/types";
import { PrefsProvider } from "./prefsContext";
import { AppShell } from "./components/shell/AppShell";
import "./widgets";
import Login from "./pages/Login";
import EntityList from "./pages/EntityList";
import EntityForm from "./pages/EntityForm";
import EntityDetail from "./pages/EntityDetail";
import Dashboard from "./pages/Dashboard";
import Settings from "./pages/Settings";
import AuditLog from "./pages/AuditLog";
import Reports from "./pages/Reports";
import PublicForm from "./pages/PublicForm";
import { SnackbarHost } from "./components/ui/Snackbar";

export default function App() {
  const [authed, setAuthed] = useState(hasToken());

  useEffect(() => onAuthChange(() => setAuthed(hasToken())), []);

  if (!authed) {
    return (
      <>
        <SnackbarHost />
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route path="/p/:tenant/:form" element={<PublicForm />} />
          <Route path="*" element={<Navigate to="/login" replace />} />
        </Routes>
      </>
    );
  }
  return (
    <>
      <SnackbarHost />
      <Shell />
    </>
  );
}

function Shell() {
  const [entities, setEntities] = useState<UiEntity[]>([]);
  const [config, setConfig] = useState<TenantConfig | null>(null);
  const [uiMeta, setUiMeta] = useState<{
    navigation: string[];
    hidden_entities: string[];
    default_dashboard?: string | null;
    workspaceNav: WorkspaceNavItem[];
  }>({ navigation: [], hidden_entities: [], workspaceNav: [] });
  const [userName, setUserName] = useState("");
  const [userEmail, setUserEmail] = useState("");
  const [roles, setRoles] = useState<string[]>([]);
  const [tenantKey, setTenantKey] = useState("local");
  const [userKey, setUserKey] = useState("anon");
  const [studio, setStudio] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    function loadUi() {
      api
        .ui()
        .then((d) => {
          setEntities(d.entities);
          setUiMeta({
            navigation: d.navigation ?? [],
            hidden_entities: d.hidden_entities ?? [],
            default_dashboard: d.default_dashboard,
            workspaceNav: d.workspace?.navigation ?? [],
          });
          if (d.branding) {
            setConfig((prev) =>
              prev
                ? { ...prev, branding: { ...prev.branding, ...d.branding } }
                : {
                    branding: d.branding ?? {},
                    ui_config: {
                      navigation: d.navigation ?? [],
                      hidden_entities: d.hidden_entities ?? [],
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
        setUserName(d.user.name);
        setUserEmail(d.user.email);
        setRoles(d.roles);
        setTenantKey(d.tenant_id || "local");
        setUserKey(d.user.email || "anon");
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
    else if (accent) root.style.setProperty("--primary", accent);
    const secondary = config?.branding.secondary_color;
    if (secondary) root.style.setProperty("--secondary", secondary);
    const name = config?.branding.company_name || config?.branding.app_name || "Workspace";
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

  const navEntities = useMemo(
    () => primaryNavEntities(entities, uiMeta.navigation, uiMeta.hidden_entities),
    [entities, uiMeta],
  );
  const resolvedConfig = useMemo(() => {
    if (!config) {
      return {
        branding: {},
        ui_config: {
          navigation: uiMeta.navigation,
          hidden_entities: uiMeta.hidden_entities,
          default_dashboard: uiMeta.default_dashboard,
        },
        enabled_apps: [],
      } as TenantConfig;
    }
    return {
      ...config,
      ui_config: {
        ...config.ui_config,
        navigation: uiMeta.navigation.length ? uiMeta.navigation : config.ui_config.navigation,
        hidden_entities: uiMeta.hidden_entities.length
          ? uiMeta.hidden_entities
          : config.ui_config.hidden_entities,
        default_dashboard: uiMeta.default_dashboard ?? config.ui_config.default_dashboard,
      },
    };
  }, [config, uiMeta]);

  const appName = config?.branding.company_name || config?.branding.app_name || "Workspace";
  const theme = {
    timezone: config?.business?.timezone || "UTC",
    locale: config?.business?.locale || "en-US",
    currency: config?.business?.currency || "USD",
  };

  const routes = (
    <Routes>
      <Route path="/" element={<Dashboard entities={entities} config={resolvedConfig} />} />
      <Route path="/login" element={<Navigate to="/" replace />} />
      <Route
        path="/settings"
        element={
          <Settings
            config={config}
            entities={entities}
            navSlugs={uiMeta.navigation}
            hiddenEntities={uiMeta.hidden_entities}
            roles={roles}
            onSaved={(next) => {
              setConfig(next);
              api
                .ui()
                .then((d) => {
                  setEntities(d.entities);
                  setUiMeta({
                    navigation: d.navigation ?? [],
                    hidden_entities: d.hidden_entities ?? [],
                    default_dashboard: d.default_dashboard,
                    workspaceNav: d.workspace?.navigation ?? [],
                  });
                })
                .catch(() => undefined);
            }}
          />
        }
      />
      <Route path="/settings/audit" element={<AuditLog />} />
      <Route path="/reports" element={<Reports />} />
      <Route path="/p/:tenant/:form" element={<PublicForm />} />
      <Route path="/:slug" element={<EntityList entities={entities} />} />
      <Route path="/:slug/new" element={<EntityForm entities={entities} />} />
      <Route path="/:slug/:id" element={<EntityDetail entities={entities} />} />
      <Route path="/:slug/:id/edit" element={<EntityForm entities={entities} />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );

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
      <PrefsProvider tenantId={tenantKey} userId={userKey}>
        <AppShell
          appName={appName}
          logo={config?.branding.logo}
          navEntities={navEntities}
          workspaceNav={uiMeta.workspaceNav}
          allEntities={entities}
          studio={studio}
          userName={userName}
          userEmail={userEmail}
          roles={roles}
        >
          {routes}
        </AppShell>
      </PrefsProvider>
    </TenantThemeContext.Provider>
  );
}
