import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  applyChrome,
  loadPrefs,
  resolvedTheme,
  savePrefs,
  type Density,
  type TablePrefs,
  type ThemeMode,
  type UserPrefs,
} from "./prefs";

type PrefsApi = {
  prefs: UserPrefs;
  theme: "light" | "dark";
  setTheme: (theme: ThemeMode) => void;
  setDensity: (density: Density) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  tablePrefs: (slug: string) => TablePrefs;
  setTablePrefs: (slug: string, patch: TablePrefs) => void;
};

const PrefsContext = createContext<PrefsApi | null>(null);

export function PrefsProvider({
  tenantId,
  userId,
  children,
}: {
  tenantId: string;
  userId: string;
  children: ReactNode;
}) {
  const [prefs, setPrefs] = useState<UserPrefs>(() => loadPrefs(tenantId, userId));

  useEffect(() => {
    setPrefs(loadPrefs(tenantId, userId));
  }, [tenantId, userId]);

  useEffect(() => {
    applyChrome(prefs);
    if (tenantId && userId) savePrefs(tenantId, userId, prefs);
  }, [prefs, tenantId, userId]);

  useEffect(() => {
    if (prefs.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyChrome(prefs);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [prefs]);

  const api = useMemo<PrefsApi>(
    () => ({
      prefs,
      theme: resolvedTheme(prefs.theme),
      setTheme: (theme) => setPrefs((p) => ({ ...p, theme })),
      setDensity: (density) => setPrefs((p) => ({ ...p, density })),
      setSidebarCollapsed: (sidebarCollapsed) => setPrefs((p) => ({ ...p, sidebarCollapsed })),
      tablePrefs: (slug) => prefs.tables[slug] ?? {},
      setTablePrefs: (slug, patch) =>
        setPrefs((p) => ({
          ...p,
          tables: { ...p.tables, [slug]: { ...p.tables[slug], ...patch } },
        })),
    }),
    [prefs],
  );

  return <PrefsContext.Provider value={api}>{children}</PrefsContext.Provider>;
}

export function usePrefs() {
  const ctx = useContext(PrefsContext);
  if (!ctx) {
    throw new Error("usePrefs must be used within PrefsProvider");
  }
  return ctx;
}

export function usePrefsOptional() {
  return useContext(PrefsContext);
}
