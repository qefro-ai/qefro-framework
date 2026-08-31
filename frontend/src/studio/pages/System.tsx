import { useEffect, useState } from "react";
import { api, type TenantConfig } from "../../api";
import { can } from "../StudioApp";

export default function System({ caps }: { caps: string[] }) {
  const [tenant, setTenant] = useState<Record<string, unknown> | null>(null);
  const [config, setConfig] = useState<TenantConfig | null>(null);
  const [nav, setNav] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => {
    api.studioTenant().then(setTenant);
    api.tenantConfig().then((c) => {
      setConfig(c);
      setNav((c.ui_config.navigation ?? []).join("\n"));
    });
  }, []);

  async function saveTenant() {
    if (!config) return;
    const next = {
      ...config,
      ui_config: {
        ...config.ui_config,
        navigation: nav
          .split("\n")
          .map((s) => s.trim())
          .filter(Boolean),
      },
    };
    const saved = await api.saveTenantConfig(next);
    setConfig(saved);
    setMessage("Tenant configuration saved.");
  }

  return (
    <div className="page">
      <h2>System</h2>
      <section className="card">
        <h3>Platform vs tenant</h3>
        <p>
          Platform Studio manages apps and global metadata. Tenant Studio below only changes this
          tenant’s branding, navigation, terminology, locale, and enabled apps.
        </p>
        <p className="muted">Runtime env: {String(tenant && (tenant as { config?: unknown }) ? "" : "")}</p>
      </section>
      {config ? (
        <form
          className="form"
          onSubmit={(e) => {
            e.preventDefault();
            saveTenant().catch((err) => setMessage(err.message));
          }}
        >
          <h3>Tenant</h3>
          <label>
            Company name
            <input
              value={config.branding.company_name ?? ""}
              onChange={(e) =>
                setConfig({ ...config, branding: { ...config.branding, company_name: e.target.value } })
              }
            />
          </label>
          <label>
            Primary color
            <input
              value={config.branding.primary_color ?? ""}
              onChange={(e) =>
                setConfig({ ...config, branding: { ...config.branding, primary_color: e.target.value } })
              }
            />
          </label>
          <label>
            Terminology (Customer → Guest)
            <input
              value={config.ui_config.terminology?.Customer ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  ui_config: {
                    ...config.ui_config,
                    terminology: { ...config.ui_config.terminology, Customer: e.target.value },
                  },
                })
              }
            />
          </label>
          <label>
            Navigation (one slug per line)
            <textarea value={nav} onChange={(e) => setNav(e.target.value)} />
          </label>
          <label>
            Locale
            <input
              value={config.business?.locale ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, locale: e.target.value },
                })
              }
            />
          </label>
          <label>
            Currency
            <input
              value={config.business?.currency ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, currency: e.target.value },
                })
              }
            />
          </label>
          <label>
            Cash account code
            <input
              value={config.business?.cash_account ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, cash_account: e.target.value },
                })
              }
              placeholder="1100"
            />
          </label>
          <label>
            Receivable account code
            <input
              value={config.business?.receivable_account ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, receivable_account: e.target.value },
                })
              }
              placeholder="1200"
            />
          </label>
          <label>
            Sales account code
            <input
              value={config.business?.sales_account ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, sales_account: e.target.value },
                })
              }
              placeholder="4100"
            />
          </label>
          <label>
            Timezone
            <input
              value={config.business?.timezone ?? ""}
              onChange={(e) =>
                setConfig({
                  ...config,
                  business: { ...config.business, timezone: e.target.value },
                })
              }
            />
          </label>
          <button type="submit" disabled={!can(caps, "studio.edit") && !can(caps, "studio.view")}>
            Save tenant settings
          </button>
        </form>
      ) : null}
      {message ? <p role="status">{message}</p> : null}
    </div>
  );
}
