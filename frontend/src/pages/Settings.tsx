import { FormEvent, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { api, ApiError, type TenantConfig, type UiEntity } from "../api";
import { settingsEntities } from "../metadata/navigation";
import { usePrefsOptional } from "../prefsContext";
import { PageHeader } from "../components/ui/PageHeader";

export default function Settings({
  config,
  entities = [],
  navSlugs = [],
  hiddenEntities = [],
  roles = [],
  onSaved,
}: {
  config: TenantConfig | null;
  entities?: UiEntity[];
  navSlugs?: string[];
  hiddenEntities?: string[];
  roles?: string[];
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
  const setup = useMemo(
    () => settingsEntities(entities, navSlugs, hiddenEntities),
    [entities, navSlugs, hiddenEntities],
  );
  const singletons = setup.filter((e) => e.singleton);
  const collections = setup.filter((e) => !e.singleton);

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
      {setup.length > 0 ? (
        <>
          <PageHeader
            kicker="Application"
            title="Setup"
            description="Configuration and setup records. Day-to-day work stays in the main menu."
          />
          <div className="cards">
            {singletons.map((entity) => (
              <Link key={entity.slug} className="card" to={`/${entity.slug}`}>
                <div className="muted">Configuration</div>
                <strong>{entity.label}</strong>
                {entity.description ? <p className="muted">{entity.description}</p> : null}
              </Link>
            ))}
            {collections.map((entity) => (
              <Link key={entity.slug} className="card" to={`/${entity.slug}`}>
                <div className="muted">{entity.module || "Setup"}</div>
                <strong>{entity.label_plural}</strong>
                {entity.description ? <p className="muted">{entity.description}</p> : null}
              </Link>
            ))}
          </div>
        </>
      ) : null}
      {roles.some((r) => r.toLowerCase() === "admin") ? (
        <p>
          <Link to="/settings/audit">Audit log</Link>
          <span className="muted"> — administrators only</span>
        </p>
      ) : null}
      <PageHeader
        kicker="Administration"
        title="Workspace settings"
        description="Branding, navigation, applications, and locale apply to this tenant only. The server enforces enabled apps and permissions — hiding a nav item is not a security control."
      />
      <form className="form form-wide" onSubmit={onSubmit}>
        {prefs ? (
          <fieldset>
            <legend>Appearance</legend>
            <div className="form-grid">
            <label className="field-cell width-half">
              Theme
              <select value={prefs.prefs.theme} onChange={(e) => prefs.setTheme(e.target.value as "light" | "dark" | "system")}>
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
            <label className="field-cell width-half">
              Density
              <select
                value={prefs.prefs.density}
                onChange={(e) => prefs.setDensity(e.target.value as "comfortable" | "compact")}
              >
                <option value="comfortable">Comfortable</option>
                <option value="compact">Compact</option>
              </select>
            </label>
            <p className="muted field-cell field-span-2">Saved on this device for the signed-in user. Accent color still comes from tenant branding.</p>
            </div>
          </fieldset>
        ) : null}
        <fieldset>
          <legend>Branding</legend>
          <div className="form-grid">
        <label className="field-cell width-half">
          Company name
          <input value={companyName} onChange={(e) => setCompanyName(e.target.value)} />
        </label>
        <label className="field-cell width-half">
          App name
          <input value={appName} onChange={(e) => setAppName(e.target.value)} />
        </label>
        <label className="field-cell width-half">
          Primary color
          <input value={primary} onChange={(e) => setPrimary(e.target.value)} placeholder="#9a3412" />
        </label>
        <label className="field-cell width-half">
          Secondary color
          <input value={secondary} onChange={(e) => setSecondary(e.target.value)} placeholder="#f4f1ea" />
        </label>
        <label className="field-cell width-half">
          Accent color
          <input value={accent} onChange={(e) => setAccent(e.target.value)} placeholder="#9a3412" />
        </label>
        <label className="field-cell width-half">
          Logo URL
          <input value={logo} onChange={(e) => setLogo(e.target.value)} />
        </label>
        <label className="field-cell width-half">
          Favicon URL
          <input value={favicon} onChange={(e) => setFavicon(e.target.value)} />
        </label>
          </div>
        </fieldset>
        <fieldset>
          <legend>Navigation and apps</legend>
          <div className="form-grid">
        <label className="field-cell width-half">
          Navigation (entity slugs, comma-separated)
          <input
            value={navigation}
            onChange={(e) => setNavigation(e.target.value)}
            placeholder="customers, reservations, orders"
          />
        </label>
        <label className="field-cell width-half">
          Enabled applications (comma-separated)
          <input
            value={apps}
            onChange={(e) => setApps(e.target.value)}
            placeholder="restaurant, crm"
          />
        </label>
          </div>
        </fieldset>
        <fieldset>
          <legend>Locale</legend>
          <div className="form-grid">
        <label className="field-cell width-half">
          Timezone
          <input value={timezone} onChange={(e) => setTimezone(e.target.value)} placeholder="UTC" />
        </label>
        <label className="field-cell width-half">
          Locale
          <input value={locale} onChange={(e) => setLocale(e.target.value)} placeholder="en-US" />
        </label>
        <label className="field-cell width-half">
          Currency
          <input value={currency} onChange={(e) => setCurrency(e.target.value)} placeholder="USD" />
        </label>
        <label className="field-cell width-half">
          Date format
          <input value={dateFormat} onChange={(e) => setDateFormat(e.target.value)} />
        </label>
        <label className="field-cell field-span-2">
          Terminology (Entity=Label, one per line)
          <textarea
            value={terminology}
            onChange={(e) => setTerminology(e.target.value)}
            placeholder={"Customer=Guest\nReservation=Booking"}
            rows={4}
          />
        </label>
          </div>
        </fieldset>
        {error && <p className="error">{error}</p>}
        {ok && <p className="ok">{ok}</p>}
        <div className="form-actions actions">
          <button type="submit">Save settings</button>
        </div>
      </form>
    </div>
  );
}
