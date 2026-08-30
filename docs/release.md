# Release process

V1.0 releases are reproducible enough to identify exactly what was shipped.

## Pipeline

```
test → lint → security audit → build → package → integration test → migration test → release
```

```bash
./scripts/setup-postgres.sh --check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro \
  cargo test --workspace -- --test-threads=1
cd frontend && npm test && cd ..
cargo audit || true
npm --prefix frontend audit || true
cargo build --workspace --release
cargo install --path crates/qefro-cli --locked --force
```

Record in the release notes:

- framework version (`1.2.0`)
- git commit
- Rustc version (`rustc -V`)
- Node version
- `Cargo.lock` and `frontend/package-lock.json`

App packages include `framework_version`, `metadata_schema`, and `ui_schema` in `qefro-package.json`.

## Artifacts

| Artifact | How |
| --- | --- |
| Qefro CLI / server | `qefro` binary (`qefro serve`, `qefro worker`) |
| Frontend | `frontend` production build |
| App packages | `qefro app package <name>` |
| Docker | `docker compose build` (see [deployment.md](deployment.md)) |

## Upgrade from 0.9

1. Backup PostgreSQL and `QEFRO_STORAGE_PATH`.
2. Deploy 1.0.0 binaries.
3. `qefro migrate`
4. `qefro doctor`
5. Smoke: login, list entities, Studio overview, a workflow action, a job, a webhook, SSE.

V0.9 data is compatible: schema apply is additive (`IF NOT EXISTS`). Failed app migrations stay `failed` and must be inspected before retry.
