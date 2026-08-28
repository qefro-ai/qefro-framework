# qefro-cli

Command-line interface for [Qefro Framework](https://github.com/qefro-ai/qefro-framework). Installs the `qefro` binary.

Walkthrough: [Create an application](https://github.com/qefro-ai/qefro-framework/blob/main/docs/creating-an-app.md), [Build a fullstack application](https://github.com/qefro-ai/qefro-framework/blob/main/docs/fullstack.md).

```bash
cargo install qefro-cli
qefro --help
```

macOS 26/27 workaround if `sqlx` fails with `mis-aligned LINKEDIT string pool`:

```bash
CARGO_PROFILE_RELEASE_STRIP=none cargo install qefro-cli
```

```bash
export DATABASE_URL=postgres://qefro:qefro@127.0.0.1:5432/qefro
qefro dev --app restaurant
```
