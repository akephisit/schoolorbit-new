#!/usr/bin/env bash
set -eu
REGISTRY_TOKEN=${REGISTRY_TOKEN:?REGISTRY_TOKEN is required}
REGISTRY_USER=${REGISTRY_USER:?REGISTRY_USER is required}
BASE_DOMAIN=${BASE_DOMAIN:?BASE_DOMAIN is required}
BACKEND_IMAGE_DIGEST=${BACKEND_IMAGE_DIGEST:?BACKEND_IMAGE_DIGEST is required}
RELEASE_SHA=${RELEASE_SHA:?RELEASE_SHA is required}
echo "${REGISTRY_TOKEN}" | podman login ghcr.io -u "${REGISTRY_USER}" --password-stdin
cd /opt/stack

base_domain="${BASE_DOMAIN}"
deployment_root=/opt/stack/deployment
runtime_source="$deployment_root/podman-compose.yml"
runtime_compose=/opt/stack/podman-compose.yml
runtime_env=/opt/stack/.env
renderer_source="$deployment_root/scripts/render_nginx_config.sh"
renderer="$deployment_root/render-nginx"
origin_root_installer_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/install_origin_root.sh"
origin_root_installer="$deployment_root/install-origin-root"
network_helper_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh"
timing_helper_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/deployment_timing.sh"
image_cleanup="$deployment_root/scripts/prune_runtime_images.sh"
clamd_matcher="$deployment_root/scripts/clamd_runtime_matches.sh"
r2_cors_helper_source="$deployment_root/scripts/reconcile_r2_cors.sh"
proxy_template="$deployment_root/nginx-configs/school-api.conf.template"
maintenance_proxy_template="$deployment_root/nginx-configs/school-api.maintenance.conf.template"
proxy_source="$deployment_root/school-api.conf"
maintenance_proxy_source="$deployment_root/school-api.maintenance.conf"
proxy_target=/opt/stack/nginx/conf.d/school-api.conf
legacy_proxy_target=/opt/stack/nginx/conf.d/school-api.${base_domain}.conf
origin_root=/opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem
backend_image=ghcr.io/akephisit/schoolorbit-backend-school
backend_image_digest="${BACKEND_IMAGE_DIGEST}"
if ! printf '%s' "$backend_image_digest" | grep -Eq '^sha256:[0-9a-f]{64}$'; then
    echo "Backend image digest is invalid"
    exit 1
fi
backend_school_image_reference="${backend_image}@${backend_image_digest}"
test -f "$runtime_source" || {
    echo "Canonical runtime definition is missing"
    exit 1
}
test -f "$runtime_env" || {
    echo "Backend runtime environment is missing"
    exit 1
}
test -f "$renderer_source" || {
    echo "Proxy renderer is missing"
    exit 1
}
test -f "$origin_root_installer_source" || {
    echo "Origin CA root installer is missing"
    exit 1
}
test -f "$network_helper_source" || {
    echo "Container network helper is missing"
    exit 1
}
test -f "$timing_helper_source" || {
    echo "Deployment timing helper is missing"
    exit 1
}
test -f "$image_cleanup" || {
    echo "Runtime image cleanup is missing"
    exit 1
}
test -x "$clamd_matcher" || {
    echo "ClamAV runtime matcher is missing"
    exit 1
}
test -f "$proxy_template" || {
    echo "School API proxy template is missing"
    exit 1
}
test -f "$maintenance_proxy_template" || {
    echo "School API maintenance template is missing"
    exit 1
}

# shellcheck source=scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
. "$timing_helper_source"

runtime_env_value() {
    local key=$1 line value decoded char
    local -i index=0
    case "$key" in
        JWT_SECRET | SESSION_HMAC_KEY | SCHOOL_ROLLBACK_JWT_SECRET | \
            BASE_DOMAIN | TRUSTED_PROXY_CIDRS | SCHOOL_ALLOWED_DEV_ORIGINS | \
            R2_ACCOUNT_ID | R2_ACCESS_KEY_ID | R2_SECRET_ACCESS_KEY | \
            R2_PUBLIC_BUCKET_NAME | R2_PRIVATE_BUCKET_NAME) ;;
        *) return 64 ;;
    esac
    line="$(grep -m 1 "^${key}=" "$runtime_env")" || return
    value=${line#*=}
    if [[ ${#value} -lt 2 || ${value:0:1} != "'" || ${value: -1} != "'" ]]; then
        echo "Runtime environment value is not installer encoded: ${key}" >&2
        return 65
    fi
    value=${value:1:${#value}-2}
    decoded=
    while ((index < ${#value})); do
        char=${value:index:1}
        if [[ $char == \\ ]]; then
            ((index += 1))
            if ((index >= ${#value})); then
                echo "Runtime environment value has invalid escaping: ${key}" >&2
                return 65
            fi
            char=${value:index:1}
            if [[ $char != \\ && $char != "'" ]]; then
                echo "Runtime environment value has invalid escaping: ${key}" >&2
                return 65
            fi
        fi
        decoded+=$char
        ((index += 1))
    done
    printf '%s' "$decoded"
}
export_school_compose_env() {
    export BACKEND_SCHOOL_IMAGE_REFERENCE="$backend_school_image_reference"
    export SESSION_HMAC_KEY="$session_hmac_key"
    export SCHOOL_ROLLBACK_JWT_SECRET="$school_rollback_jwt_secret"
    export BASE_DOMAIN="$runtime_base_domain"
    export TRUSTED_PROXY_CIDRS="$trusted_proxy_cidrs"
    export SCHOOL_ALLOWED_DEV_ORIGINS="$school_allowed_dev_origins"
}
unset_school_compose_env() {
    unset BACKEND_SCHOOL_IMAGE_REFERENCE SESSION_HMAC_KEY
    unset SCHOOL_ROLLBACK_JWT_SECRET BASE_DOMAIN
    unset TRUSTED_PROXY_CIDRS SCHOOL_ALLOWED_DEV_ORIGINS
}
if ! session_hmac_key="$(runtime_env_value SESSION_HMAC_KEY)"; then
    echo "Required backend-school runtime value is unavailable: SESSION_HMAC_KEY"
    exit 1
fi

if ! school_rollback_jwt_secret="$(runtime_env_value SCHOOL_ROLLBACK_JWT_SECRET)"; then
    echo "Required backend-school runtime value is unavailable: SCHOOL_ROLLBACK_JWT_SECRET"
    exit 1
fi
if ! admin_jwt_secret="$(runtime_env_value JWT_SECRET)"; then
    echo "Required backend-admin runtime value is unavailable: JWT_SECRET"
    exit 1
fi
if [ "${#session_hmac_key}" -lt 32 ]; then
    echo "Backend-school runtime value is too short: SESSION_HMAC_KEY"
    exit 1
fi
if [ "${#school_rollback_jwt_secret}" -lt 32 ]; then
    echo "Backend-school runtime value is too short: SCHOOL_ROLLBACK_JWT_SECRET"
    exit 1
fi
if [ "$session_hmac_key" = "$school_rollback_jwt_secret" ] ||
    [ "$session_hmac_key" = "$admin_jwt_secret" ] ||
    [ "$school_rollback_jwt_secret" = "$admin_jwt_secret" ]; then
    echo "School session secrets must be distinct from each other and admin JWT_SECRET"
    exit 1
fi
if ! runtime_base_domain="$(runtime_env_value BASE_DOMAIN)"; then
    echo "Required backend-school runtime value is unavailable: BASE_DOMAIN"
    exit 1
fi
if [ "$runtime_base_domain" != "$base_domain" ]; then
    echo "Backend-school runtime BASE_DOMAIN does not match the deployment domain"
    exit 1
fi
if ! trusted_proxy_cidrs="$(runtime_env_value TRUSTED_PROXY_CIDRS)" ||
    [ -z "$trusted_proxy_cidrs" ]; then
    echo "Required backend-school runtime value is unavailable: TRUSTED_PROXY_CIDRS"
    exit 1
fi
if ! school_allowed_dev_origins="$(runtime_env_value SCHOOL_ALLOWED_DEV_ORIGINS)"; then
    echo "Required backend-school runtime value is unavailable: SCHOOL_ALLOWED_DEV_ORIGINS"
    exit 1
fi
unset admin_jwt_secret

install -m 0755 "$renderer_source" "$renderer"
install -m 0755 "$origin_root_installer_source" "$origin_root_installer"
"$origin_root_installer" "$origin_root"
export_school_compose_env
cp "$runtime_source" "${runtime_compose}.next"
podman-compose -f "${runtime_compose}.next" --dry-run up -d backend-school >/dev/null 2>&1
unset_school_compose_env
mv "${runtime_compose}.next" "$runtime_compose"
"$renderer" "$proxy_template" "$proxy_source" "$base_domain" "${RELEASE_SHA}" ready
"$renderer" "$maintenance_proxy_template" "$maintenance_proxy_source" "$base_domain" "${RELEASE_SHA}" maintenance "$(cat "/opt/stack/deployment/releases/${RELEASE_SHA}/probe-token")"

proxy_matches="$(grep -lF "server_name school-api.${base_domain};" /opt/stack/nginx/conf.d/*.conf 2>/dev/null || true)"
proxy_match_count="$(printf '%s\n' "$proxy_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
if [ "$proxy_match_count" -gt 1 ]; then
    echo "Expected at most one active school API proxy config, found ${proxy_match_count}"
    exit 1
fi
if [ "$proxy_match_count" -eq 1 ] &&
    [ "$proxy_matches" != "$proxy_target" ] &&
    [ "$proxy_matches" != "$legacy_proxy_target" ]; then
    echo "School API proxy exists at an unexpected path: ${proxy_matches}"
    exit 1
fi
proxy_previous_target="$proxy_target"
if [ "$proxy_match_count" -eq 1 ]; then
    proxy_previous_target="$proxy_matches"
fi
public_bucket="$(runtime_env_value R2_PUBLIC_BUCKET_NAME)"
private_bucket="$(runtime_env_value R2_PRIVATE_BUCKET_NAME)"
if [ "$public_bucket" = "$private_bucket" ]; then
    echo "Public and private File Platform buckets must be different"
    exit 1
fi

r2_started="$(schoolorbit_timer_now)"
AWS_ACCESS_KEY_ID="$(runtime_env_value R2_ACCESS_KEY_ID)"
export AWS_ACCESS_KEY_ID
AWS_SECRET_ACCESS_KEY="$(runtime_env_value R2_SECRET_ACCESS_KEY)"
export AWS_SECRET_ACCESS_KEY
export AWS_DEFAULT_REGION=auto
export AWS_EC2_METADATA_DISABLED=true
r2_account_id="$(runtime_env_value R2_ACCOUNT_ID)"
r2_endpoint="https://${r2_account_id}.r2.cloudflarestorage.com"
r2_cli() {
    podman run --rm \
        -e AWS_ACCESS_KEY_ID \
        -e AWS_SECRET_ACCESS_KEY \
        -e AWS_DEFAULT_REGION \
        -e AWS_EC2_METADATA_DISABLED \
        docker.io/amazon/aws-cli:2.36.9 \
        --endpoint-url "$r2_endpoint" \
        "$@"
}
podman pull docker.io/amazon/aws-cli:2.36.9
if ! r2_cli s3api head-bucket --bucket "$public_bucket" >/dev/null 2>&1; then
    echo "Configured public File Platform bucket is not accessible"
    exit 1
fi
if ! r2_cli s3api head-bucket --bucket "$private_bucket" >/dev/null 2>&1; then
    if ! r2_cli s3api create-bucket --bucket "$private_bucket" >/dev/null; then
        echo "Private File Platform bucket is absent and could not be created"
        exit 1
    fi
fi
r2_cli s3api head-bucket --bucket "$private_bucket" >/dev/null

private_cors_origin="https://*.${base_domain}"
private_cors_policy="$(
    jq -cn --arg origin "$private_cors_origin" \
        '{CORSRules:[{AllowedOrigins:[$origin],AllowedMethods:["GET","HEAD"],AllowedHeaders:["Range"],ExposeHeaders:["Accept-Ranges","Content-Length","Content-Range","Content-Type","ETag"],MaxAgeSeconds:3600}]}'
)"
# shellcheck source=scripts/reconcile_r2_cors.sh
. "$r2_cors_helper_source"
schoolorbit_reconcile_r2_cors "$private_bucket" "$private_cors_policy"
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY
schoolorbit_timer_report r2_reconciliation "$r2_started"

# shellcheck source=scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh
. "$network_helper_source"
compose_up_quiet() {
    if [ "$#" -eq 1 ] && [ "$1" = clamd ]; then
        # podman-compose <1.4 ignores the service's pids_limit.
        # Restrict this compatibility flag to the scanner only.
        podman-compose --podman-run-args='--pids-limit=256' \
            -f "$runtime_compose" up -d --no-deps clamd >/dev/null 2>&1
    else
        podman-compose -f "$runtime_compose" up -d "$@" >/dev/null 2>&1
    fi
}
reconnect_backend_network() {
    container=$1
    service_alias=$2
    schoolorbit_ensure_container_network_aliases \
        schoolorbit-web "$container" "$service_alias" "$container" schoolorbit-nginx
}
recreate_nginx() {
    podman stop schoolorbit-nginx >/dev/null 2>&1 || true
    podman rm schoolorbit-nginx >/dev/null 2>&1 || true
    compose_up_quiet nginx
}
validate_nginx_config_with_retry() {
    nginx_test_output=""
    for nginx_test_attempt in $(seq 1 30); do
        if nginx_test_output="$(podman exec schoolorbit-nginx nginx -t 2>&1)"; then
            printf '%s\n' "$nginx_test_output"
            return 0
        fi
        if [ "$nginx_test_attempt" -lt 30 ]; then
            sleep 2
        fi
    done
    printf '%s\n' "$nginx_test_output" >&2
    return 1
}
podman-compose -f "$runtime_compose" --dry-run up -d clamd backend-school >/dev/null 2>&1

# The scanner gets an isolated container network and no published port.
scanner_started="$(schoolorbit_timer_now)"
clamd_image=docker.io/clamav/clamav-debian:1.5.3
podman pull "$clamd_image"
if clamd_match_output="$("$clamd_matcher" "$clamd_image" schoolorbit-clamd)"; then
    if [ "$clamd_match_output" != "clamd_action=reused" ]; then
        echo "ClamAV runtime matcher returned an invalid result"
        exit 1
    fi
    printf '%s\n' "$clamd_match_output"
else
    case "$clamd_match_output" in
        'clamd_drift reason=missing_container') clamd_reason=missing_container ;;
        'clamd_drift reason=container_inspect') clamd_reason=container_inspect ;;
        'clamd_drift reason=image_inspect') clamd_reason=image_inspect ;;
        'clamd_drift reason=image') clamd_reason=image ;;
        'clamd_drift reason=memory') clamd_reason=memory ;;
        'clamd_drift reason=cpu') clamd_reason=cpu ;;
        'clamd_drift reason=pids') clamd_reason=pids ;;
        'clamd_drift reason=restart') clamd_reason=restart ;;
        'clamd_drift reason=security') clamd_reason=security ;;
        'clamd_drift reason=published_port') clamd_reason=published_port ;;
        'clamd_drift reason=signature_volume') clamd_reason=signature_volume ;;
        'clamd_drift reason=network') clamd_reason=network ;;
        'clamd_drift reason=running') clamd_reason=running ;;
        'clamd_drift reason=health') clamd_reason=health ;;
        *) clamd_reason=matcher_failure ;;
    esac
    printf 'clamd_action=recreated reason=%s\n' "$clamd_reason"
    if podman container exists schoolorbit-clamd; then
        podman stop schoolorbit-clamd
        podman rm schoolorbit-clamd
    fi
    compose_up_quiet clamd
fi

expected_clamd_memory_bytes=$((3 * 1024 * 1024 * 1024))
clamd_pids_limit="$(podman inspect --format '{{.HostConfig.PidsLimit}}' schoolorbit-clamd)"
if [ "$clamd_pids_limit" != 256 ]; then
    echo "Clamd PID limit mismatch after creation; refusing to continue"
    exit 1
fi
clamd_memory_bytes="$(podman inspect --format '{{.HostConfig.Memory}}' schoolorbit-clamd)"
if [ "$clamd_memory_bytes" != "$expected_clamd_memory_bytes" ]; then
    echo "Clamd memory limit mismatch: expected ${expected_clamd_memory_bytes} bytes, got ${clamd_memory_bytes} bytes"
    exit 1
fi
scanner_ready=false
for attempt in $(seq 1 60); do
    scanner_status="$(podman inspect --format '{{.State.Health.Status}}' schoolorbit-clamd 2>/dev/null || true)"
    if [ "$scanner_status" = "healthy" ]; then
        scanner_ready=true
        break
    fi
    if [ "$attempt" -lt 60 ]; then
        sleep 10
    fi
done
if [ "$scanner_ready" != "true" ]; then
    podman logs --tail 100 schoolorbit-clamd
    exit 1
fi
schoolorbit_timer_report scanner_readiness "$scanner_started"

jq_image=ghcr.io/jqlang/jq:1.7.1
podman pull "$jq_image"
image_pull_started="$(schoolorbit_timer_now)"
podman pull "$backend_school_image_reference"
schoolorbit_timer_report image_pull "$image_pull_started"

# Keep the shared admin upstream resolvable before Nginx is recreated.
reconnect_backend_network schoolorbit-backend-admin backend-admin

proxy_backup=""
proxy_existed=0
if [ -f "$proxy_previous_target" ]; then
    proxy_backup="$(mktemp /opt/stack/nginx/conf.d/.school-api.backup.XXXXXX)"
    cp "$proxy_previous_target" "$proxy_backup"
    proxy_existed=1
fi
restore_pre_maintenance_proxy() {
    rm -f "$proxy_target"
    if [ "$proxy_existed" -eq 1 ]; then
        cp "$proxy_backup" "$proxy_previous_target"
    fi
    recreate_nginx
    validate_nginx_config_with_retry
    podman exec schoolorbit-nginx nginx -s reload
}
if [ "$proxy_previous_target" != "$proxy_target" ]; then
    rm -f "$proxy_previous_target"
fi
echo "Activate School API maintenance for release ${RELEASE_SHA}"
cp "$maintenance_proxy_source" "$proxy_target"
if ! recreate_nginx ||
    ! validate_nginx_config_with_retry ||
    ! podman exec schoolorbit-nginx nginx -s reload; then
    restore_pre_maintenance_proxy
    [ -z "$proxy_backup" ] || rm -f "$proxy_backup"
    echo "School API could not enter validated maintenance mode"
    exit 1
fi

backend_readiness_started="$(schoolorbit_timer_now)"

# Recreate backend-school only; do not restart unrelated services.
if podman container exists schoolorbit-backend-school; then
    podman rm --force schoolorbit-backend-school
    if podman container exists schoolorbit-backend-school; then
        echo "Stale backend-school container remains after forced removal"
        exit 1
    fi
fi
export_school_compose_env
compose_up_quiet --no-deps backend-school
unset_school_compose_env
unset session_hmac_key school_rollback_jwt_secret
unset runtime_base_domain trusted_proxy_cidrs school_allowed_dev_origins
reconnect_backend_network schoolorbit-backend-school backend-school

# /ready checks control plane, both R2 buckets, and clamd.
# The child shell expands its own positional arguments.
# shellcheck disable=SC2016
if ! timeout 180 bash -c '
  for attempt in $(seq 1 36); do
    if curl -fsS "$1" >/dev/null; then
      exit 0
    fi
    if [ "$attempt" -lt 36 ]; then
      sleep 5
    fi
  done
  exit 1
' _ http://127.0.0.1:8081/ready; then
    podman logs --tail 100 schoolorbit-backend-school
    echo "Cutover image is not ready; maintenance remains enabled and pre-cutover rollback is disabled"
    exit 1
fi
schoolorbit_timer_report backend_readiness "$backend_readiness_started"

migration_response="$(mktemp /opt/stack/.backend-school-migration.XXXXXX)"
chmod 600 "$migration_response"
cleanup_migration_response() {
    rm -f "$migration_response"
}
print_migration_verification_failure() {
    podman run --rm -i "$jq_image" -r '
    "tenant_migration_summary latest_version=\(.latest_version // "unknown") total=\(.total // "unknown") success=\(.success // "unknown") failed=\(.failed // "unknown")",
    (
      .results[]?
      | "tenant_migration_result subdomain=\(.subdomain // "unknown") status=\(.status // "unknown") version=\(.version // "none") error_code=\(
          if .error == null then "none"
          else (([.error | scan("ACADEMIC_[A-Z0-9_]+") ] | first) // "redacted")
          end
        )"
    ),
    (
      .results[]? as $result
      | $result.academicDiagnostics.currentTeacherConflicts[]?
      | "tenant_academic_teacher_conflict subdomain=\($result.subdomain // "unknown") teacher_id=\(.teacherId // "unknown") timetable_version_id=\(.timetableVersionId // "unknown") day=\(.dayOfWeek // "unknown") bell_schedule_period_id=\(.bellSchedulePeriodId // "unknown") entry_count=\(.entryCount // "unknown") group_code_count=\(.groupCodeCount // "unknown") entry_ids=\((.entryIds // []) | tojson) group_codes=\((.groupCodes // []) | tojson)"
    )
  '
}
trap cleanup_migration_response EXIT

internal_api_secret="$(
    podman inspect schoolorbit-backend-school \
        --format '{{range .Config.Env}}{{println .}}{{end}}' |
        sed -n 's/^INTERNAL_API_SECRET=//p' |
        tail -n 1
)"
if [ -z "$internal_api_secret" ]; then
    echo "Backend internal migration credential is unavailable; maintenance remains enabled"
    exit 1
fi
tenant_migration_started="$(schoolorbit_timer_now)"
if ! curl \
    --fail \
    --silent \
    --show-error \
    --connect-timeout 10 \
    --max-time 900 \
    -X POST \
    -H "X-Internal-Secret: ${internal_api_secret}" \
    -o "$migration_response" \
    http://127.0.0.1:8081/internal/migrate-all; then
    echo "Tenant migration request failed; maintenance remains enabled"
    exit 1
fi
if ! podman run --rm -i "$jq_image" -e '
    .latest_version as $latest
    | $latest >= 32
    and .total > 0
    and .failed == 0
    and .success == .total
    and all(
      .results[];
      .version == $latest
      and .error == null
      and (.status == "migrated" or .status == "already_migrated")
    )
  ' <"$migration_response" >/dev/null; then
    print_migration_verification_failure <"$migration_response" ||
        echo "Tenant migration diagnostic could not be rendered"
    echo "Tenant migration verification failed; maintenance remains enabled"
    exit 1
fi
schoolorbit_timer_report tenant_migration "$tenant_migration_started"

migration_status_started="$(schoolorbit_timer_now)"
if ! curl \
    --fail \
    --silent \
    --show-error \
    --connect-timeout 10 \
    --max-time 120 \
    -H "X-Internal-Secret: ${internal_api_secret}" \
    -o "$migration_response" \
    http://127.0.0.1:8081/internal/migration-status; then
    echo "Tenant migration status request failed; maintenance remains enabled"
    exit 1
fi
migration_completion_filter="
    .latest_version as \$latest
    | \$latest >= 84
    and .total_schools > 0
    and (.schools | length) == .total_schools
    and .migrated == .total_schools
    and .pending == 0 and .failed == 0 and .outdated == 0
    and all(
      .schools[];
      .migration_version == \$latest
      and .migration_status == \"migrated\"
      and .migration_error == null
      and .academicCoreCutover.migrationVersion == 45
      and .academicCoreCutover.status == \"cleanupCompleted\"
      and .academicCoreCutover.passed == true
      and all(.academicCoreCutover.checks[]; .passed == true)
      and .gradebookResultsCutover.migrationVersion == 60
      and .gradebookResultsCutover.status == \"cutoverCompleted\"
      and .gradebookResultsCutover.passed == true
      and all(.gradebookResultsCutover.checks[]; .passed == true)
      and .deliveryTimetableCutover.migrationVersion == 88
      and .deliveryTimetableCutover.status == \"cutoverCompleted\"
      and .deliveryTimetableCutover.passed == true
      and (.deliveryTimetableCutover.checks | length) == 30
      and all(.deliveryTimetableCutover.checks[]; .passed == true)
    )
  "
if ! podman run --rm -i "$jq_image" -e "$migration_completion_filter" <"$migration_response" >/dev/null; then
    echo "Tenant migration or cutover audit verification failed; maintenance remains enabled"
    exit 1
fi
schoolorbit_timer_report migration_status "$migration_status_started"
unset internal_api_secret
rm -f "$migration_response"
trap - EXIT

[ -z "$proxy_backup" ] || rm -f "$proxy_backup"
echo "School API remains in maintenance until the authenticated smoke completes"
