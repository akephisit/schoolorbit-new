#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
migration_dir="$repo_root/backend-school/migrations"
database_url="${MIGRATION_AUDIT_DATABASE_URL:-${DATABASE_URL:-}}"

if [[ ! -d "$migration_dir" ]]; then
    printf 'Migration directory not found: %s\n' "$migration_dir" >&2
    exit 1
fi

repo_stats="$(
    MIGRATION_DIR="$migration_dir" python3 - <<'PY'
import os
from pathlib import Path

migration_dir = Path(os.environ["MIGRATION_DIR"])
versions = []
for path in migration_dir.glob("*.sql"):
    prefix = path.name.split("_", 1)[0]
    if prefix.isdigit():
        versions.append(int(prefix))

if not versions:
    raise SystemExit("No migration files found")

versions.sort()
missing = [str(version) for version in range(versions[0], versions[-1] + 1) if version not in versions]
print(f"{len(versions)}\t{versions[0]}\t{versions[-1]}\t{','.join(missing)}")
PY
)"

IFS=$'\t' read -r repo_count repo_min_version repo_max_version repo_missing_versions <<< "$repo_stats"
repo_missing_versions="${repo_missing_versions:-}"

printf 'Repository migrations\n'
printf '  count: %s\n' "$repo_count"
printf '  min version: %s\n' "$repo_min_version"
printf '  max version: %s\n' "$repo_max_version"
printf '  gaps: %s\n' "${repo_missing_versions:-none}"

failures=0

if [[ "$repo_count" != "1" || "$repo_min_version" != "1" || "$repo_max_version" != "1" ]]; then
    printf 'FAIL active migrations must contain only backend-school/migrations/001_baseline.sql.\n' >&2
    failures=$((failures + 1))
fi

if [[ -n "$repo_missing_versions" ]]; then
    printf 'FAIL active migration versions have gaps.\n' >&2
    failures=$((failures + 1))
fi

if [[ -z "$database_url" ]]; then
    printf '\nSKIP database audit: set MIGRATION_AUDIT_DATABASE_URL to check a tenant database.\n'
    if [[ "$failures" -eq 0 ]]; then
        printf '\nClean migration baseline check passed.\n'
        exit 0
    fi
    printf '\nClean migration baseline check failed with %s failure(s).\n' "$failures" >&2
    exit 1
fi

if ! command -v psql >/dev/null 2>&1; then
    printf 'psql is required for database audit.\n' >&2
    exit 1
fi

db_stats="$(
    psql "$database_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL
WITH legacy_permission_codes(code) AS (
    VALUES
        ('settings.read'),
        ('settings.update'),
        ('dashboard'),
        ('student.create'),
        ('student.delete'),
        ('organization_work.create'),
        ('activity.members.manage'),
        ('admission.verify'),
        ('admission.scores'),
        ('admission.enroll')
),
migration_stats AS (
    SELECT
        COUNT(*) FILTER (WHERE success) AS applied_count,
        COALESCE(MAX(version) FILTER (WHERE success), 0) AS max_version,
        COUNT(*) FILTER (WHERE NOT success) AS failed_count,
        COUNT(*) FILTER (WHERE version = $repo_max_version AND success) AS current_version_count
    FROM _sqlx_migrations
),
permission_stats AS (
    SELECT COUNT(*) AS legacy_permission_count
    FROM permissions p
    JOIN legacy_permission_codes legacy ON legacy.code = p.code
)
SELECT
    migration_stats.applied_count,
    migration_stats.max_version,
    migration_stats.failed_count,
    migration_stats.current_version_count,
    permission_stats.legacy_permission_count
FROM migration_stats
CROSS JOIN permission_stats;
SQL
)"

IFS=$'\t' read -r applied_count db_max_version failed_count current_version_count legacy_permission_count <<< "$db_stats"

printf '\nTenant migration audit\n'
printf '  applied count: %s\n' "$applied_count"
printf '  max version: %s\n' "$db_max_version"
printf '  failed migrations: %s\n' "$failed_count"
printf '  current repo version applied: %s\n' "$current_version_count"
printf '  legacy permission codes: %s\n' "$legacy_permission_count"

if [[ "$applied_count" != "$repo_count" ]]; then
    printf 'FAIL applied migration count does not match repository count.\n' >&2
    failures=$((failures + 1))
fi

if [[ "$db_max_version" != "$repo_max_version" ]]; then
    printf 'FAIL tenant max migration version does not match repository max version.\n' >&2
    failures=$((failures + 1))
fi

if [[ "$failed_count" != "0" ]]; then
    printf 'FAIL tenant has failed migration rows.\n' >&2
    failures=$((failures + 1))
fi

if [[ "$current_version_count" != "1" ]]; then
    printf 'FAIL tenant has not applied the current repository migration version.\n' >&2
    failures=$((failures + 1))
fi

if [[ "$legacy_permission_count" != "0" ]]; then
    printf 'FAIL tenant still has legacy permission codes.\n' >&2
    failures=$((failures + 1))
fi

if [[ "$failures" -eq 0 ]]; then
    printf '\nClean migration baseline check passed.\n'
else
    printf '\nClean migration baseline check failed with %s failure(s).\n' "$failures" >&2
    exit 1
fi
