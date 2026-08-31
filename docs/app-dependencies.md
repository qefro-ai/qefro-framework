# App dependencies

Keep the solver small. Each app lists named dependencies with semver requirements:

```toml
framework_version = ">=1.0,<2.0"

[dependencies]
inventory = ">=1.0,<2.0"
```

`core`, `qefro-framework`, `qefro`, and `framework` are the current runtime, not separately installed apps. They are checked against `FRAMEWORK_VERSION` (this crate, 1.3.x).

Other names must already be **installed** (not merely catalogued) at a matching version. Missing or incompatible versions fail `qefro app validate` and `qefro app install`.

Direct cycles (`A` depends on `B` while the current graph already lists `A`) are rejected. There is no marketplace, no transitive SAT solver, and no automatic download of dependencies.

The legacy key `depends_on = ["inventory", "qefro-framework"]` is merged into `[dependencies]` with requirement `*` for non-framework names.
