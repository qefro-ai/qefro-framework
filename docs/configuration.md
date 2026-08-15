# Configuration

Install the CLI first so `qefro` is on your PATH:

```bash
cargo install --path crates/qefro-cli --locked --force
qefro --help
```

Qefro reads process configuration from the environment. Copy `.env.example` to `.env` locally. Never commit secrets.

## Required in production

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | PostgreSQL connection string |
| `JWT_SECRET` | Session signing key (must not be the development default) |

## Optional

| Variable | Default | Purpose |
| --- | --- | --- |
| `QEFRO_BIND` / `QEFRO_BIND_ADDRESS` | `127.0.0.1:8080` | HTTP listen address |
| `QEFRO_ENV` | `development` | `development` or `production` |
| `QEFRO_PUBLIC_URL` | `http://127.0.0.1:8080` | Public origin for links |
| `QEFRO_LOG_LEVEL` | `info` | Used when `RUST_LOG` is unset |
| `QEFRO_STORAGE_PATH` | `./var/qefro-storage` | Local tenant blob root |
| `QEFRO_AUTO_MIGRATE` | `true` unless `QEFRO_ENV=production` | Apply schema on process start |
| `QEFRO_EMBED_WORKER` | `true` unless `QEFRO_ENV=production` | HTTP process also polls jobs |
| `RUST_LOG` | (from `QEFRO_LOG_LEVEL`) | tracing filter |

| `QEFRO_DB_MAX_CONNECTIONS` | `10` (clamped 2–100) | PostgreSQL pool size |
| `QEFRO_DB_ACQUIRE_TIMEOUT_SECS` | `10` | Pool acquire timeout |

Precedence: **environment > defaults**. There is no required config file in V1.0. Invalid production configuration (`JWT_SECRET` still the development default, unparseable `QEFRO_BIND`) fails at `qefro serve` / `qefro worker` startup.

`qefro migrate` always applies schema, even when `QEFRO_AUTO_MIGRATE=false`.

Production boot:

```bash
export QEFRO_ENV=production
export QEFRO_AUTO_MIGRATE=false
export QEFRO_EMBED_WORKER=false
qefro migrate
qefro serve
# another process:
qefro worker
```

Do not put `DATABASE_URL` or `JWT_SECRET` in source, Docker layers, or logs.
