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
        SMOKE_REQUIRE_AUTH
        SMOKE_ACADEMIC_CONTEXT
        SMOKE_DIRECT_BACKEND
        SMOKE_RESOLVE_IP
        FILE_SMOKE_PNG
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
SMOKE_REQUIRE_AUTH="${SMOKE_REQUIRE_AUTH:-false}"
SMOKE_ACADEMIC_CONTEXT="${SMOKE_ACADEMIC_CONTEXT:-false}"
SMOKE_DIRECT_BACKEND="${SMOKE_DIRECT_BACKEND:-false}"
SMOKE_RESOLVE_IP="${SMOKE_RESOLVE_IP:-}"
FILE_SMOKE_PNG="${FILE_SMOKE_PNG:-}"

SMOKE_API_URL="${SMOKE_API_URL%/}"
SMOKE_ADMIN_API_URL="${SMOKE_ADMIN_API_URL%/}"
SMOKE_TENANT_URL="${SMOKE_TENANT_URL%/}"
SMOKE_ORIGIN="${SMOKE_ORIGIN%/}"

case "$SMOKE_ACADEMIC_CONTEXT" in
    true | false) ;;
    *)
        printf 'SMOKE_ACADEMIC_CONTEXT must be true or false.\n' >&2
        exit 64
        ;;
esac
case "$SMOKE_DIRECT_BACKEND" in
    true | false) ;;
    *)
        printf 'SMOKE_DIRECT_BACKEND must be true or false.\n' >&2
        exit 64
        ;;
esac

admin_api_host=${SMOKE_ADMIN_API_URL#https://}
admin_api_host=${admin_api_host%%/*}
school_api_host=${SMOKE_API_URL#https://}
school_api_host=${school_api_host%%/*}
declare -a admin_api_curl_options=()
declare -a school_api_curl_options=()
if [[ -n $SMOKE_RESOLVE_IP ]]; then
    if [[ $SMOKE_ADMIN_API_URL != https://* || $SMOKE_API_URL != https://* ]] ||
        [[ ! $admin_api_host =~ ^[A-Za-z0-9.-]+$ || ! $school_api_host =~ ^[A-Za-z0-9.-]+$ ]] ||
        [[ ! $SMOKE_RESOLVE_IP =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then
        printf 'SMOKE_RESOLVE_IP requires valid HTTPS API hostnames and an IPv4 address.\n' >&2
        exit 64
    fi
    admin_api_curl_options+=(--resolve "${admin_api_host}:443:${SMOKE_RESOLVE_IP}")
    school_api_curl_options+=(--resolve "${school_api_host}:443:${SMOKE_RESOLVE_IP}")
fi

failures=0
cookie_jar="$(mktemp)"
tmp_dir="$(mktemp -d)"
file_smoke_id=
csrf_token=

cleanup() {
    if [[ -n $file_smoke_id ]]; then
        local -a cleanup_csrf_options=()
        if [[ -n $csrf_token ]]; then
            cleanup_csrf_options=(-H "X-CSRF-Token: $csrf_token")
        fi
        curl -sS --max-time "$SMOKE_TIMEOUT_SECONDS" \
            "${school_api_curl_options[@]}" \
            -X DELETE \
            -b "$cookie_jar" \
            -c "$cookie_jar" \
            -H "Origin: $SMOKE_ORIGIN" \
            -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
            "${cleanup_csrf_options[@]}" \
            -o /dev/null \
            "$SMOKE_API_URL/api/files/$file_smoke_id" || true
    fi
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

request_with_options() {
    local options_name="$1"
    local name="$2"
    local method="$3"
    local url="$4"
    local headers_file="$5"
    local body_file="$6"
    shift 6
    local -n request_curl_options="$options_name"

    local status
    if ! status="$(curl -sS --max-time "$SMOKE_TIMEOUT_SECONDS" \
        "${request_curl_options[@]}" \
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

request() {
    # shellcheck disable=SC2034 # Read through the nameref in request_with_options.
    local -a default_curl_options=()
    request_with_options default_curl_options "$@"
}

admin_api_request() {
    request_with_options admin_api_curl_options "$@"
}

school_api_request() {
    request_with_options school_api_curl_options "$@"
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

capture_csrf() {
    local headers_file="$1"
    local value

    value="$(awk '
        {
            line = $0
            sub(/\r$/, "", line)
            lower = tolower(line)
            if (index(lower, "x-csrf-token:") == 1) {
                sub(/^[^:]+:[ \t]*/, "", line)
                print line
            }
        }
    ' "$headers_file" | tail -n 1)"
    [[ -n $value ]] || return 1
    csrf_token="$value"
}

refresh_csrf_if_present() {
    local headers_file="$1"
    local value

    value="$(awk '
        {
            line = $0
            sub(/\r$/, "", line)
            lower = tolower(line)
            if (index(lower, "x-csrf-token:") == 1) {
                sub(/^[^:]+:[ \t]*/, "", line)
                print line
            }
        }
    ' "$headers_file" | tail -n 1)"
    [[ -z $value ]] || csrf_token="$value"
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

expect_cors_header() {
    if [[ $SMOKE_DIRECT_BACKEND == true ]]; then
        return
    fi
    expect_header "$@"
}

expect_cors_header_contains_ci() {
    if [[ $SMOKE_DIRECT_BACKEND == true ]]; then
        return
    fi
    expect_header_contains_ci "$@"
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

    if EXPECTED_SMOKE_USERNAME="$username" python3 - "$body_file" <<'PY'; then
import json
import os
import sys

path = sys.argv[1]
expected_username = os.environ["EXPECTED_SMOKE_USERNAME"]
with open(path, encoding="utf-8") as handle:
    data = json.load(handle)

if isinstance(data, dict) and isinstance(data.get("data"), dict):
    data = data["data"]

user = data.get("user", data) if isinstance(data, dict) else {}
actual = user.get("username") if isinstance(user, dict) else None
raise SystemExit(0 if actual == expected_username else 1)
PY
        pass "$name username"
    else
        fail "$name username mismatch"
    fi
}

expect_json_one_current_session() {
    local name="$1"
    local body_file="$2"

    if python3 - "$body_file" <<'PY'; then
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)

sessions = payload.get("data", {}).get("sessions", [])
is_valid = (
    isinstance(sessions, list)
    and len(sessions) >= 1
    and sum(session.get("isCurrent") is True for session in sessions if isinstance(session, dict)) == 1
)
raise SystemExit(0 if is_valid else 1)
PY
        pass "$name exactly one current session"
    else
        fail "$name expected exactly one current session"
    fi
}

expect_json_success() {
    local name="$1"
    local body_file="$2"

    if python3 - "$body_file" <<'PY'; then
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)

raise SystemExit(0 if isinstance(payload, dict) and payload.get("success") is True else 1)
PY
        pass "$name success envelope"
    else
        fail "$name expected a successful API envelope"
    fi
}

# Academic Core maintenance read-only smoke start
academic_context_get() {
    local key="$1"
    local name="$2"
    local path="$3"
    local headers_file="$tmp_dir/academic-$key.headers"
    local body_file="$tmp_dir/academic-$key.body"
    local status

    status="$(school_api_request "$name" GET "$SMOKE_API_URL$path" "$headers_file" "$body_file" \
        -b "$cookie_jar" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
    expect_status "$name" "$status" "200"
    if [[ $status == 200 ]]; then
        expect_json_success "$name" "$body_file"
    fi
    refresh_csrf_if_present "$headers_file"
}

run_academic_context_smoke() {
    local context_body="$tmp_dir/academic-context.body"
    local selected_years="$tmp_dir/academic-years"
    local selected_terms="$tmp_dir/academic-terms"
    local year_id
    local term_id
    local year_index=0
    local term_index=0

    academic_context_get "context" "academic context options" "/api/academic/context/options"
    if ! python3 - "$context_body" "$selected_years" "$selected_terms" <<'PY'; then
import json
import sys
import uuid

context_path, years_path, terms_path = sys.argv[1:]
with open(context_path, encoding="utf-8") as handle:
    payload = json.load(handle)

data = payload.get("data") if isinstance(payload, dict) else None
if not isinstance(data, dict):
    raise SystemExit(1)

years = data.get("years")
terms = data.get("terms")
if not isinstance(years, list) or not isinstance(terms, list):
    raise SystemExit(1)

def canonical_uuid(value):
    try:
        parsed = uuid.UUID(value)
    except (AttributeError, TypeError, ValueError):
        return None
    return str(parsed) if str(parsed) == value.lower() else None

year_rows = []
for row in years:
    if not isinstance(row, dict):
        continue
    year_id = canonical_uuid(row.get("id"))
    if year_id:
        year_rows.append(year_id)

term_rows = []
for row in terms:
    if not isinstance(row, dict):
        continue
    year_id = canonical_uuid(row.get("academicYearId"))
    term_id = canonical_uuid(row.get("id"))
    if year_id and term_id:
        term_rows.append((year_id, term_id))

active_year_id = canonical_uuid(data.get("activeAcademicYearId"))
active_term_id = canonical_uuid(data.get("activeAcademicTermId"))
selected_year_ids = []
for candidate in [active_year_id, *year_rows]:
    if candidate and candidate in year_rows and candidate not in selected_year_ids:
        selected_year_ids.append(candidate)
    if len(selected_year_ids) == 2:
        break

selected_term_rows = []
for candidate in term_rows:
    if candidate[1] == active_term_id:
        selected_term_rows.append(candidate)
        break
for candidate in term_rows:
    if candidate not in selected_term_rows:
        selected_term_rows.append(candidate)
    if len(selected_term_rows) == 2:
        break

if years and not selected_year_ids:
    raise SystemExit(1)
if terms and not selected_term_rows:
    raise SystemExit(1)
if any(year_id not in year_rows for year_id, _ in selected_term_rows):
    raise SystemExit(1)

with open(years_path, "w", encoding="ascii") as handle:
    handle.writelines(f"{year_id}\n" for year_id in selected_year_ids)
with open(terms_path, "w", encoding="ascii") as handle:
    handle.writelines(f"{year_id}\t{term_id}\n" for year_id, term_id in selected_term_rows)
PY
        fail "academic context options are canonical and internally consistent"
        return
    fi
    pass "academic context options are canonical and internally consistent"

    academic_context_get "years" "academic years" "/api/academic/years"
    while IFS= read -r year_id; do
        year_index=$((year_index + 1))
        academic_context_get "terms-$year_index" "academic terms context $year_index" "/api/academic/terms?academicYearId=$year_id"
        academic_context_get "dashboard-$year_index" "staff dashboard context $year_index" "/api/staff/dashboard?academicYearId=$year_id"
        academic_context_get "supervision-cycles-$year_index" "supervision cycles context $year_index" "/api/supervision/cycles?academicYearId=$year_id"
        academic_context_get "admission-rounds-$year_index" "admission rounds context $year_index" "/api/admission/rounds?academicYearId=$year_id"
    done <"$selected_years"

    while IFS=$'\t' read -r year_id term_id; do
        term_index=$((term_index + 1))
        academic_context_get "offerings-$term_index" "academic offerings context $term_index" "/api/academic/offerings?academicTermId=$term_id"
        academic_context_get "assessments-$term_index" "assessment plans context $term_index" "/api/academic/assessments/plans?academicTermId=$term_id"
        academic_context_get "timetable-$term_index" "academic timetable context $term_index" "/api/academic/timetable?academicTermId=$term_id"
        academic_context_get "exams-$term_index" "exam schedules context $term_index" "/api/academic/exam-schedules?academicTermId=$term_id"
        academic_context_get "supervision-$term_index" "supervision observations context $term_index" "/api/supervision/observations?academicYearId=$year_id&academicTermId=$term_id"
    done <"$selected_terms"
}
# Academic Core maintenance read-only smoke end

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
status="$(admin_api_request "admin health" GET "$SMOKE_ADMIN_API_URL/health" "$admin_health_headers" "$admin_health_body")"
expect_status "admin health" "$status" "200"

admin_ready_headers="$tmp_dir/admin-ready.headers"
admin_ready_body="$tmp_dir/admin-ready.body"
status="$(admin_api_request "admin readiness" GET "$SMOKE_ADMIN_API_URL/ready" "$admin_ready_headers" "$admin_ready_body")"
expect_status "admin readiness" "$status" "200"
expect_body_contains "admin readiness" "$admin_ready_body" '"status":"ready"'

health_headers="$tmp_dir/health.headers"
health_body="$tmp_dir/health.body"
status="$(school_api_request "school API health" GET "$SMOKE_API_URL/health" "$health_headers" "$health_body")"
expect_status "school API health" "$status" "200"
expect_body_contains "school API health" "$health_body" '"status":"healthy"'

ready_headers="$tmp_dir/ready.headers"
ready_body="$tmp_dir/ready.body"
status="$(school_api_request "school API readiness" GET "$SMOKE_API_URL/ready" "$ready_headers" "$ready_body")"
expect_status "school API readiness" "$status" "200"
expect_body_contains "school API readiness" "$ready_body" '"status":"ready"'
expect_body_contains "school API readiness" "$ready_body" '"controlPlane":"connected"'
expect_body_contains "school API readiness" "$ready_body" '"filePlatform":"ready"'

me_unauth_headers="$tmp_dir/me-unauth.headers"
me_unauth_body="$tmp_dir/me-unauth.body"
status="$(school_api_request "unauthenticated /me" GET "$SMOKE_API_URL/api/auth/me" "$me_unauth_headers" "$me_unauth_body" \
    -H "Origin: $SMOKE_ORIGIN" \
    -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
expect_status "unauthenticated /me" "$status" "401"
expect_cors_header "unauthenticated /me" "$me_unauth_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"

legacy_me_headers="$tmp_dir/me-legacy-cookie.headers"
legacy_me_body="$tmp_dir/me-legacy-cookie.body"
status="$(school_api_request "legacy cookie /me" GET "$SMOKE_API_URL/api/auth/me" "$legacy_me_headers" "$legacy_me_body" \
    -b 'auth_token=synthetic.legacy.jwt' \
    -H "Origin: $SMOKE_ORIGIN" \
    -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
expect_status "legacy cookie /me" "$status" "401"
expect_cors_header "legacy cookie /me" "$legacy_me_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
: >"$cookie_jar"

if [[ $SMOKE_DIRECT_BACKEND == false ]]; then
    preflight_headers="$tmp_dir/preflight.headers"
    preflight_body="$tmp_dir/preflight.body"
    status="$(school_api_request "login preflight" OPTIONS "$SMOKE_API_URL/api/auth/login" "$preflight_headers" "$preflight_body" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "Access-Control-Request-Method: POST" \
        -H "Access-Control-Request-Headers: content-type,x-school-subdomain,x-csrf-token")"
    expect_status "login preflight" "$status" "204"
    expect_cors_header "login preflight" "$preflight_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_cors_header_contains_ci "login preflight" "$preflight_headers" "access-control-allow-headers" "x-school-subdomain"
    expect_cors_header_contains_ci "login preflight" "$preflight_headers" "access-control-allow-headers" "x-csrf-token"
else
    printf '\nSKIP proxy CORS preflight: direct backend maintenance smoke.\n'
fi

if [[ -z "$SMOKE_USERNAME" || -z "$SMOKE_PASSWORD" ]]; then
    login_validation_headers="$tmp_dir/login-validation.headers"
    login_validation_body="$tmp_dir/login-validation.body"
    status="$(school_api_request "login validation" POST "$SMOKE_API_URL/api/auth/login" "$login_validation_headers" "$login_validation_body" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        -H "Content-Type: application/json" \
        --data '{}')"
    expect_status "login validation" "$status" "400"
    expect_cors_header "login validation" "$login_validation_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    if [[ $SMOKE_REQUIRE_AUTH == true ]]; then
        fail "authenticated checks are required but smoke credentials are missing"
    else
        printf '\nSKIP authenticated checks: set SMOKE_USERNAME and SMOKE_PASSWORD to test login.\n'
    fi
else
    login_payload="$tmp_dir/login.json"
    SMOKE_USERNAME="$SMOKE_USERNAME" \
        SMOKE_PASSWORD="$SMOKE_PASSWORD" \
        SMOKE_REMEMBER_ME="$SMOKE_REMEMBER_ME" \
        python3 - <<'PY' >"$login_payload"
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
    status="$(school_api_request "login" POST "$SMOKE_API_URL/api/auth/login" "$login_headers" "$login_body" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        -H "Content-Type: application/json" \
        --data-binary "@$login_payload")"
    expect_status "login" "$status" "200"
    expect_cors_header "login" "$login_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_cors_header_contains_ci "login" "$login_headers" "access-control-expose-headers" "x-csrf-token"
    expect_json_username "login" "$login_body" "$SMOKE_USERNAME"

    if capture_csrf "$login_headers"; then
        pass "login CSRF response header"
    else
        fail "login CSRF response header missing"
    fi

    if grep -Eq '[[:space:]]__Host-schoolorbit_session[[:space:]]' "$cookie_jar"; then
        pass "login opaque session cookie"
    else
        fail "login opaque session cookie missing"
    fi
    if grep -Eq '[[:space:]]auth_token[[:space:]]' "$cookie_jar"; then
        fail "login retained legacy auth_token cookie"
    else
        pass "login legacy auth_token cookie absent"
    fi

    me_headers="$tmp_dir/me.headers"
    me_body="$tmp_dir/me.body"
    status="$(school_api_request "authenticated /me" GET "$SMOKE_API_URL/api/auth/me" "$me_headers" "$me_body" \
        -b "$cookie_jar" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
    expect_status "authenticated /me" "$status" "200"
    expect_cors_header "authenticated /me" "$me_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_json_username "authenticated /me" "$me_body" "$SMOKE_USERNAME"
    if capture_csrf "$me_headers"; then
        pass "authenticated /me CSRF response header"
    else
        fail "authenticated /me CSRF response header missing"
    fi

    if [[ $SMOKE_ACADEMIC_CONTEXT == true ]]; then
        print_section "Academic Core read-only context smoke"
        run_academic_context_smoke
    fi

    sessions_headers="$tmp_dir/sessions.headers"
    sessions_body="$tmp_dir/sessions.body"
    status="$(school_api_request "session list" GET "$SMOKE_API_URL/api/auth/sessions" "$sessions_headers" "$sessions_body" \
        -b "$cookie_jar" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN")"
    expect_status "session list" "$status" "200"
    expect_cors_header "session list" "$sessions_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_json_one_current_session "session list" "$sessions_body"
    refresh_csrf_if_present "$sessions_headers"

    sse_headers="$tmp_dir/notifications-sse.headers"
    set +e
    sse_status="$(curl -sS --no-buffer --max-time 5 \
        "${school_api_curl_options[@]}" \
        -X GET \
        -D "$sse_headers" \
        -o /dev/null \
        -w '%{http_code}' \
        -b "$cookie_jar" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        "$SMOKE_API_URL/api/notifications/stream")"
    sse_curl_status=$?
    set -e
    if [[ $sse_status == 200 && ($sse_curl_status -eq 0 || $sse_curl_status -eq 28) ]]; then
        pass "notification SSE status 200"
    else
        fail "notification SSE expected status 200 with a bounded stream"
    fi
    expect_header_contains_ci "notification SSE" "$sse_headers" "content-type" "text/event-stream"
    expect_cors_header "notification SSE" "$sse_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
    expect_cors_header "notification SSE" "$sse_headers" "access-control-allow-credentials" "true"
    refresh_csrf_if_present "$sse_headers"

    if [[ -n $FILE_SMOKE_PNG ]]; then
        if [[ ! -r $FILE_SMOKE_PNG ]]; then
            fail "private file smoke PNG is not readable"
        else
            upload_headers="$tmp_dir/file-upload.headers"
            upload_body="$tmp_dir/file-upload.body"
            status="$(school_api_request "private file upload" POST "$SMOKE_API_URL/api/files" "$upload_headers" "$upload_body" \
                -b "$cookie_jar" \
                -c "$cookie_jar" \
                -H "Origin: $SMOKE_ORIGIN" \
                -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
                -H "X-CSRF-Token: $csrf_token" \
                -F 'purpose=profile_image' \
                -F "file=@$FILE_SMOKE_PNG;type=image/png")"
            expect_status "private file upload" "$status" "201"
            expect_cors_header "private file upload" "$upload_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
            refresh_csrf_if_present "$upload_headers"

            if file_smoke_id="$(
                python3 - "$upload_body" <<'PY'
import json
import sys
import uuid

try:
    with open(sys.argv[1], encoding="utf-8") as handle:
        payload = json.load(handle)
    value = payload["data"]["id"]
    uuid.UUID(value)
except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError):
    raise SystemExit(1)
print(value)
PY
            )"; then
                pass "private file upload identity"
            else
                file_smoke_id=
                fail "private file upload response identity"
            fi

            if [[ -n $file_smoke_id ]]; then
                grant_headers="$tmp_dir/file-grant.headers"
                grant_body="$tmp_dir/file-grant.body"
                : >"$grant_body"
                chmod 0600 "$grant_body"
                status="$(school_api_request "private file download grant" POST "$SMOKE_API_URL/api/files/$file_smoke_id/download" "$grant_headers" "$grant_body" \
                    -b "$cookie_jar" \
                    -c "$cookie_jar" \
                    -H "Origin: $SMOKE_ORIGIN" \
                    -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
                    -H "X-CSRF-Token: $csrf_token")"
                expect_status "private file download grant" "$status" "200"
                expect_cors_header "private file download grant" "$grant_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
                refresh_csrf_if_present "$grant_headers"

                downloaded_file="$tmp_dir/file-download.png"
                if python3 - "$grant_body" "$downloaded_file" "$SMOKE_ORIGIN" <<'PY'; then
import json
import sys
import urllib.parse
import urllib.request

grant_path, output_path, origin = sys.argv[1:]
try:
    with open(grant_path, encoding="utf-8") as handle:
        url = json.load(handle)["data"]["url"]
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise ValueError("invalid grant URL")
    request = urllib.request.Request(
        url,
        headers={"Origin": origin, "User-Agent": "SchoolOrbit-Smoke/1.0"},
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        content = response.read()
    with open(output_path, "wb") as handle:
        handle.write(content)
except Exception:
    raise SystemExit(1)
PY
                    pass "private file external grant fetch"
                else
                    fail "private file external grant fetch"
                fi

                if [[ -f $downloaded_file ]] && cmp -s "$FILE_SMOKE_PNG" "$downloaded_file"; then
                    pass "private file downloaded bytes"
                else
                    fail "private file downloaded bytes mismatch"
                fi

                delete_headers="$tmp_dir/file-delete.headers"
                delete_body="$tmp_dir/file-delete.body"
                status="$(school_api_request "private file delete" DELETE "$SMOKE_API_URL/api/files/$file_smoke_id" "$delete_headers" "$delete_body" \
                    -b "$cookie_jar" \
                    -c "$cookie_jar" \
                    -H "Origin: $SMOKE_ORIGIN" \
                    -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
                    -H "X-CSRF-Token: $csrf_token")"
                expect_status "private file delete" "$status" "200"
                refresh_csrf_if_present "$delete_headers"
                if [[ $status == 200 ]]; then
                    file_smoke_id=
                fi
            fi
        fi
    fi

    logout_headers="$tmp_dir/logout.headers"
    logout_body="$tmp_dir/logout.body"
    status="$(school_api_request "current session logout" POST "$SMOKE_API_URL/api/auth/logout" "$logout_headers" "$logout_body" \
        -b "$cookie_jar" \
        -c "$cookie_jar" \
        -H "Origin: $SMOKE_ORIGIN" \
        -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
        -H "X-CSRF-Token: $csrf_token")"
    expect_status "current session logout" "$status" "200"
    expect_cors_header "current session logout" "$logout_headers" "access-control-allow-origin" "$SMOKE_ORIGIN"
fi

if [[ "$failures" -eq 0 ]]; then
    printf '\nSmoke test passed.\n'
else
    printf '\nSmoke test failed with %s failure(s).\n' "$failures" >&2
    exit 1
fi
