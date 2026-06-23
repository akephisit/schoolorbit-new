#!/usr/bin/env bash
set -euo pipefail

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup_root="${BACKUP_ROOT:-./backups/postgres}"
retention_days="${BACKUP_RETENTION_DAYS:-14}"
rclone_remote="${BACKUP_RCLONE_REMOTE:-}"
admin_database_url="${DATABASE_URL:-}"
admin_backup_name="schoolorbit_admin_${timestamp}.dump"
backup_dir="${backup_root}/${timestamp}"
tenant_list=""

cleanup() {
    if [[ -n "$tenant_list" && -f "$tenant_list" ]]; then
        rm -f "$tenant_list"
    fi
}
trap cleanup EXIT

if [[ -z "$admin_database_url" ]]; then
    printf 'Set DATABASE_URL to the backend-admin database URL.\n' >&2
    exit 1
fi

if [[ ! "$retention_days" =~ ^[0-9]+$ ]]; then
    printf 'BACKUP_RETENTION_DAYS must be a non-negative integer.\n' >&2
    exit 1
fi

for command in pg_dump psql; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for PostgreSQL backups.\n' "$command" >&2
        exit 1
    fi
done

if [[ -n "$rclone_remote" ]] && ! command -v rclone >/dev/null 2>&1; then
    printf 'BACKUP_RCLONE_REMOTE is set but rclone is not installed.\n' >&2
    exit 1
fi

mkdir -p "$backup_dir"
chmod 700 "$backup_dir"

dump_database() {
    local database_url="$1"
    local output_path="$2"

    pg_dump \
        --dbname="$database_url" \
        --format=custom \
        --no-owner \
        --no-privileges \
        --file="$output_path"
}

printf 'Backing up admin database...\n'
dump_database "$admin_database_url" "${backup_dir}/${admin_backup_name}"

tenant_list="$(mktemp)"
chmod 600 "$tenant_list"

psql \
    --dbname="$admin_database_url" \
    -v ON_ERROR_STOP=1 \
    -X \
    -q \
    -A \
    -F $'\t' \
    -t <<'SQL' > "$tenant_list"
SELECT subdomain, db_connection_string
FROM schools
WHERE status IN ('active', 'provisioning')
  AND db_connection_string IS NOT NULL
  AND btrim(db_connection_string) <> ''
ORDER BY subdomain;
SQL

tenant_count=0
while IFS=$'\t' read -r subdomain tenant_database_url; do
    if [[ -z "$subdomain" || -z "$tenant_database_url" ]]; then
        continue
    fi

    tenant_count=$((tenant_count + 1))
    safe_subdomain="$(printf '%s' "$subdomain" | LC_ALL=C tr -c 'A-Za-z0-9_-' '_')"
    if [[ -z "$safe_subdomain" ]]; then
        safe_subdomain="tenant"
    fi

    output_name="tenant_${safe_subdomain}_${timestamp}.dump"
    output_path="${backup_dir}/${output_name}"
    if [[ -e "$output_path" ]]; then
        output_name="tenant_${safe_subdomain}_${tenant_count}_${timestamp}.dump"
        output_path="${backup_dir}/${output_name}"
    fi

    printf 'Backing up tenant: %s\n' "$subdomain"
    dump_database "$tenant_database_url" "$output_path"
done < "$tenant_list"

manifest="${backup_dir}/manifest.txt"
{
    printf 'timestamp=%s\n' "$timestamp"
    printf 'admin_backup=%s\n' "$admin_backup_name"
    printf 'tenant_count=%s\n' "$tenant_count"
    find "$backup_dir" -maxdepth 1 -type f -name 'tenant_*.dump' -printf 'tenant_backup=%f\n' | sort
} > "$manifest"
chmod 600 "$manifest"

if [[ -n "$rclone_remote" ]]; then
    printf 'Uploading backup to configured rclone remote...\n'
    rclone copy "$backup_dir" "${rclone_remote%/}/${timestamp}/"
fi

find "$backup_root" \
    -mindepth 1 \
    -maxdepth 1 \
    -type d \
    -name '????????T??????Z' \
    -mtime "+$retention_days" \
    -print \
    -exec rm -rf {} +

printf 'Backup completed: %s\n' "$backup_dir"
