import { FormEvent, useEffect, useState } from "react";
import { api, ApiError, type TenantConfig } from "../api";
import { usePrefsOptional } from "../prefsContext";

export default function Settings({
  config,
  onSaved,
}: {
  config: TenantConfig | null;
  onSaved: (next: TenantConfig) => void;
}) {
  const [companyName, setCompanyName] = useState("");
  const [appName, setAppName] = useState("");
  const [primary, setPrimary] = useState("");
  const [secondary, setSecondary] = useState("");
  const [accent, setAccent] = useState("");
  const [logo, setLogo] = useState("");
  const [favicon, setFavicon] = useState("");
  const [navigation, setNavigation] = useState("");
  const [apps, setApps] = useState("");
  const [timezone, setTimezone] = useState("");
  const [locale, setLocale] = useState("");
  const [currency, setCurrency] = useState("");
  const [dateFormat, setDateFormat] = useState("");
  const [terminology, setTerminology] = useState("");
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");
  const prefs = usePrefsOptional();

  useEffect(() => {
    if (!config) return;
    setCompanyName(config.branding.company_name ?? "");
    setAppName(config.branding.app_name ?? "");
    setPrimary(config.branding.primary_color ?? "");
    setSecondary(config.branding.secondary_color ?? "");
    setAccent(config.branding.accent_color ?? "");
    setLogo(config.branding.logo ?? "");
    setFavicon(config.branding.favicon ?? "");
    setNavigation((config.ui_config.navigation ?? []).join(", "));
    setApps((config.enabled_apps ?? []).join(", "));
    setTimezone(config.business?.timezone ?? "UTC");
    setLocale(config.business?.locale ?? "en-US");
    setCurrency(config.business?.currency ?? "USD");
    setDateFormat(config.business?.date_format ?? "YYYY-MM-DD");
    const terms = config.ui_config.terminology ?? {};
    setTerminology(
      Object.entries(terms)
        .map(([k, v]) => `${k}=${v}`)
        .join("\n"),
    );
  }, [config]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!config) return;
    setError("");
    setOk("");
    const terms: Record<string, string> = {};
    for (const line of terminology.split("\n")) {
      const idx = line.indexOf("=");
      if (idx <= 0) continue;
      terms[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
    }
    const next: TenantConfig = {
      ...config,
      branding: {
        ...config.branding,
        company_name: companyName || null,
        app_name: appName || null,
        primary_color: primary || null,
        secondary_color: secondary || null,
        accent_color: accent || null,
        logo: logo || null,
        favicon: favicon || null,
      },
      ui_config: {
        ...config.ui_config,
        navigation: navigation
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        terminology: terms,
      },
      enabled_apps: apps
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      business: {
        timezone: timezone || "UTC",
        locale: locale || "en-US",
        currency: currency || "USD",
        date_format: dateFormat || "YYYY-MM-DD",
        number_format: config.business?.number_format ?? "1,234.56",
      },
    };
    try {
      const saved = await api.saveTenantConfig(next);
      onSaved(saved);
      setOk("Saved");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "failed");
    }
  }

  return (
    <div className="page">
      <div className="badge">Tenant</div>
      <h2>Workspace settings</h2>
      <p className="muted">
        Branding, navigation, applications, and locale apply to this tenant only. The server
        enforces enabled apps and permissions — hiding a nav item is not a security control.
      </p>
      <form className="form" onSubmit={onSubmit}>
        {prefs ? (
          <fieldset>
            <legend>Appearance</legend>
            <label>
              Theme
              <select value={prefs.prefs.theme} onChange={(e) => prefs.setTheme(e.target.value as "light" | "dark" | "system")}>
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
            <label>
              Density
              <select
                value={prefs.prefs.density}
                onChange={(e) => prefs.setDensity(e.target.value as "comfortable" | "compact")}
              >
                <option value="comfortable">Comfortable</option>
                <option value="compact">Compact</option>
              </select>
            </label>
            <p className="muted">Saved on this device for the signed-in user. Accent color still comes from tenant branding.</p>
          </fieldset>
        ) : null}
        <fieldset>
          <legend>Branding</legend>
        <label>
          Company name
          <input value={companyName} onChange={(e) => setCompanyName(e.target.value)} />
        </label>
        <label>
          App name
          <input value={appName} onChange={(e) => setAppName(e.target.value)} />
        </label>
        <label>
          Primary color
          <input value={primary} onChange={(e) => setPrimary(e.target.value)} placeholder="#9a3412" />
        </label>
        <label>
          Secondary color
          <input value={secondary} onChange={(e) => setSecondary(e.target.value)} placeholder="#f4f1ea" />
        </label>
        <label>
          Accent color
          <input value={accent} onChange={(e) => setAccent(e.target.value)} placeholder="#9a3412" />
        </label>
        <label>
          Logo URL
          <input value={logo} onChange={(e) => setLogo(e.target.value)} />
        </label>
        <label>
          Favicon URL
          <input value={favicon} onChange={(e) => setFavicon(e.target.value)} />
        </label>
        </fieldset>
        <fieldset>
          <legend>Navigation and apps</legend>
        <label>
          Navigation (entity slugs, comma-separated)
          <input
            value={navigation}
            onChange={(e) => setNavigation(e.target.value)}
            placeholder="customers, reservations, orders"
          />
        </label>
        <label>
          Enabled applications (comma-separated)
          <input
            value={apps}
            onChange={(e) => setApps(e.target.value)}
            placeholder="restaurant, crm"
          />
        </label>
        </fieldset>
        <fieldset>
          <legend>Locale</legend>
        <label>
          Timezone
          <input value={timezone} onChange={(e) => setTimezone(e.target.value)} placeholder="UTC" />
        </label>
        <label>
          Locale
          <input value={locale} onChange={(e) => setLocale(e.target.value)} placeholder="en-US" />
        </label>
        <label>
          Currency
          <input value={currency} onChange={(e) => setCurrency(e.target.value)} placeholder="USD" />
        </label>
        <label>
          Date format
          <input value={dateFormat} onChange={(e) => setDateFormat(e.target.value)} />
        </label>
        <label>
          Terminology (Entity=Label, one per line)
          <textarea
            value={terminology}
            onChange={(e) => setTerminology(e.target.value)}
            placeholder={"Customer=Guest\nReservation=Booking"}
            rows={4}
          />
        </label>
        </fieldset>
        {error && <p className="error">{error}</p>}
        {ok && <p className="ok">{ok}</p>}
        <button type="submit">Save settings</button>
      </form>
    </div>
  );
}
