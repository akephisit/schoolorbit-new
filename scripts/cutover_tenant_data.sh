#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_url="${CUTOVER_SOURCE_DATABASE_URL:-}"
target_url="${CUTOVER_TARGET_DATABASE_URL:-}"
mode="${CUTOVER_MODE:-dry-run}"
target_schema="${CUTOVER_TARGET_SCHEMA:-}"
allow_non_test_target="${CUTOVER_ALLOW_NON_TEST_TARGET:-}"
confirm_truncate="${CUTOVER_CONFIRM_TARGET_TRUNCATE:-}"
keep_schema="${CUTOVER_KEEP_SCHEMA:-}"
reset_target_schema="${CUTOVER_RESET_TARGET_SCHEMA:-}"

if [[ -z "$source_url" ]]; then
    printf 'Set CUTOVER_SOURCE_DATABASE_URL to the current tenant database URL.\n' >&2
    exit 1
fi

if [[ -z "$target_url" ]]; then
    printf 'Set CUTOVER_TARGET_DATABASE_URL to the clean baseline target database URL.\n' >&2
    exit 1
fi

if [[ "$mode" != "dry-run" && "$mode" != "apply" ]]; then
    printf 'CUTOVER_MODE must be dry-run or apply.\n' >&2
    exit 1
fi

if [[ -z "$target_schema" ]]; then
    if [[ "$mode" != "dry-run" ]]; then
        printf 'Set CUTOVER_TARGET_SCHEMA explicitly when CUTOVER_MODE=apply.\n' >&2
        exit 1
    fi
    target_schema="schoolorbit_cutover_$(date +%s)_$$"
fi

if [[ -z "$reset_target_schema" ]]; then
    if [[ "$mode" == "dry-run" ]]; then
        reset_target_schema="1"
    else
        reset_target_schema="0"
    fi
fi

if [[ -z "$confirm_truncate" && "$mode" == "dry-run" ]]; then
    confirm_truncate="$target_schema"
fi

if [[ ! "$target_schema" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
    printf 'CUTOVER_TARGET_SCHEMA must contain only ASCII letters, numbers, and underscores, and must not start with a number.\n' >&2
    exit 1
fi

if [[ "$target_schema" == "public" && "${CUTOVER_ALLOW_PUBLIC_TARGET:-}" != "1" ]]; then
    printf 'Refusing public target schema unless CUTOVER_ALLOW_PUBLIC_TARGET=1 is set intentionally.\n' >&2
    exit 1
fi

if [[ "$target_url" != *"schoolorbit_test"* && "$allow_non_test_target" != "1" ]]; then
    printf 'Refusing non-test target database unless CUTOVER_ALLOW_NON_TEST_TARGET=1 is set intentionally.\n' >&2
    exit 1
fi

if [[ "$source_url" == "$target_url" && "$target_schema" == "public" ]]; then
    printf 'Source and public target cannot be the same database.\n' >&2
    exit 1
fi

if [[ "$confirm_truncate" != "$target_schema" ]]; then
    printf 'Set CUTOVER_CONFIRM_TARGET_TRUNCATE=%s to confirm target truncation.\n' "$target_schema" >&2
    exit 1
fi

for command in psql pg_dump cargo diff; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for tenant data cutover.\n' "$command" >&2
        exit 1
    fi
done

raw_dump="$(mktemp)"
normalized_dump="$(mktemp)"
restore_dump="$(mktemp)"
source_tables="$(mktemp)"
target_tables="$(mktemp)"
source_counts="$(mktemp)"
target_counts="$(mktemp)"
foreign_keys_made_deferrable="0"
user_triggers_disabled="0"

cleanup() {
    if [[ "$user_triggers_disabled" == "1" && "$mode" == "apply" ]]; then
        set_target_user_triggers "ENABLE TRIGGER USER" >/dev/null 2>&1 || true
    fi

    if [[ "$foreign_keys_made_deferrable" == "1" && "$mode" == "apply" ]]; then
        set_target_foreign_key_deferrability "NOT DEFERRABLE" >/dev/null 2>&1 || true
    fi

    rm -f "$raw_dump" "$normalized_dump" "$restore_dump" "$source_tables" "$target_tables" "$source_counts" "$target_counts"

    if [[ "$mode" == "dry-run" && "$keep_schema" != "1" && "$target_schema" != "public" ]]; then
        psql "$target_url" -v ON_ERROR_STOP=1 -X -q \
            -c "DROP SCHEMA IF EXISTS \"$target_schema\" CASCADE;" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

run_target_migrations() {
    if [[ "$target_schema" == "public" ]]; then
        printf 'Skipping migration runner for public schema. The clean target database must already have applied 001_baseline.sql.\n'
        return
    fi

    (
        cd "$repo_root/backend-school"
        MIGRATION_SCHEMA_DATABASE_URL="$target_url" \
        MIGRATION_SCHEMA_NAME="$target_schema" \
            cargo run --quiet --bin migrate_tenant_schema
    )
}

table_list() {
    local database_url="$1"
    local schema="$2"

    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -t <<SQL
SELECT table_name
FROM information_schema.tables
WHERE table_schema = '$schema'
  AND table_type = 'BASE TABLE'
  AND table_name <> '_sqlx_migrations'
ORDER BY table_name;
SQL
}

table_counts() {
    local database_url="$1"
    local schema="$2"

    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL
WITH counted_tables AS (
    SELECT
        table_name,
        (
            xpath(
                '/row/count/text()',
                query_to_xml(
                    format('SELECT count(*) AS count FROM %I.%I', table_schema, table_name),
                    false,
                    true,
                    ''
                )
            )
        )[1]::text::bigint AS row_count
    FROM information_schema.tables
    WHERE table_schema = '$schema'
      AND table_type = 'BASE TABLE'
      AND table_name <> '_sqlx_migrations'
)
SELECT table_name, row_count
FROM counted_tables
ORDER BY table_name;
SQL
}

verify_clean_migration_history() {
    local stats
    stats="$(
        psql "$target_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL
SET search_path TO "$target_schema", public;
SELECT COUNT(*), COALESCE(MAX(version), 0)
FROM _sqlx_migrations
WHERE success;
SQL
    )"

    local migration_count migration_max_version
    IFS=$'\t' read -r migration_count migration_max_version <<< "$stats"

    if [[ "$migration_count" != "1" || "$migration_max_version" != "1" ]]; then
        printf 'Target must be a clean baseline with exactly one successful SQLx migration at version 1; found count=%s max=%s.\n' "$migration_count" "$migration_max_version" >&2
        exit 1
    fi
}

prepare_target_schema() {
    if [[ "$target_schema" == "public" ]]; then
        verify_clean_migration_history
        return
    fi

    if [[ "$reset_target_schema" == "1" ]]; then
        psql "$target_url" -v ON_ERROR_STOP=1 -X -q \
            -c "DROP SCHEMA IF EXISTS \"$target_schema\" CASCADE;"
    fi

    psql "$target_url" -v ON_ERROR_STOP=1 -X -q \
        -c "CREATE SCHEMA \"$target_schema\";"

    run_target_migrations
    verify_clean_migration_history
}

compare_table_lists() {
    table_list "$source_url" public > "$source_tables"
    table_list "$target_url" "$target_schema" > "$target_tables"

    if ! diff -u "$source_tables" "$target_tables"; then
        printf 'Source and target table lists differ; refusing to copy data.\n' >&2
        exit 1
    fi
}

truncate_target_application_tables() {
    psql "$target_url" -v ON_ERROR_STOP=1 -X -q <<SQL
DO \$\$
DECLARE
    truncate_statement text;
BEGIN
    SELECT
        'TRUNCATE TABLE '
        || string_agg(format('%I.%I', table_schema, table_name), ', ' ORDER BY table_name)
        || ' RESTART IDENTITY CASCADE'
    INTO truncate_statement
    FROM information_schema.tables
    WHERE table_schema = '$target_schema'
      AND table_type = 'BASE TABLE'
      AND table_name <> '_sqlx_migrations';

    IF truncate_statement IS NULL THEN
        RAISE EXCEPTION 'No application tables found in target schema %', '$target_schema';
    END IF;

    EXECUTE truncate_statement;
END
\$\$;
SQL
}

set_target_foreign_key_deferrability() {
    local deferrability="$1"

    case "$deferrability" in
        "DEFERRABLE INITIALLY IMMEDIATE" | "NOT DEFERRABLE") ;;
        *)
            printf 'Invalid foreign key deferrability mode: %s\n' "$deferrability" >&2
            exit 1
            ;;
    esac

    psql "$target_url" -v ON_ERROR_STOP=1 -X -q <<SQL
DO \$\$
DECLARE
    constraint_record record;
BEGIN
    FOR constraint_record IN
        SELECT
            table_namespace.nspname AS table_schema,
            table_class.relname AS table_name,
            constraint_info.conname AS constraint_name
        FROM pg_constraint constraint_info
        JOIN pg_class table_class ON table_class.oid = constraint_info.conrelid
        JOIN pg_namespace table_namespace ON table_namespace.oid = table_class.relnamespace
        WHERE table_namespace.nspname = '$target_schema'
          AND constraint_info.contype = 'f'
    LOOP
        EXECUTE format(
            'ALTER TABLE %I.%I ALTER CONSTRAINT %I $deferrability',
            constraint_record.table_schema,
            constraint_record.table_name,
            constraint_record.constraint_name
        );
    END LOOP;
END
\$\$;
SQL
}

set_target_user_triggers() {
    local trigger_mode="$1"

    case "$trigger_mode" in
        "DISABLE TRIGGER USER" | "ENABLE TRIGGER USER") ;;
        *)
            printf 'Invalid trigger mode: %s\n' "$trigger_mode" >&2
            exit 1
            ;;
    esac

    psql "$target_url" -v ON_ERROR_STOP=1 -X -q <<SQL
DO \$\$
DECLARE
    table_record record;
BEGIN
    FOR table_record IN
        SELECT table_schema, table_name
        FROM information_schema.tables
        WHERE table_schema = '$target_schema'
          AND table_type = 'BASE TABLE'
          AND table_name <> '_sqlx_migrations'
        ORDER BY table_name
    LOOP
        EXECUTE format(
            'ALTER TABLE %I.%I $trigger_mode',
            table_record.table_schema,
            table_record.table_name
        );
    END LOOP;
END
\$\$;
SQL
}

dump_source_data() {
    pg_dump "$source_url" \
        --schema=public \
        --data-only \
        --quote-all-identifiers \
        --no-owner \
        --no-privileges \
        --exclude-table=public._sqlx_migrations \
        > "$raw_dump"

    TARGET_SCHEMA="$target_schema" perl -ne '
        next if /set_config\('\''search_path'\''/;
        s/"public"\./"$ENV{TARGET_SCHEMA}"\./g;
        print;
    ' \
        "$raw_dump" > "$normalized_dump"

    if grep -q '_sqlx_migrations' "$normalized_dump"; then
        printf 'Dump unexpectedly contains _sqlx_migrations.\n' >&2
        exit 1
    fi

    if [[ "$target_schema" != "public" ]] && grep -q '"public"\.' "$normalized_dump"; then
        printf 'Dump still contains public schema qualifiers after normalization.\n' >&2
        exit 1
    fi
}

restore_target_data() {
    set_target_foreign_key_deferrability "DEFERRABLE INITIALLY IMMEDIATE"
    foreign_keys_made_deferrable="1"
    set_target_user_triggers "DISABLE TRIGGER USER"
    user_triggers_disabled="1"

    {
        printf 'BEGIN;\n'
        printf 'SET LOCAL search_path TO "%s", public;\n' "$target_schema"
        printf 'SET CONSTRAINTS ALL DEFERRED;\n'
        cat "$normalized_dump"
        printf '\nCOMMIT;\n'
    } > "$restore_dump"

    psql "$target_url" -v ON_ERROR_STOP=1 -X -q -f "$restore_dump"

    set_target_user_triggers "ENABLE TRIGGER USER"
    user_triggers_disabled="0"
    set_target_foreign_key_deferrability "NOT DEFERRABLE"
    foreign_keys_made_deferrable="0"
}

compare_row_counts() {
    table_counts "$source_url" public > "$source_counts"
    table_counts "$target_url" "$target_schema" > "$target_counts"

    if ! diff -u "$source_counts" "$target_counts"; then
        printf 'Source and target row counts differ after restore.\n' >&2
        exit 1
    fi
}

printf 'Tenant data cutover mode: %s\n' "$mode"
printf 'Target schema: %s\n' "$target_schema"

prepare_target_schema
compare_table_lists
truncate_target_application_tables
dump_source_data
restore_target_data
run_target_migrations
verify_clean_migration_history
compare_row_counts

printf 'Tenant data cutover validation passed for target schema: %s\n' "$target_schema"

if [[ "$mode" == "dry-run" && "$keep_schema" != "1" ]]; then
    printf 'Dry-run target schema will be dropped during cleanup. Set CUTOVER_KEEP_SCHEMA=1 to inspect it manually.\n'
fi
