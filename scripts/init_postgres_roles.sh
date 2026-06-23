#!/usr/bin/env bash
set -Eeuo pipefail

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "error: ${name} must be set for SchoolOrbit PostgreSQL initialization" >&2
    exit 1
  fi
}

require_env ADMIN_APP_PASSWORD
require_env TENANT_OWNER_PASSWORD

provisioner_role="${POSTGRES_USER:-schoolorbit_provisioner}"
if [[ -z "${provisioner_role}" ]]; then
  echo "error: POSTGRES_USER must not be empty" >&2
  exit 1
fi

echo "Initializing SchoolOrbit PostgreSQL roles and admin database"

psql \
  --username "${provisioner_role}" \
  --dbname postgres \
  -v ON_ERROR_STOP=1 \
  -v provisioner_role="${provisioner_role}" \
  -v admin_app_password="${ADMIN_APP_PASSWORD}" \
  -v tenant_owner_password="${TENANT_OWNER_PASSWORD}" <<'SQL'
ALTER ROLE :"provisioner_role" WITH LOGIN CREATEDB;

SELECT format('CREATE ROLE schoolorbit_admin_app LOGIN PASSWORD %L', :'admin_app_password')
WHERE NOT EXISTS (
    SELECT 1 FROM pg_roles WHERE rolname = 'schoolorbit_admin_app'
);
\gexec

ALTER ROLE schoolorbit_admin_app WITH LOGIN PASSWORD :'admin_app_password';

SELECT format('CREATE ROLE schoolorbit_tenant_owner LOGIN PASSWORD %L', :'tenant_owner_password')
WHERE NOT EXISTS (
    SELECT 1 FROM pg_roles WHERE rolname = 'schoolorbit_tenant_owner'
);
\gexec

ALTER ROLE schoolorbit_tenant_owner WITH LOGIN PASSWORD :'tenant_owner_password';

GRANT schoolorbit_tenant_owner TO :"provisioner_role";

SELECT 'CREATE DATABASE schoolorbit_admin OWNER schoolorbit_admin_app'
WHERE NOT EXISTS (
    SELECT 1 FROM pg_database WHERE datname = 'schoolorbit_admin'
);
\gexec

ALTER DATABASE schoolorbit_admin OWNER TO schoolorbit_admin_app;
GRANT CONNECT ON DATABASE schoolorbit_admin TO schoolorbit_admin_app;
GRANT CONNECT ON DATABASE postgres TO :"provisioner_role";
SQL

echo "SchoolOrbit PostgreSQL roles and admin database initialized"
