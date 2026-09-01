# Theming

Tenant branding (logo, favicon, accent / primary / secondary) comes from tenant settings. Empty fields are filled from the enabled app’s default branding (`AppModule` / `[branding]` in `app.toml`), then the tenant name. The renderer sets CSS variables (`--accent`, `--primary`, `--secondary`). Applications can set defaults with `qefro.theme({ primary, radius, density })`; tenant branding still wins. **Arbitrary tenant CSS or JavaScript is rejected** — there is no injection surface. See [qefro.js](qefro-js.md).

User appearance (this device, scoped to tenant + user):

- theme: `light` | `dark` | `system`
- density: `comfortable` | `compact`
- sidebar collapsed

Compact density tightens tables, documents, and controls for ERP-style work. Dark theme swaps surfaces and status chips; accent still comes from the tenant.

Favicon and document title follow company / app name. Do not put secrets or tenant IDs in the chrome.
