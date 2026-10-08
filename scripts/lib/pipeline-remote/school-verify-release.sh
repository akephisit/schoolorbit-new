#!/usr/bin/env bash
set -eu
BASE_DOMAIN=${BASE_DOMAIN:?BASE_DOMAIN is required}
RELEASE_SHA=${RELEASE_SHA:?RELEASE_SHA is required}
BACKEND_IMAGE_DIGEST=${BACKEND_IMAGE_DIGEST:?BACKEND_IMAGE_DIGEST is required}
RELEASE_ID=${RELEASE_ID:?RELEASE_ID is required}
base_domain="${BASE_DOMAIN}"
smoke_subdomain=${ACADEMIC_CORE_SMOKE_SUBDOMAIN:-sandbox}
proxy_source=/opt/stack/deployment/school-api.conf
maintenance_proxy_source=/opt/stack/deployment/school-api.maintenance.conf
proxy_target=/opt/stack/nginx/conf.d/school-api.conf
origin_root=/opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem
smoke_script=/opt/stack/deployment/scripts/smoke_test.sh
timing_helper_source=/opt/stack/deployment/scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
image_cleanup=/opt/stack/deployment/scripts/prune_runtime_images.sh
test -f "$proxy_source"
test -f "$maintenance_proxy_source"
test -f "$origin_root"
test -f "$smoke_script"
test -f "$timing_helper_source"
test -x "$image_cleanup"
# shellcheck source=scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
. "$timing_helper_source"
proxy_cutover_started="$(schoolorbit_timer_now)"

restore_maintenance() {
    cp "$maintenance_proxy_source" "$proxy_target"
    podman exec schoolorbit-nginx nginx -t
    podman exec schoolorbit-nginx nginx -s reload || true
}

cp "$maintenance_proxy_source" "$proxy_target"
if ! podman exec schoolorbit-nginx nginx -t; then
    restore_maintenance
    echo "Normal school API proxy configuration is invalid; maintenance remains enabled"
    exit 1
fi
if ! podman exec schoolorbit-nginx nginx -s reload; then
    restore_maintenance
    echo "Normal school API proxy reload failed; maintenance remains enabled"
    exit 1
fi

school_host="school-api.${base_domain}"
probe=$(cat "/opt/stack/deployment/releases/${RELEASE_SHA}/probe-token")
direct_ready_ok=false
for attempt in $(seq 1 12); do
    if direct_ready="$(
        curl --fail --silent --show-error --max-time 15 \
            --cacert "$origin_root" \
            --resolve "${school_host}:443:127.0.0.1" \
            -H "X-Schoolorbit-Release-Probe: $probe" "https://${school_host}/ready"
    )" && printf '%s' "$direct_ready" | jq -e \
        '.status == "ready" and .controlPlane == "connected" and .filePlatform == "ready"' >/dev/null; then
        direct_ready_ok=true
        break
    fi
    [ "$attempt" -eq 12 ] || sleep 2
done
if [ "$direct_ready_ok" != true ]; then
    restore_maintenance
    echo "Direct school API readiness failed; maintenance restored"
    exit 1
fi

if ! SMOKE_SUBDOMAIN="$smoke_subdomain" \
    SMOKE_API_URL="https://${school_host}" \
    SMOKE_ADMIN_API_URL="https://admin-api.${base_domain}" \
    SMOKE_TENANT_URL="https://${smoke_subdomain}.${base_domain}" \
    SMOKE_ORIGIN="https://${smoke_subdomain}.${base_domain}" \
    SMOKE_REQUIRE_AUTH=true \
    SMOKE_ACADEMIC_CONTEXT=true \
    SMOKE_DIRECT_BACKEND=false \
    SMOKE_RESOLVE_IP=127.0.0.1 \
    SMOKE_CA_CERT="$origin_root" \
    SMOKE_RELEASE_PROBE_TOKEN="$probe" \
    SMOKE_USERNAME="$SMOKE_USERNAME" \
    SMOKE_PASSWORD="$SMOKE_PASSWORD" \
    bash "$smoke_script"; then
    restore_maintenance
    echo "Public release smoke failed; maintenance restored"
    exit 1
fi

backend_image=ghcr.io/akephisit/schoolorbit-backend-school
backend_image_digest="${BACKEND_IMAGE_DIGEST}"
if ! printf '%s' "$backend_image_digest" | grep -Eq '^sha256:[0-9a-f]{64}$'; then
    restore_maintenance
    echo "Accepted backend image digest is invalid; maintenance restored"
    exit 1
fi
backend_image_reference="${backend_image}@${backend_image_digest}"
podman tag "$backend_image_reference" "${backend_image}:${RELEASE_ID}"
echo "School image verified; global acceptance owns latest tags and cleanup."
schoolorbit_timer_report proxy_cutover "$proxy_cutover_started"
unset SMOKE_USERNAME SMOKE_PASSWORD
echo "School API release ${RELEASE_ID} is ready"
