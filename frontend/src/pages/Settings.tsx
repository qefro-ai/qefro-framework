import { FormEvent, useEffect, useState } from "react";
import { api, ApiError, type TenantConfig } from "../api";

export default function Settings({
  config,
  onSaved,
}: {
  config: TenantConfig | null;
  onSaved: (next: TenantConfig) => void;
}) {
  const [appName, setAppName] = useState("");
  const [primary, setPrimary] = useState("");
  const [logo, setLogo] = useState("");
  const [navigation, setNavigation] = useState("");
  const [error, setError] = useState("");
  const [ok, setOk] = useState("");

  useEffect(() => {
    if (!config) return;
    setAppName(config.branding.app_name ?? "");
    setPrimary(config.branding.primary_color ?? "");
    setLogo(config.branding.logo ?? "");
    setNavigation((config.ui_config.navigation ?? []).join(", "));
  }, [config]);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!config) return;
    setError("");
    setOk("");
    const next: TenantConfig = {
      ...config,
      branding: {
        ...config.branding,
        app_name: appName || null,
        primary_color: primary || null,
        logo: logo || null,
      },
      ui_config: {
        ...config.ui_config,
        navigation: navigation
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
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
    <div>
      <div className="badge">Tenant</div>
      <h2>Branding & navigation</h2>
      <p className="muted">
        These settings apply to this tenant only. The frontend is not the authority for permissions
        or workflows.
      </p>
      <form className="form" onSubmit={onSubmit}>
        <label>
          App name
          <input value={appName} onChange={(e) => setAppName(e.target.value)} />
        </label>
        <label>
          Primary color
          <input value={primary} onChange={(e) => setPrimary(e.target.value)} placeholder="#9a3412" />
        </label>
        <label>
          Logo URL
          <input value={logo} onChange={(e) => setLogo(e.target.value)} />
        </label>
        <label>
          Navigation (entity slugs, comma-separated)
          <input
            value={navigation}
            onChange={(e) => setNavigation(e.target.value)}
            placeholder="customers, reservations, orders"
          />
        </label>
        {error && <p className="error">{error}</p>}
        {ok && <p className="muted">{ok}</p>}
        <button type="submit">Save</button>
      </form>
    </div>
  );
}
