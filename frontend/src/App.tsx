import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import StudioApp from "./studio/StudioApp";
import {
  api,
  ApiError,
  AppShell,
  applyBranding,
  clearToken,
  defaultExtensions,
  emitUiEvent,
  hasToken,
  METADATA_EVENT,
  onAuthChange,
  PrefsProvider,
  primaryNavEntities,
  Qefro,
  QefroProvider,
  QefroPublicRoutes,
  QefroRoutes,
  SnackbarHost,
  TenantThemeContext,
  type TenantConfig,
  type UiEntity,
  type WorkspaceNavItem,
} from "@qefro/js";

export const qefro = new Qefro({ apiUrl: "/api/v1" });
void qefro.init();

export default function App() {
  const [authed, setAuthed] = useState(hasToken());

  useEffect(() => onAuthChange(() => setAuthed(hasToken())), []);

  if (!authed) {
    return (
      <>
        <SnackbarHost />
        <QefroPublicRoutes />
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
    workspaceShortcuts: Array<{ label: string; to: string; entity?: string; kind?: string }>;
  }>({ navigation: [], hidden_entities: [], workspaceNav: [], workspaceShortcuts: [] });
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
            workspaceShortcuts: d.workspace?.shortcuts ?? [],
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
          emitUiEvent("workspace:ready", { entities: d.entities.length });
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
    applyBranding(config, qefro.getTheme());
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

  const snapshot = {
    entities,
    config: resolvedConfig,
    navigation: uiMeta.navigation,
    hiddenEntities: uiMeta.hidden_entities,
    workspaceNav: uiMeta.workspaceNav,
    workspaceShortcuts: uiMeta.workspaceShortcuts,
    userName,
    userEmail,
    roles,
    studio,
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
      <QefroProvider runtime={qefro} snapshot={snapshot}>
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
            extraNav={defaultExtensions.navigation}
          >
            <QefroRoutes
              entities={entities}
              config={resolvedConfig}
              shortcuts={uiMeta.workspaceShortcuts}
              navSlugs={uiMeta.navigation}
              hiddenEntities={uiMeta.hidden_entities}
              roles={roles}
              onConfigSaved={(next) => {
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
                      workspaceShortcuts: d.workspace?.shortcuts ?? [],
                    });
                  })
                  .catch(() => undefined);
              }}
            />
          </AppShell>
        </PrefsProvider>
      </QefroProvider>
    </TenantThemeContext.Provider>
  );
}
