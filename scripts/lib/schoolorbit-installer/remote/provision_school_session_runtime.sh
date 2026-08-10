#!/usr/bin/env bash
set -euo pipefail

ROOT_PREFIX=${SCHOOLORBIT_INSTALLER_TEST_ROOT-}
ROOT_PREFIX=${ROOT_PREFIX%/}
if [[ -z $ROOT_PREFIX ]]; then
    if ((EUID != 0)); then
        echo "ERROR: run this file with sudo"
        exit 1
    fi
else
    if [[ $ROOT_PREFIX != /* || $ROOT_PREFIX == / || ! -d $ROOT_PREFIX ]]; then
        echo "ERROR: invalid isolated test root"
        exit 1
    fi
fi
env_file="${ROOT_PREFIX}/opt/stack/.env"

if [[ ! -f $env_file || -L $env_file ]]; then
    echo "ERROR: runtime environment file is missing or is a symlink"
    exit 1
fi

command -v openssl >/dev/null || {
    echo "ERROR: openssl is unavailable"
    exit 1
}

require_one_key() {
    local key=$1
    local file=$2
    local count
    count=$(grep -c "^${key}=" "$file" || true)
    if ((count != 1)); then
        echo "ERROR: expected exactly one ${key} entry"
        exit 1
    fi
}

require_one_key JWT_SECRET "$env_file"

if grep -Eq '^(SESSION_HMAC_KEY|SCHOOL_ROLLBACK_JWT_SECRET)=' "$env_file"; then
    echo "STOP: school session secrets already exist; no changes were made"
    exit 2
fi

admin_jwt_line_before=$(grep -m1 '^JWT_SECRET=' "$env_file")
session_hmac_key=$(openssl rand -hex 32)
school_rollback_jwt_secret=$(openssl rand -hex 32)

while [[ $session_hmac_key == "$school_rollback_jwt_secret" ]]; do
    school_rollback_jwt_secret=$(openssl rand -hex 32)
done

timestamp=$(date -u +%Y%m%dT%H%M%SZ)
backup="${env_file}.before-session-${timestamp}"
temporary=$(mktemp "${env_file}.next.XXXXXX")
trap 'rm -f -- "$temporary"' EXIT

cp -a -- "$env_file" "$backup"
chmod 0600 "$backup"
if [[ -z $ROOT_PREFIX ]]; then
    chown --reference="$env_file" "$temporary"
fi
chmod 0600 "$temporary"

while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
        SESSION_HMAC_KEY=* | SCHOOL_ROLLBACK_JWT_SECRET=* | SCHOOL_LEGACY_JWT_SECRET=* | BASE_DOMAIN=* | TRUSTED_PROXY_CIDRS=* | SCHOOL_ALLOWED_DEV_ORIGINS=*) ;;
        *)
            printf '%s\n' "$line"
            ;;
    esac
done <"$env_file" >"$temporary"

{
    printf "SESSION_HMAC_KEY='%s'\n" "$session_hmac_key"
    printf "SCHOOL_ROLLBACK_JWT_SECRET='%s'\n" "$school_rollback_jwt_secret"
    printf "BASE_DOMAIN='schoolorbit.app'\n"
    printf "TRUSTED_PROXY_CIDRS='10.0.0.0/8,172.16.0.0/12'\n"
    printf "SCHOOL_ALLOWED_DEV_ORIGINS=''\n"
} >>"$temporary"

require_one_key JWT_SECRET "$temporary"
require_one_key SESSION_HMAC_KEY "$temporary"
require_one_key SCHOOL_ROLLBACK_JWT_SECRET "$temporary"

admin_jwt_line_after=$(grep -m1 '^JWT_SECRET=' "$temporary")
if [[ $admin_jwt_line_after != "$admin_jwt_line_before" ]]; then
    echo "ERROR: existing admin JWT_SECRET would change"
    exit 1
fi

mv -f -- "$temporary" "$env_file"
trap - EXIT
chmod 0600 "$env_file"

unset session_hmac_key school_rollback_jwt_secret
unset admin_jwt_line_before admin_jwt_line_after

echo "Runtime configuration updated successfully"
echo "Backup: $backup"
echo "Do not restart the container yet"
