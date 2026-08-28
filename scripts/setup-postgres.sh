#!/usr/bin/env bash
# Create or verify the dedicated Qefro Postgres role and database.
# Preferred: `docker compose up -d postgres` (see docker-compose.yml).
# Fallback: local Postgres on 5432 (Homebrew, Postgres.app, etc.).
#
# The compose POSTGRES_USER is a superuser. The local fallback matches that so
# schema apply can CREATE in `public` (PostgreSQL 15+ revokes CREATE from PUBLIC).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
URL="${DATABASE_URL:-postgres://qefro:qefro@127.0.0.1:5432/qefro}"
ADMIN_URL="${QEFRO_PG_ADMIN_URL:-postgres://${USER}@127.0.0.1:5432/postgres}"

redact() {
  echo "$1" | sed -E 's#://([^:/]+):([^@]+)@#://\1:***@#'
}

can_login() {
  psql "$URL" -c 'SELECT 1' >/dev/null 2>&1
}

can_ddl() {
  psql "$URL" -v ON_ERROR_STOP=1 -c 'CREATE TABLE IF NOT EXISTS _qefro_setup_check(id int); DROP TABLE _qefro_setup_check;' >/dev/null 2>&1
}

if [[ "${1:-}" == "--check" ]]; then
  if can_login && can_ddl; then
    echo "PostgreSQL ready ($(redact "$URL"))"
    exit 0
  fi
  echo "PostgreSQL not ready at $(redact "$URL")" >&2
  echo "Run: docker compose up -d postgres" >&2
  echo "  or: scripts/setup-postgres.sh" >&2
  exit 1
fi

if can_login && can_ddl; then
  echo "PostgreSQL ready ($(redact "$URL"))"
  exit 0
fi

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  echo "Starting docker compose postgres…"
  (cd "$ROOT" && docker compose up -d postgres)
  for _ in $(seq 1 30); do
    if can_login && can_ddl; then
      echo "PostgreSQL ready ($(redact "$URL"))"
      exit 0
    fi
    sleep 1
  done
  echo "docker compose postgres did not become ready for DDL on 5432." >&2
  echo "If another Postgres already owns that port, the local fallback runs next." >&2
fi

echo "Ensuring role qefro and database qefro on local Postgres…"
echo "Admin URL: $(redact "$ADMIN_URL")"
if ! psql "$ADMIN_URL" -c 'SELECT 1' >/dev/null 2>&1; then
  echo "Cannot connect as admin ($(redact "$ADMIN_URL"))." >&2
  echo "Set QEFRO_PG_ADMIN_URL to a superuser URL, or start Docker:" >&2
  echo "  docker compose up -d postgres" >&2
  exit 1
fi

psql "$ADMIN_URL" -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'qefro') THEN
    CREATE ROLE qefro LOGIN PASSWORD 'qefro' SUPERUSER CREATEDB;
  ELSE
    ALTER ROLE qefro WITH LOGIN SUPERUSER CREATEDB;
  END IF;
END
$$;
SQL

if ! psql "$ADMIN_URL" -tAc "SELECT 1 FROM pg_database WHERE datname = 'qefro'" | grep -q 1; then
  psql "$ADMIN_URL" -v ON_ERROR_STOP=1 -c "CREATE DATABASE qefro OWNER qefro;"
else
  psql "$ADMIN_URL" -v ON_ERROR_STOP=1 -c "ALTER DATABASE qefro OWNER TO qefro;"
fi

psql "$ADMIN_URL" -d qefro -v ON_ERROR_STOP=1 <<'SQL'
GRANT ALL ON SCHEMA public TO qefro;
ALTER SCHEMA public OWNER TO qefro;
SQL

if can_login && can_ddl; then
  echo "PostgreSQL ready ($(redact "$URL"))"
  echo "export DATABASE_URL=$URL"
  exit 0
fi

echo "Role/database exist but qefro still cannot CREATE in public." >&2
echo "Check pg_hba.conf allows password auth on 127.0.0.1." >&2
exit 1
