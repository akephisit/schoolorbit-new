#!/usr/bin/env bash
set -euo pipefail

admin_url="${RESTORE_CHECK_ADMIN_URL:-${SELF_HOSTED_POSTGRES_ADMIN_URL:-}}"
dump_path="${RESTORE_CHECK_DUMP_PATH:-}"
database_kind="${RESTORE_CHECK_KIND:-tenant}"
database_name="restore_check_$(date -u +%Y%m%d%H%M%S)_$$"
created_database="0"
restore_check_passed="0"

cleanup() {
    local status=$?

    if [[ "$created_database" == "1" ]]; then
        dropdb --if-exists --maintenance-db="$admin_url" "$database_name" >/dev/null 2>&1 || true
    fi

    if [[ "$status" -ne 0 && "$restore_check_passed" != "1" ]]; then
        printf 'Restore check failed for %s dump: %s\n' "$database_kind" "$dump_path" >&2
    fi
}
trap cleanup EXIT

database_url_for_database() {
    local source_url="$1"
    local target_database="$2"
    local url_without_query="$source_url"
    local query_string=""
    local url_prefix

    if [[ "$source_url" == *\?* ]]; then
        query_string="?${source_url#*\?}"
        url_without_query="${source_url%%\?*}"
    fi

    url_prefix="${url_without_query%/*}"
    if [[ "$url_prefix" == "$url_without_query" || -z "$url_prefix" ]]; then
        printf 'RESTORE_CHECK_ADMIN_URL must include a database path.\n' >&2
        exit 1
    fi

    printf '%s/%s%s\n' "$url_prefix" "$target_database" "$query_string"
}

if [[ -z "$admin_url" ]]; then
    printf 'Set RESTORE_CHECK_ADMIN_URL or SELF_HOSTED_POSTGRES_ADMIN_URL.\n' >&2
    exit 1
fi

if [[ -z "$dump_path" || ! -f "$dump_path" ]]; then
    printf 'Set RESTORE_CHECK_DUMP_PATH to an existing .dump file.\n' >&2
    exit 1
fi

case "$database_kind" in
    admin | tenant) ;;
    *)
        printf 'RESTORE_CHECK_KIND must be admin or tenant.\n' >&2
        exit 1
        ;;
esac

for command in createdb dropdb pg_restore psql; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for restore checks.\n' "$command" >&2
        exit 1
    fi
done

restore_url="$(database_url_for_database "$admin_url" "$database_name")"

printf 'Creating disposable restore-check database: %s\n' "$database_name"
createdb --maintenance-db="$admin_url" "$database_name"
created_database="1"

printf 'Restoring %s dump: %s\n' "$database_kind" "$dump_path"
pg_restore \
    --dbname="$restore_url" \
    --no-owner \
    --no-privileges \
    --exit-on-error \
    "$dump_path"

printf 'Running %s restore sanity queries...\n' "$database_kind"
case "$database_kind" in
    admin)
        schools_count="$(
            psql --dbname="$restore_url" -v ON_ERROR_STOP=1 -X -q -A -t \
                -c 'SELECT COUNT(*) FROM schools;'
        )"
        printf 'Admin schools count: %s\n' "$schools_count"
        ;;
    tenant)
        successful_migrations_count="$(
            psql --dbname="$restore_url" -v ON_ERROR_STOP=1 -X -q -A -t \
                -c 'SELECT COUNT(*) FROM _sqlx_migrations WHERE success;'
        )"
        permissions_count="$(
            psql --dbname="$restore_url" -v ON_ERROR_STOP=1 -X -q -A -t \
                -c 'SELECT COUNT(*) FROM permissions;'
        )"
        printf 'Tenant successful migrations count: %s\n' "$successful_migrations_count"
        printf 'Tenant permissions count: %s\n' "$permissions_count"
        ;;
esac

restore_check_passed="1"
printf 'Restore check passed for %s dump: %s\n' "$database_kind" "$dump_path"
