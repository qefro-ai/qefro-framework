# Theming

Tenant branding (logo, favicon, accent / primary / secondary) comes from tenant settings. The renderer sets CSS variables (`--accent`, `--primary`). **Arbitrary tenant CSS or JavaScript is rejected** — there is no injection surface.

User appearance (this device, scoped to tenant + user):

- theme: `light` | `dark` | `system`
- density: `comfortable` | `compact`
- sidebar collapsed

Compact density tightens tables, documents, and controls for ERP-style work. Dark theme swaps surfaces and status chips; accent still comes from the tenant.

Favicon and document title follow company / app name. Do not put secrets or tenant IDs in the chrome.
