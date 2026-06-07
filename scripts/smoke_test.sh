#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
smoke_env_file="${SMOKE_ENV_FILE:-$repo_root/.env.smoke.local}"

if [[ -f "$smoke_env_file" ]]; then
    smoke_env_keys=(
        SMOKE_SUBDOMAIN
        SMOKE_API_URL
        SMOKE_ADMIN_API_URL
        SMOKE_TENANT_URL
        SMOKE_ORIGIN
        SMOKE_TIMEOUT_SECONDS
        SMOKE_USERNAME
        SMOKE_PASSWORD
        SMOKE_REMEMBER_ME
    )
    declare -A smoke_env_overrides=()

    for key in "${smoke_env_keys[@]}"; do
        if [[ -v $key ]]; then
            smoke_env_overrides["$key"]="${!key}"
        fi
    done

    set -a
    # shellcheck disable=SC1090
    source "$smoke_env_file"
    set +a

    for key in "${!smoke_env_overrides[@]}"; do
        export "$key=${smoke_env_overrides[$key]}"
    done
fi

SMOKE_SUBDOMAIN="${SMOKE_SUBDOMAIN:-sandbox}"
SMOKE_API_URL="${SMOKE_API_URL:-https://school-api.schoolorbit.app}"
SMOKE_ADMIN_API_URL="${SMOKE_ADMIN_API_URL:-https://admin-api.schoolorbit.app}"
SMOKE_TENANT_URL="${SMOKE_TENANT_URL:-https://${SMOKE_SUBDOMAIN}.schoolorbit.app}"
SMOKE_ORIGIN="${SMOKE_ORIGIN:-$SMOKE_TENANT_URL}"
SMOKE_TIMEOUT_SECONDS="${SMOKE_TIMEOUT_SECONDS:-20}"
SMOKE_USERNAME="${SMOKE_USERNAME:-}"
SMOKE_PASSWORD="${SMOKE_PASSWORD:-}"
SMOKE_REMEMBER_ME="${SMOKE_REMEMBER_ME:-true}"

SMOKE_API_URL="${SMOKE_API_URL%/}"
SMOKE_ADMIN_API_URL="${SMOKE_ADMIN_API_URL%/}"
SMOKE_TENANT_URL="${SMOKE_TENANT_URL%/}"
SMOKE_ORIGIN="${SMOKE_ORIGIN%/}"

failures=0
cookie_jar="$(mktemp)"
tmp_dir="$(mktemp -d)"

cleanup() {
    rm -f "$cookie_jar"
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

pass() {
    printf 'PASS %s\n' "$1"
}

fail() {
    failures=$((failures + 1))
    printf 'FAIL %s\n' "$1" >&2
}

request() {
    local name="$1"
    local method="$2"
    local url="$3"
    local headers_file="$4"
    local body_file="$5"
    shift 5

    local status
    if ! status="$(curl -sS --max-time "$SMOKE_TIMEOUT_SECONDS" \
        -X "$method" \
        -D "$headers_file" \
        -o "$body_file" \
        -w '%{http_code}' \
        "$@" \
        "$url")"; then
        fail "$name request failed"
        printf '000'
        return
    fi

    printf '%s' "$status"
}

header_value() {
    local headers_file="$1"
    local header_name="$2"
    awk -v header_name="$header_name" '
        BEGIN { header_name = tolower(header_name) }
        {
            line = $0
            sub(/\r$/, "", line)
            lower = tolower(line)
            if (index(lower, header_name ":") == 1) {
                sub(/^[^:]+:[ \t]*/, "", line)
                print line
                exit
            }
        }
    ' "$headers_file"
}

expect_status() {
    local name="$1"
    local actual="$2"
    local expected="$3"

    if [[ "$actual" == "$expected" ]]; then
        pass "$name status $expected"
    else
        fail "$name expected status $expected, got $actual"
    fi
}

expect_header() {
    local name="$1"
    local headers_file="$2"
    local header_name="$3"
    local expected="$4"
    local actual
    actual="$(header_value "$headers_file" "$header_name")"

    if [[ "$actual" == "$expected" ]]; then
        pass "$name $header_name"
    else
        fail "$name expected $header_name=$expected, got ${actual:-<missing>}"
    fi
}

expect_header_contains_ci() {
    local name="$1"
    local headers_file="$2"
    local header_name="$3"
    local expected_fragment="$4"
    local actual
    local actual_lower
    local expected_lower

    actual="$(header_value "$headers_file" "$header_name")"
    actual_lower="${actual,,}"
    expected_lower="${expected_fragment,,}"

    if [[ "$actual_lower" == *"$expected_lower"* ]]; then
        pass "$name $header_name contains $expected_fragment"
    else
        fail "$name expected $header_name to contain $expected_fragment, got ${actual:-<missing>}"
    fi
}

expect_body_contains() {
    local name="$1"
    local body_file="$2"
    local needle="$3"

    if grep -Fq "$needle" "$body_file"; then
        pass "$name body contains expected text"
    else
        fail "$name body missing expected text"
    fi
}

expect_json_username() {
    local name="$1"
    local body_file="$2"
    local username="$3"

    if python3 - "$body_file" "$username" <<'PY'
import json
import sys

path, expected_username = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as handle:
    data = json.load(handle)

if isinstance(data, dict) and isinstance(data.get("data"), dict):
    data = data["data"]

user = data.get("user", data) if isinstance(data, dict) else {}
actual = user.get("username") if isinstance(user, dict) else None
raise SystemExit(0 if actual == expected_username else 1)
PY
    then
        pass "$name username"
    else
        fail "$name username mismatch"
    fi
}

print_section() {
    printf '\n== %s ==\n' "$1"
}

print_section "SchoolOrbit smoke test"
printf 'Tenant: %s\n' "$SMOKE_TENANT_URL"
printf 'API: %s\n' "$SMOKE_API_URL"
printf 'Origin: %s\n' "$SMOKE_ORIGIN"
printf 'Subdomain header: %s\n' "$SMOKE_SUBDOMAIN"

tenant_headers="$tmp_dir/tenant.headers"
tenant_body="$tmp_dir/tenant.body"
status="$(request "tenant page" GET "$SMOKE_TENANT_URL/" "$tenant_headers" "$tenant_body")"
expect_status "tenant page" "$status" "200"
expect_body_contains "tenant page" "$tenant_body" "<!doctype html>"

admin_health_headers="$tmp_dir/admin-health.headers"
admin_health_body="$tmp_dir/admin-health.body"
status="$(request "admin health" GET "$SMOKE_ADMIN_API_URL/health" "$admin_health_headers" "$admin_health_body")"
expect_status "admin health" "$status" "200"

health_headers="$tmp_dir/health.headers"
health_body="$tmp_dir/health.body"
status="$(request "school API health" GET "$SMOKE_API_URL/health" "$health_headers" "$health_body")"
expect_status "school API health" "$status" "200"
expect_body_contains "school API health" "$health_body" '"status":"healthy"'

me_unauth_headers="$tmp_dir/me-unauth.headers"
me_unauth_body="$tmp_dir/me-unauth.body"
status="$(request "unauthenticated /me" GET "$SMOKE_API_URL/api/auth/me" "$me_unauth_headers" "$me_unauth_body" \
    -H "Origin: $SMOKE_ORIGIN" \
    -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
expect_status "unauthenticated /me" "$status" "401"
expect_header "unauthenticated /me" "$me_unauth_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"

preflight_headers="$tmp_dir/preflight.headers"
preflight_body="$tmp_dir/preflight.body"
status="$(request "login preflight" OPTIONS "$SMOKE_API_URL/api/auth/login" "$preflight_headers" "$preflight_body" \
    -H "Origin: $SMOKE_ORIGIN" \
    -H "Access-Control-Request-Method: POST" \
    -H "Access-Control-Request-Headers: content-type,authorization,x-school-subdomain")"
expect_status "login preflight" "$status" "204"
expect_header "login preflight" "$preflight_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
expect_header_contains_ci "login preflight" "$preflight_headers" "access-control-allow-headers" "x-school-subdomain"

if [[ -z "$SMOKE_USERNAME" || -z "$SMOKE_PASSWORD" ]]; then
    login_validation_headers="$tmp_dir/login-validation.headers"
    login_validation_body="$tmp_dir/login-validation.body"
    status="$(request "login validation" POST "$SMOKE_API_URL/api/auth/login" "$login_validation_headers" "$login_validation_body" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        -H "Content-Type: application/json" \
        --data '{}')"
    expect_status "login validation" "$status" "422"
    expect_header "login validation" "$login_validation_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    printf '\nSKIP authenticated checks: set SMOKE_USERNAME and SMOKE_PASSWORD to test login.\n'
else
    login_payload="$tmp_dir/login.json"
    SMOKE_USERNAME="$SMOKE_USERNAME" \
    SMOKE_PASSWORD="$SMOKE_PASSWORD" \
    SMOKE_REMEMBER_ME="$SMOKE_REMEMBER_ME" \
    python3 - <<'PY' > "$login_payload"
import json
import os

remember = os.environ.get("SMOKE_REMEMBER_ME", "true").lower() in {"1", "true", "yes", "on"}
print(json.dumps({
    "username": os.environ["SMOKE_USERNAME"],
    "password": os.environ["SMOKE_PASSWORD"],
    "rememberMe": remember,
}))
PY

    login_headers="$tmp_dir/login.headers"
    login_body="$tmp_dir/login.body"
    status="$(request "login" POST "$SMOKE_API_URL/api/auth/login" "$login_headers" "$login_body" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        -H "Content-Type: application/json" \
        --data-binary "@$login_payload")"
    expect_status "login" "$status" "200"
    expect_header "login" "$login_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_json_username "login" "$login_body" "$SMOKE_USERNAME"

    if grep -q 'auth_token' "$cookie_jar"; then
        pass "login auth_token cookie"
    else
        fail "login auth_token cookie missing"
    fi

    me_headers="$tmp_dir/me.headers"
    me_body="$tmp_dir/me.body"
    status="$(request "authenticated /me" GET "$SMOKE_API_URL/api/auth/me" "$me_headers" "$me_body" \
        -b "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
    expect_status "authenticated /me" "$status" "200"
    expect_header "authenticated /me" "$me_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_json_username "authenticated /me" "$me_body" "$SMOKE_USERNAME"
fi

if [[ "$failures" -eq 0 ]]; then
    printf '\nSmoke test passed.\n'
else
    printf '\nSmoke test failed with %s failure(s).\n' "$failures" >&2
    exit 1
fi
