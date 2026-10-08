#!/usr/bin/env bash
set -euo pipefail
mode=${1:?enter, verify or accept is required}
: "${BASE_DOMAIN:?}"
: "${RELEASE_SHA:?}"
[[ $RELEASE_SHA =~ ^[0-9a-f]{40}$ ]] || exit 64
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
root="$stack/deployment"
renderer="$root/scripts/render_nginx_config.sh"
mkdir -p "$root/releases/$RELEASE_SHA"
journal="$root/releases/$RELEASE_SHA"
public_status() {
    local part host status body
    for part in school admin; do
        host="${part}-api.${BASE_DOMAIN}"
        body=$(mktemp)
        status=$(curl --silent --show-error --max-time 30 --cacert "$stack/nginx/ssl/cloudflare-origin-rsa-root.pem" \
            --resolve "${host}:443:127.0.0.1" -o "$body" -w '%{http_code}' "https://${host}/ready")
        if [[ $1 == maintenance ]]; then
            if [[ $status != 503 ]] || ! jq -e '.error == "maintenance"' "$body" >/dev/null; then
                rm -f "$body"
                return 1
            fi
        else
            if [[ $status != 200 ]] || ! jq -e '.status == "ready"' "$body" >/dev/null; then
                rm -f "$body"
                return 1
            fi
        fi
        rm -f "$body"
    done
}
if [[ $mode == enter ]]; then
    if [[ ! -f $journal/probe-token ]]; then
        umask 077
        openssl rand -hex 32 >"$journal/probe-token"
    fi
    probe=$(cat "$journal/probe-token")
    for part in school admin; do
        source="$root/nginx-configs/${part}-api.maintenance.conf.template"
        "$renderer" "$source" "$journal/${part}-maintenance.conf" "$BASE_DOMAIN" "$RELEASE_SHA" maintenance "$probe"
        chmod 0600 "$journal/${part}-maintenance.conf"
        target="$stack/nginx/conf.d/${part}-api.conf"
        legacy="$stack/nginx/conf.d/${part}-api.${BASE_DOMAIN}.conf"
        # The previous tracked owner used a domain-suffixed Admin filename.
        [[ ! -f $legacy ]] || rm -f "$legacy"
        install -m 0600 "$journal/${part}-maintenance.conf" "$target"
        if ! podman container exists "schoolorbit-backend-${part}"; then
            # A fresh origin must serve 503 before either backend exists. Nginx
            # resolves literal upstream names at configuration load; omit only
            # private upstream locations until that backend is created.
            sed -i -e '/error_page 418/d' -e '/if (.*release_probe = 1)/d' \
                -e '/    location @release_probe {/,/^    }/d' \
                -e '/    location \^~ \/internal\/ {/,/^    }/d' "$target"
        fi
    done
    printf '%s\n' "$RELEASE_SHA" >"$root/pending-release"
    if ! podman container exists schoolorbit-nginx; then
        test -f "$stack/.env"
        cp "$root/podman-compose.yml" "$stack/podman-compose.yml"
        podman-compose -f "$stack/podman-compose.yml" up -d --no-deps nginx >/dev/null 2>&1
    else
        podman start schoolorbit-nginx >/dev/null
    fi
    podman exec schoolorbit-nginx nginx -t
    podman exec schoolorbit-nginx nginx -s reload
    public_status maintenance
    echo 'Public School and Admin APIs are in maintenance.'
elif [[ $mode == accept || $mode == verify ]]; then
    test "$(cat "$root/pending-release")" = "$RELEASE_SHA"
    probe=$(cat "$journal/probe-token")
    origin_root=${stack}/nginx/ssl/cloudflare-origin-rsa-root.pem
    for part in school admin; do
        host="${part}-api.${BASE_DOMAIN}"
        response=$(curl --fail --silent --show-error --max-time 30 --cacert "$origin_root" \
            --resolve "${host}:443:127.0.0.1" -H "X-Schoolorbit-Release-Probe: $probe" "https://${host}/ready")
        printf '%s' "$response" | jq -e '.status == "ready"' >/dev/null
    done
    SMOKE_SUBDOMAIN=sandbox SMOKE_API_URL="https://school-api.${BASE_DOMAIN}" \
        SMOKE_ADMIN_API_URL="https://admin-api.${BASE_DOMAIN}" SMOKE_TENANT_URL="https://sandbox.${BASE_DOMAIN}" \
        SMOKE_ORIGIN="https://sandbox.${BASE_DOMAIN}" SMOKE_REQUIRE_AUTH=true SMOKE_ACADEMIC_CONTEXT=true \
        SMOKE_DIRECT_BACKEND=false SMOKE_RESOLVE_IP=127.0.0.1 SMOKE_CA_CERT="$origin_root" \
        SMOKE_RELEASE_PROBE_TOKEN="$probe" bash "$root/scripts/smoke_test.sh"
    if [[ $mode == verify ]]; then
        if [[ -n ${PREPARED_IMAGES-} ]]; then
            printf '%s' "$PREPARED_IMAGES" | jq -e 'type == "object"' >/dev/null
            while IFS='|' read -r component digest; do
                case "$component" in backend-admin | backend-school) ;; *) exit 64 ;; esac
                [[ $digest =~ ^sha256:[0-9a-f]{64}$ ]] || exit 64
                repository="ghcr.io/akephisit/schoolorbit-${component}"
                podman tag "${repository}@${digest}" "${repository}:${RELEASE_SHA}"
                podman tag "${repository}@${digest}" "${repository}:latest"
                bash "$root/scripts/prune_runtime_images.sh" "$repository" 3
            done < <(printf '%s' "$PREPARED_IMAGES" | jq -er 'to_entries[] | [.key,.value] | join("|")')
        fi
        echo "Private proxy readiness and authenticated smoke passed; maintenance remains enabled."
        exit 0
    fi
    restore_maintenance() {
        for part in school admin; do install -m 0600 "$journal/${part}-maintenance.conf" "${stack}/nginx/conf.d/${part}-api.conf"; done
        podman exec schoolorbit-nginx nginx -t
        podman exec schoolorbit-nginx nginx -s reload
    }
    # A failed renderer, ledger write or interrupted acceptance must not leave
    # partially normal configurations for a later proxy restart.
    trap 'status=$?; if ((status != 0)); then restore_maintenance; fi' EXIT
    for part in school admin; do
        "$renderer" "$root/nginx-configs/${part}-api.conf.template" "${stack}/nginx/conf.d/${part}-api.conf" "$BASE_DOMAIN" "$RELEASE_SHA" ready
    done
    if ! podman exec schoolorbit-nginx nginx -t || ! podman exec schoolorbit-nginx nginx -s reload; then
        restore_maintenance
        exit 1
    fi
    if ! public_status ready; then
        restore_maintenance
        echo 'Public release readiness failed; maintenance restored' >&2
        exit 1
    fi
    # Registry aliases and GitHub baselines were published before this final step.
    printf '%s\n' "$RELEASE_SHA" >"$root/accepted-release"
    rm -f "$root/pending-release"
    echo 'All selected release checks passed; public maintenance is disabled.'
else
    exit 64
fi
