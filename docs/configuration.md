# Configuration

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
