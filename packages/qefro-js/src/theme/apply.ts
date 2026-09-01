import type { TenantConfig } from "../sdk/client";
import type { ThemeConfig, ThemeRadius } from "../core/extensions";

const RADIUS: Record<string, string> = {
  small: "4px",
  medium: "8px",
  large: "12px",
};

function radiusValue(radius: ThemeRadius | undefined): string | undefined {
  if (radius == null) return undefined;
  if (typeof radius === "string" && RADIUS[radius]) return RADIUS[radius];
  return String(radius);
}

/** Application theme defaults. Tenant branding overrides these when present. */
export function applyTheme(theme: ThemeConfig | undefined | null) {
  if (!theme || typeof document === "undefined") return;
  const root = document.documentElement;
  if (theme.primary) root.style.setProperty("--primary", theme.primary);
  if (theme.accent || theme.primary) {
    const accent = theme.accent || theme.primary;
    if (accent) {
      root.style.setProperty("--accent", accent);
      if (!theme.primary) root.style.setProperty("--primary", accent);
    }
  }
  if (theme.secondary) root.style.setProperty("--secondary", theme.secondary);
  const radius = radiusValue(theme.radius);
  if (radius) {
    root.style.setProperty("--radius", radius);
    root.style.setProperty("--control-radius", radius);
    root.style.setProperty("--md-shape-md", radius);
    root.style.setProperty("--qefro-shape-md", radius);
  }
  if (theme.fontFamily) root.style.setProperty("--font", theme.fontFamily);
  if (theme.density) root.dataset.density = theme.density;
  if (theme.mode && theme.mode !== "system") root.dataset.theme = theme.mode;
  if (theme.favicon) {
    let link = document.querySelector("link[rel='icon']") as HTMLLinkElement | null;
    if (!link) {
      link = document.createElement("link");
      link.rel = "icon";
      document.head.appendChild(link);
    }
    link.href = theme.favicon;
  }
}

/**
 * Tenant branding from `/api/v1/meta/ui` and `/api/v1/tenants/me/config`.
 * Wins over application `qefro.theme()` defaults. Does not inject CSS or JS.
 */
export function applyBranding(config: TenantConfig | null | undefined, theme?: ThemeConfig | null) {
  if (typeof document === "undefined") return;
  applyTheme(theme);
  const root = document.documentElement;
  const primary = config?.branding.primary_color;
  const accent = config?.branding.accent_color || primary || theme?.accent || theme?.primary;
  if (accent) root.style.setProperty("--accent", accent);
  if (primary) root.style.setProperty("--primary", primary);
  else if (accent) root.style.setProperty("--primary", accent);
  const secondary = config?.branding.secondary_color;
  if (secondary) root.style.setProperty("--secondary", secondary);
  const name = config?.branding.company_name || config?.branding.app_name || "Workspace";
  document.title = name;
  const favicon = config?.branding.favicon || theme?.favicon;
  if (favicon) {
    let link = document.querySelector("link[rel='icon']") as HTMLLinkElement | null;
    if (!link) {
      link = document.createElement("link");
      link.rel = "icon";
      document.head.appendChild(link);
    }
    link.href = favicon;
  }
}
