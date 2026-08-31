# App packaging

A `.qefro` file is a ZIP of application definitions plus `qefro-package.json` (format, name, version, file list, SHA-256). It does not contain PostgreSQL data, JWT secrets, `.env` files, or credentials.

```bash
qefro app validate myshop
qefro app package myshop
# writes myshop-0.1.0.qefro
qefro app info myshop-0.1.0.qefro
qefro app install myshop-0.1.0.qefro
```

## Contents

Allowlisted paths only:

- `app.toml`, `README.md`, `runtime.toml`
- `entities/`, `workflows/`, `permissions/`
- `reports/`, `dashboards/`, `pages/`, `print_formats/`
- `seeds/`, `hooks/`, `migrations/`, `assets/`, `tools/`

Assets must be relative. `../` and absolute paths are rejected at pack and extract time. Extraction never writes outside `.qefro/store/<name>/`. Duplicate zip entries, oversized files (8 MiB each, 32 MiB total), and checksum mismatches fail the install.

The SHA-256 covers definition files in sorted path order. Signing (PKI) is not implemented in V0.7; the checksum is the hook for it.

## Builtin apps

`qefro app package restaurant` packages the catalog `app.toml`. Runtime behavior still comes from the compiled `qefro-restaurant` crate. YAML apps package their full definitions and run on any V0.7 CLI without compiling those entities into the framework.

## Integrity

Treat `.qefro` packages as untrusted input. `qefro app install` validates the archive, then the manifest, then dependencies, then writes the store directory. A failed validation deletes the partial extract and does not mark the app installed.
