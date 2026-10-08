#!/usr/bin/env bash
set -eu
BASE_DOMAIN=${BASE_DOMAIN:?BASE_DOMAIN is required}
smoke_subdomain=${ACADEMIC_CORE_SMOKE_SUBDOMAIN:-}
case "$smoke_subdomain" in
    '' | *[!a-z0-9-]* | -* | *-)
        echo "Academic Core smoke subdomain is invalid"
        exit 1
        ;;
esac
if [ "${#smoke_subdomain}" -gt 63 ]; then
    echo "Academic Core smoke subdomain is invalid"
    exit 1
fi
if [ -z "${SMOKE_USERNAME:-}" ] || [ -z "${SMOKE_PASSWORD:-}" ]; then
    echo "Academic Core maintenance smoke credentials are unavailable"
    exit 1
fi

base_domain="${BASE_DOMAIN}"
deployment_root=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment
smoke_script=$deployment_root/scripts/smoke_test.sh
timing_helper_source=$deployment_root/scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
test -f "$smoke_script" || {
    echo "Academic Core maintenance smoke script is unavailable"
    exit 1
}
test -f "$timing_helper_source" || {
    echo "Deployment timing helper is unavailable"
    exit 1
}
# shellcheck source=scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
. "$timing_helper_source"

authenticated_smoke_started="$(schoolorbit_timer_now)"
SMOKE_SUBDOMAIN="$smoke_subdomain" \
    SMOKE_API_URL=http://localhost:8081 \
    SMOKE_ADMIN_API_URL=http://localhost:8080 \
    SMOKE_TENANT_URL="https://${smoke_subdomain}.${base_domain}" \
    SMOKE_ORIGIN="https://${smoke_subdomain}.${base_domain}" \
    SMOKE_REQUIRE_AUTH=true \
    SMOKE_ACADEMIC_CONTEXT=true \
    SMOKE_DIRECT_BACKEND=true \
    SMOKE_USERNAME="$SMOKE_USERNAME" \
    SMOKE_PASSWORD="$SMOKE_PASSWORD" \
    bash "$smoke_script"
schoolorbit_timer_report authenticated_smoke "$authenticated_smoke_started"
unset SMOKE_USERNAME SMOKE_PASSWORD
