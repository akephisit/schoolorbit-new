#!/usr/bin/env bash
set -euo pipefail

backend_admin_url="${BACKEND_ADMIN_URL:-}"
internal_api_secret="${INTERNAL_API_SECRET:-}"
github_output="${GITHUB_OUTPUT:-}"
max_attempts="${TENANT_DISCOVERY_MAX_ATTEMPTS:-120}"
retry_delay_seconds="${TENANT_DISCOVERY_RETRY_DELAY_SECONDS:-10}"

if [[ -z "$backend_admin_url" ]]; then
    printf 'BACKEND_ADMIN_URL is required for tenant discovery.\n' >&2
    exit 1
fi

if [[ -z "$internal_api_secret" ]]; then
    printf 'INTERNAL_API_SECRET is required for tenant discovery.\n' >&2
    exit 1
fi

if [[ -z "$github_output" ]]; then
    printf 'GITHUB_OUTPUT is required for tenant discovery.\n' >&2
    exit 1
fi

if [[ ! "$max_attempts" =~ ^[0-9]+$ ]] || ((max_attempts < 1 || max_attempts > 120)); then
    printf 'TENANT_DISCOVERY_MAX_ATTEMPTS must be between 1 and 120.\n' >&2
    exit 1
fi

if [[ ! "$retry_delay_seconds" =~ ^[0-9]+$ ]] || ((retry_delay_seconds > 30)); then
    printf 'TENANT_DISCOVERY_RETRY_DELAY_SECONDS must be between 0 and 30.\n' >&2
    exit 1
fi

for command in curl node; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for tenant discovery.\n' "$command" >&2
        exit 1
    fi
done

response_file="$(mktemp)"
chmod 600 "$response_file"
cleanup() {
    rm -f "$response_file"
}
trap cleanup EXIT

http_status=""
for ((attempt = 1; attempt <= max_attempts; attempt++)); do
    request_succeeded=1
    if ! http_status="$(
        curl \
            --silent \
            --show-error \
            --connect-timeout 10 \
            --max-time 30 \
            --output "$response_file" \
            --write-out '%{http_code}' \
            --header "X-Internal-Secret: ${internal_api_secret}" \
            --header "X-Internal-Caller: deploy-all-schools" \
            "${backend_admin_url%/}/internal/schools?status=active"
    )"; then
        request_succeeded=0
        http_status="000"
    fi

    if [[ "$request_succeeded" -eq 1 && "$http_status" == "200" ]]; then
        break
    fi

    retryable=0
    if [[ "$request_succeeded" -eq 0 ||
        "$http_status" == "404" ||
        "$http_status" == "429" ||
        "$http_status" =~ ^5[0-9][0-9]$ ]]; then
        retryable=1
    fi

    if [[ "$retryable" -eq 1 && "$attempt" -lt "$max_attempts" ]]; then
        printf 'Tenant discovery is not ready (attempt %s/%s); retrying.\n' \
            "$attempt" "$max_attempts" >&2
        sleep "$retry_delay_seconds"
        continue
    fi

    if [[ "$request_succeeded" -eq 0 ]]; then
        printf 'Tenant discovery could not reach backend-admin.\n' >&2
    else
        printf 'Tenant discovery failed with status %s.\n' "$http_status" >&2
    fi
    exit 1
done

if ! schools="$(
    node --input-type=module - "$response_file" <<'NODE'
import { readFileSync } from 'node:fs';

const responsePath = process.argv[2];
const response = JSON.parse(readFileSync(responsePath, 'utf8'));
if (!Array.isArray(response.schools)) {
    throw new Error('schools must be an array');
}

const subdomainPattern = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;
const subdomains = response.schools.map((school) => school?.subdomain);
if (subdomains.length === 0) {
    throw new Error('no active school tenants were returned');
}
if (subdomains.some((subdomain) => (
    typeof subdomain !== 'string' || !subdomainPattern.test(subdomain)
))) {
    throw new Error('a school tenant has an invalid subdomain');
}
if (new Set(subdomains).size !== subdomains.length) {
    throw new Error('duplicate school tenant subdomains were returned');
}

process.stdout.write(JSON.stringify(subdomains.map((subdomain) => ({ subdomain }))));
NODE
)"; then
    printf 'Tenant discovery returned an invalid response.\n' >&2
    exit 1
fi

printf 'schools=%s\n' "$schools" >> "$github_output"
printf 'Tenant discovery succeeded.\n'
