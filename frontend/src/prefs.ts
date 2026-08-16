export type ThemeMode = "light" | "dark" | "system";
export type Density = "comfortable" | "compact";

export type TablePrefs = {
  columns?: string[];
  pageSize?: number;
  sort?: string;
  view?: string;
};

export type UserPrefs = {
  theme: ThemeMode;
  density: Density;
  sidebarCollapsed: boolean;
  tables: Record<string, TablePrefs>;
};

const DEFAULTS: UserPrefs = {
  theme: "system",
  density: "comfortable",
  sidebarCollapsed: false,
  tables: {},
};

function key(tenantId: string, userId: string) {
  return `qefro.prefs.${tenantId}.${userId}`;
}

export function loadPrefs(tenantId: string, userId: string): UserPrefs {
  try {
    const raw = localStorage.getItem(key(tenantId, userId));
    if (!raw) return { ...DEFAULTS, tables: {} };
    const parsed = JSON.parse(raw) as Partial<UserPrefs>;
    return {
      theme: parsed.theme ?? DEFAULTS.theme,
      density: parsed.density ?? DEFAULTS.density,
      sidebarCollapsed: Boolean(parsed.sidebarCollapsed),
      tables: parsed.tables ?? {},
    };
  } catch {
    return { ...DEFAULTS, tables: {} };
  }
}

export function savePrefs(tenantId: string, userId: string, prefs: UserPrefs) {
  localStorage.setItem(key(tenantId, userId), JSON.stringify(prefs));
}

export function resolvedTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

export function applyChrome(prefs: UserPrefs) {
  const root = document.documentElement;
  root.dataset.theme = resolvedTheme(prefs.theme);
  root.dataset.density = prefs.density;
  root.dataset.sidebar = prefs.sidebarCollapsed ? "collapsed" : "open";
}
