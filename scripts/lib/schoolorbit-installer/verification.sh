#!/usr/bin/env bash

VERIFICATION_MODULE_DIRECTORY=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
VERIFICATION_REPOSITORY_ROOT=${SCHOOLORBIT_REPOSITORY_ROOT:-$(cd -- "$VERIFICATION_MODULE_DIRECTORY/../../.." && pwd)}

_verification_temp_file() {
    local temporary
    umask 077
    temporary=$(mktemp "${TMPDIR:-/tmp}/schoolorbit-verify.XXXXXX")
    chmod 0600 "$temporary"
    printf '%s\n' "$temporary"
}

_verify_https_json() {
    local host=$1 path=$2 mode=$3 jq_filter=$4 description=$5
    local body status
    local -a curl_options=(
        --silent
        --show-error
        --connect-timeout 10
        --max-time 30
        --output
    )
    body=$(_verification_temp_file) || return
    curl_options+=("$body" --write-out '%{http_code}')
    if [[ $mode == direct ]]; then
        [[ -r ${SO_CF_ORIGIN_ROOT_FILE-} ]] || {
            command rm -f "$body"
            die 78 'Verified Cloudflare Origin CA root is unavailable'
            return
        }
        curl_options+=(
            --cacert "$SO_CF_ORIGIN_ROOT_FILE"
            --resolve "$host:443:${SO_CONFIG[target]}"
        )
    fi

    if ! status=$(curl "${curl_options[@]}" "https://$host$path"); then
        command rm -f "$body"
        die 69 "$description request failed"
        return
    fi
    if [[ $status != 200 ]]; then
        command rm -f "$body"
        die 69 "$description returned HTTP $status"
        return
    fi
    if ! jq -e "$jq_filter" "$body" >/dev/null 2>&1; then
        command rm -f "$body"
        die 78 "$description identity or readiness response is invalid"
        return
    fi
    command rm -f "$body"
}

_verify_api_services() {
    local mode=$1
    local admin_host="admin-api.${SO_CONFIG[base_domain]}"
    local school_host="school-api.${SO_CONFIG[base_domain]}"
    _verify_https_json "$admin_host" / "$mode" \
        '.service == "SchoolOrbit Backend Admin"' 'Backend Admin identity' || return
    _verify_https_json "$admin_host" /ready "$mode" \
        '.status == "ready" and .database == "connected"' 'Backend Admin readiness' || return
    _verify_https_json "$school_host" / "$mode" \
        '.service == "SchoolOrbit Backend School"' 'Backend School identity' || return
    _verify_https_json "$school_host" /ready "$mode" \
        '.status == "ready" and .controlPlane == "connected" and .filePlatform == "ready"' \
        'Backend School readiness'
}

_verify_remote_runtime() {
    local remote_command
    if [[ ${SO_CONFIG[bootstrap_user]} == root ]]; then
        remote_command="runuser -u ${SO_CONFIG[server_user]} -- bash -s"
    else
        remote_command="sudo -niu ${SO_CONFIG[server_user]} bash -s"
    fi

    _vps_ssh "$remote_command" <<'REMOTE_SCRIPT'
set -euo pipefail
cd /opt/stack
podman-compose -f podman-compose.yml config >/dev/null
podman exec schoolorbit-nginx nginx -t
deadline=$((SECONDS + 720))
for container in schoolorbit-backend-admin schoolorbit-backend-school schoolorbit-clamd schoolorbit-nginx; do
    while true; do
        status=$(podman inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container")
        case "$status" in
            healthy) break ;;
            running)
                [[ $container == schoolorbit-nginx ]] && break
                ;;
            unhealthy | exited | dead)
                printf 'Container %s entered terminal state %s\n' "$container" "$status" >&2
                exit 1
                ;;
        esac
        ((SECONDS < deadline)) || {
            printf 'Timed out waiting for container %s\n' "$container" >&2
            exit 75
        }
        sleep 5
    done
done
REMOTE_SCRIPT
}

verify_direct_origin() {
    require_command curl || return
    require_command jq || return
    _verify_remote_runtime || die 69 'Target runtime verification failed' || return
    _verify_api_services direct
}

_verify_html_service() {
    local url=$1 description=$2 body status
    body=$(_verification_temp_file) || return
    if ! status=$(curl --silent --show-error --connect-timeout 10 --max-time 30 \
        --output "$body" --write-out '%{http_code}' "$url"); then
        command rm -f "$body"
        die 69 "$description request failed"
        return
    fi
    if [[ $status != 200 ]] || ! grep -Eiq '<!doctype[[:space:]]+html' "$body"; then
        command rm -f "$body"
        die 78 "$description did not return the expected HTML"
        return
    fi
    command rm -f "$body"
}

verify_public_services() {
    require_command curl || return
    require_command jq || return
    local smoke_subdomain=${SO_SECRETS[SMOKE_SUBDOMAIN]-}
    local smoke_username=${SO_SECRETS[SMOKE_USERNAME]-}
    local smoke_password=${SO_SECRETS[SMOKE_PASSWORD]-}
    local smoke_script=${SCHOOLORBIT_SMOKE_SCRIPT:-$VERIFICATION_REPOSITORY_ROOT/scripts/smoke_test.sh}
    local smoke_output
    [[ -n $smoke_subdomain && -n $smoke_username && -n $smoke_password ]] || die 64 'Authenticated smoke credentials are required' || return
    [[ -x $smoke_script ]] || die 69 'Smoke test script is unavailable' || return

    _verify_api_services public || return
    _verify_html_service "https://admin.${SO_CONFIG[base_domain]}/" 'Frontend Admin' || return
    _verify_html_service "https://$smoke_subdomain.${SO_CONFIG[base_domain]}/" 'Tenant frontend' || return

    if ! smoke_output=$(
        export SMOKE_SUBDOMAIN=$smoke_subdomain
        export SMOKE_API_URL="https://school-api.${SO_CONFIG[base_domain]}"
        export SMOKE_ADMIN_API_URL="https://admin-api.${SO_CONFIG[base_domain]}"
        export SMOKE_TENANT_URL="https://$smoke_subdomain.${SO_CONFIG[base_domain]}"
        export SMOKE_ORIGIN=$SMOKE_TENANT_URL
        export SMOKE_USERNAME=$smoke_username
        export SMOKE_PASSWORD=$smoke_password
        export SMOKE_REQUIRE_AUTH=true
        [[ -z ${FILE_SMOKE_PNG-} ]] || export FILE_SMOKE_PNG
        "$smoke_script" 2>&1
    ); then
        warn "$smoke_output"
        die 78 'Authenticated public smoke verification failed'
        return
    fi
    info "$smoke_output"
}
