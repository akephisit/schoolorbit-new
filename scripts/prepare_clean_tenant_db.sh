#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
database_url="${PREPARE_CLEAN_TENANT_DATABASE_URL:-}"
target_schema="${PREPARE_CLEAN_TENANT_SCHEMA:-public}"
confirm="${PREPARE_CLEAN_TENANT_CONFIRM:-}"
allow_non_test="${PREPARE_CLEAN_TENANT_ALLOW_NON_TEST:-}"
reset_schema="${PREPARE_CLEAN_TENANT_RESET_SCHEMA:-0}"
drop_schema_on_exit="${PREPARE_CLEAN_TENANT_DROP_SCHEMA_ON_EXIT:-0}"
schema_created_or_reset="0"

if [[ -z "$database_url" ]]; then
    printf 'Set PREPARE_CLEAN_TENANT_DATABASE_URL to the clean tenant database URL.\n' >&2
    exit 1
fi

if [[ ! "$target_schema" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
    printf 'PREPARE_CLEAN_TENANT_SCHEMA must contain only ASCII letters, numbers, and underscores, and must not start with a number.\n' >&2
    exit 1
fi

if [[ "$confirm" != "$target_schema" ]]; then
    printf 'Set PREPARE_CLEAN_TENANT_CONFIRM=%s to confirm clean baseline preparation.\n' "$target_schema" >&2
    exit 1
fi

if [[ "$database_url" != *"schoolorbit_test"* && "$allow_non_test" != "1" ]]; then
    printf 'Refusing non-test database unless PREPARE_CLEAN_TENANT_ALLOW_NON_TEST=1 is set intentionally.\n' >&2
    exit 1
fi

if [[ "$target_schema" == "public" && "$reset_schema" == "1" ]]; then
    printf 'Refusing to reset public schema. Use a brand-new database instead.\n' >&2
    exit 1
fi

for command in psql cargo; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required to prepare a clean tenant database.\n' "$command" >&2
        exit 1
    fi
done

cleanup() {
    if [[ "$target_schema" != "public" && "$drop_schema_on_exit" == "1" && "$schema_created_or_reset" == "1" ]]; then
        psql "$database_url" -v ON_ERROR_STOP=1 -X -q \
            -c "DROP SCHEMA IF EXISTS \"$target_schema\" CASCADE;" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

schema_exists() {
    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -t <<SQL
SELECT EXISTS (
    SELECT 1
    FROM information_schema.schemata
    WHERE schema_name = '$target_schema'
);
SQL
}

migration_history_exists() {
    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -t <<SQL
SELECT to_regclass(format('%I._sqlx_migrations', '$target_schema')) IS NOT NULL;
SQL
}

migration_stats() {
    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL
SELECT
    COUNT(*) FILTER (WHERE success) AS migration_count,
    COALESCE(MAX(version) FILTER (WHERE success), 0) AS migration_max_version,
    COUNT(*) FILTER (WHERE NOT success) AS failed_migration_count
FROM "$target_schema"._sqlx_migrations;
SQL
}

application_table_count() {
    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -t <<SQL
SELECT COUNT(*)
FROM information_schema.tables
WHERE table_schema = '$target_schema'
  AND table_type = 'BASE TABLE'
  AND table_name <> '_sqlx_migrations';
SQL
}

prepare_schema() {
    if [[ "$target_schema" == "public" ]]; then
        return
    fi

    if [[ "$reset_schema" == "1" ]]; then
        psql "$database_url" -v ON_ERROR_STOP=1 -X -q \
            -c "DROP SCHEMA IF EXISTS \"$target_schema\" CASCADE;"
        schema_created_or_reset="1"
    fi

    if [[ "$(schema_exists)" != "t" ]]; then
        psql "$database_url" -v ON_ERROR_STOP=1 -X -q \
            -c "CREATE SCHEMA \"$target_schema\";"
        schema_created_or_reset="1"
    fi
}

run_clean_migration() {
    (
        cd "$repo_root/backend-school"
        MIGRATION_SCHEMA_DATABASE_URL="$database_url" \
        MIGRATION_SCHEMA_NAME="$target_schema" \
        MIGRATION_SCHEMA_ALLOW_PUBLIC=1 \
            cargo run --quiet --bin migrate_tenant_schema
    )
}

validate_clean_schema() {
    local stats migration_count migration_max_version failed_migration_count
    stats="$(migration_stats)"
    IFS=$'\t' read -r migration_count migration_max_version failed_migration_count <<< "$stats"

    if [[ "$migration_count" != "1" || "$migration_max_version" != "1" || "$failed_migration_count" != "0" ]]; then
        printf 'Target migration history is not clean: count=%s max=%s failed=%s.\n' \
            "$migration_count" "$migration_max_version" "$failed_migration_count" >&2
        exit 1
    fi

    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL
SET search_path TO "$target_schema", public;
SELECT 'users', COUNT(*) FROM users;
SELECT 'permissions', COUNT(*) FROM permissions;
SELECT 'organization_units', COUNT(*) FROM organization_units;
SQL
}

prepare_schema

if [[ "$(migration_history_exists)" == "t" ]]; then
    stats="$(migration_stats)"
    IFS=$'\t' read -r migration_count migration_max_version failed_migration_count <<< "$stats"

    if [[ "$migration_count" == "1" && "$migration_max_version" == "1" && "$failed_migration_count" == "0" ]]; then
        printf 'Clean tenant baseline is already applied to schema: %s\n' "$target_schema"
        validate_clean_schema
        exit 0
    fi

    printf 'Refusing target with existing non-clean migration history: count=%s max=%s failed=%s.\n' \
        "$migration_count" "$migration_max_version" "$failed_migration_count" >&2
    exit 1
fi

existing_application_table_count="$(application_table_count)"
if [[ "$existing_application_table_count" != "0" ]]; then
    printf 'Refusing target schema with %s existing application tables and no clean migration history.\n' \
        "$existing_application_table_count" >&2
    exit 1
fi

printf 'Applying clean tenant baseline to schema: %s\n' "$target_schema"
run_clean_migration
printf 'Clean tenant baseline validation\n'
validate_clean_schema
