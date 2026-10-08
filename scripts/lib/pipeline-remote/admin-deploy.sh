#!/usr/bin/env bash
set -eu
REGISTRY_TOKEN=${REGISTRY_TOKEN:?REGISTRY_TOKEN is required}
REGISTRY_USER=${REGISTRY_USER:?REGISTRY_USER is required}
BASE_DOMAIN=${BASE_DOMAIN:?BASE_DOMAIN is required}
RELEASE_SHA=${RELEASE_SHA:?RELEASE_SHA is required}
BACKEND_IMAGE_DIGEST=${BACKEND_IMAGE_DIGEST:?BACKEND_IMAGE_DIGEST is required}
echo "${REGISTRY_TOKEN}" | podman login ghcr.io -u "${REGISTRY_USER}" --password-stdin
cd /opt/stack

base_domain="${BASE_DOMAIN}"
deployment_root=/opt/stack/deployment
runtime_source="$deployment_root/podman-compose.yml"
runtime_compose=/opt/stack/podman-compose.yml
renderer_source="$deployment_root/scripts/render_nginx_config.sh"
renderer="$deployment_root/render-nginx"
origin_root_installer_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/install_origin_root.sh"
origin_root_installer="$deployment_root/install-origin-root"
network_helper_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh"
timing_helper_source="$deployment_root/scripts/lib/schoolorbit-installer/remote/deployment_timing.sh"
image_cleanup="$deployment_root/scripts/prune_runtime_images.sh"
proxy_template="$deployment_root/nginx-configs/admin-api.conf.template"
proxy_target=/opt/stack/nginx/conf.d/admin-api.conf
legacy_proxy_target=/opt/stack/nginx/conf.d/admin-api.${base_domain}.conf
origin_root=/opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem
compose_up_quiet() {
    podman-compose -f "$runtime_compose" up -d "$@" >/dev/null 2>&1
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

test -f /opt/stack/.env || {
    echo "Backend runtime environment is missing"
    exit 1
}
test -f "$runtime_source" || {
    echo "Canonical runtime definition is missing"
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
test -f "$proxy_template" || {
    echo "Admin API proxy template is missing"
    exit 1
}

# shellcheck source=scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh
. "$network_helper_source"
# shellcheck source=scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
. "$timing_helper_source"
reconnect_backend_network() {
    container=$1
    service_alias=$2
    schoolorbit_ensure_container_network_aliases \
        schoolorbit-web "$container" "$service_alias" "$container" schoolorbit-nginx
}

install -m 0755 "$renderer_source" "$renderer"
install -m 0755 "$origin_root_installer_source" "$origin_root_installer"
"$origin_root_installer" "$origin_root"
cp "$runtime_source" "${runtime_compose}.next"
podman-compose -f "${runtime_compose}.next" --dry-run up -d backend-admin >/dev/null 2>&1
mv "${runtime_compose}.next" "$runtime_compose"

proxy_next="$(mktemp /opt/stack/nginx/conf.d/.admin-api.conf.XXXXXX)"
"$renderer" "$proxy_template" "$proxy_next" "$base_domain" "${RELEASE_SHA}" ready
chmod 0644 "$proxy_next"

proxy_matches="$(
    grep -lF "server_name admin-api.${base_domain};" \
        /opt/stack/nginx/conf.d/*.conf 2>/dev/null || true
)"
proxy_match_count="$(printf '%s\n' "$proxy_matches" | sed '/^$/d' | wc -l | tr -d ' ')"
if [ "$proxy_match_count" -gt 1 ]; then
    rm -f "$proxy_next"
    echo "Multiple active admin API proxy configs were found"
    exit 1
fi
if [ "$proxy_match_count" -eq 1 ] &&
    [ "$proxy_matches" != "$proxy_target" ] &&
    [ "$proxy_matches" != "$legacy_proxy_target" ]; then
    rm -f "$proxy_next"
    echo "Admin API proxy exists at an unexpected path: ${proxy_matches}"
    exit 1
fi
proxy_previous_target="$proxy_target"
if [ "$proxy_match_count" -eq 1 ]; then
    proxy_previous_target="$proxy_matches"
fi

backend_image=ghcr.io/akephisit/schoolorbit-backend-admin
backend_image_digest="${BACKEND_IMAGE_DIGEST}"
printf '%s' "$backend_image_digest" | grep -Eq '^sha256:[0-9a-f]{64}$'
backend_image_reference="${backend_image}@${backend_image_digest}"
image_pull_started="$(schoolorbit_timer_now)"
podman pull "$backend_image_reference"
schoolorbit_timer_report image_pull "$image_pull_started"
backend_readiness_started="$(schoolorbit_timer_now)"
export BACKEND_ADMIN_IMAGE="$backend_image_reference"
podman stop schoolorbit-backend-admin || true
podman rm schoolorbit-backend-admin || true
compose_up_quiet --no-deps backend-admin
unset BACKEND_ADMIN_IMAGE
reconnect_backend_network schoolorbit-backend-admin backend-admin

# The child shell expands its own positional arguments.
# shellcheck disable=SC2016
if ! timeout 180 bash -c '
  for attempt in $(seq 1 36); do
    if curl -fsS "$1" >/dev/null; then
      exit 0
    fi
    if [ "$attempt" -lt 36 ]; then sleep 5; fi
  done
  exit 1
' _ http://127.0.0.1:8080/ready; then
    rm -f "$proxy_next"
    podman logs --tail 100 schoolorbit-backend-admin
    exit 1
fi
schoolorbit_timer_report backend_readiness "$backend_readiness_started"

proxy_backup=""
proxy_existed=0
if [ -f "$proxy_previous_target" ]; then
    proxy_backup="$(mktemp /opt/stack/nginx/conf.d/.admin-api.backup.XXXXXX)"
    cp "$proxy_previous_target" "$proxy_backup"
    proxy_existed=1
fi
restore_proxy() {
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
mv "$proxy_next" /opt/stack/deployment/admin-api.conf
cp "/opt/stack/deployment/releases/${RELEASE_SHA}/admin-maintenance.conf" "$proxy_target"
if ! recreate_nginx ||
    ! validate_nginx_config_with_retry ||
    ! podman exec schoolorbit-nginx nginx -s reload; then
    restore_proxy
    echo "Admin API proxy configuration could not be activated"
    exit 1
fi

admin_host="admin-api.${base_domain}"
origin_verification_started="$(schoolorbit_timer_now)"
probe=$(cat "/opt/stack/deployment/releases/${RELEASE_SHA}/probe-token")
direct_admin_ok=false
for attempt in $(seq 1 12); do
    if direct_ready="$(
        curl --fail --silent --show-error --max-time 10 \
            --cacert "$origin_root" \
            --resolve "${admin_host}:443:127.0.0.1" \
            -H "X-Schoolorbit-Release-Probe: $probe" "https://${admin_host}/ready"
    )" && printf '%s' "$direct_ready" | jq -e \
        '.status == "ready" and .database == "connected"' >/dev/null &&
        direct_identity="$(
            curl --fail --silent --show-error --max-time 10 \
                --cacert "$origin_root" \
                --resolve "${admin_host}:443:127.0.0.1" \
                -H "X-Schoolorbit-Release-Probe: $probe" "https://${admin_host}/"
        )" && printf '%s' "$direct_identity" | jq -e \
        '.service == "SchoolOrbit Backend Admin"' >/dev/null; then
        direct_admin_ok=true
        break
    fi
    if [ "$attempt" -lt 12 ]; then
        sleep 2
    fi
done
if [ "$direct_admin_ok" != true ]; then
    restore_proxy
    echo "Direct admin API verification failed"
    exit 1
fi
schoolorbit_timer_report origin_verification "$origin_verification_started"

[ -z "$proxy_backup" ] || rm -f "$proxy_backup"
echo "Admin backend passed origin acceptance; maintenance remains enabled."
