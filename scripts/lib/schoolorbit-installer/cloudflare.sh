#!/usr/bin/env bash

CF_API_BASE_URL=${CF_API_BASE_URL:-https://api.cloudflare.com/client/v4}
CF_CERTIFICATE=
SO_CF_ZONE_ID=
SO_CF_ACCOUNT_ID=
SO_CF_ADMIN_RECORD_ID=
SO_CF_SCHOOL_RECORD_ID=
SO_CF_CERTIFICATE_ID=
SO_CF_CERTIFICATE_EXPIRES=
SO_CF_DNS_RECORDS=
SO_DNS_SNAPSHOT=
SO_DNS_SNAPSHOT_ETAG=
SO_DNS_CURRENT_ETAG=
SO_DNS_ORIGINAL_IP=

_cf_temporary_file() {
    local temporary
    umask 077
    temporary=$(mktemp "${TMPDIR:-/tmp}/schoolorbit-cf.XXXXXX")
    chmod 0600 "$temporary"
    printf '%s\n' "$temporary"
}

_cf_request() {
    local method=$1 path=$2 response_file=$3 request_file=${4-}
    local header_file errors
    local -a curl_args
    header_file=$(_cf_temporary_file) || return
    printf 'Authorization: Bearer %s\n' "${SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]-}" >"$header_file"
    curl_args=(
        --silent --show-error --fail-with-body
        --request "$method"
        --header "@$header_file"
        --header 'Content-Type: application/json'
        --output "$response_file"
    )
    [[ -z $request_file ]] || curl_args+=(--data-binary "@$request_file")
    curl_args+=("$CF_API_BASE_URL$path")

    if ! curl "${curl_args[@]}"; then
        command rm -f "$header_file"
        die 69 'Cloudflare API request failed'
        return
    fi
    command rm -f "$header_file"

    if ! jq -e '.success == true' "$response_file" >/dev/null 2>&1; then
        errors=$(jq -r '[.errors[]? | "\(.code): \(.message)"] | join("; ")' "$response_file" 2>/dev/null) || errors='invalid provider response'
        warn "Cloudflare API rejected the request: $errors"
        return 69
    fi
}

_cf_refresh_dns() {
    local response admin_host school_host records
    [[ -n $SO_CF_ZONE_ID ]] || die 78 'Cloudflare zone is not loaded' || return
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/zones/$SO_CF_ZONE_ID/dns_records?per_page=5000000" "$response"; then
        command rm -f "$response"
        return 69
    fi
    admin_host="admin-api.${SO_CONFIG[base_domain]}"
    school_host="school-api.${SO_CONFIG[base_domain]}"
    records=$(jq -cer --arg admin "$admin_host" --arg school "$school_host" '
        [.result[] | select(.name == $admin or .name == $school)]
        | if length != 2 then error("ambiguous API DNS records") else . end
        | if (map(select(.name == $admin)) | length) != 1 or
             (map(select(.name == $school)) | length) != 1
          then error("ambiguous API DNS names") else . end
        | if any(.[]; .type != "A") then error("API DNS records must be A records") else . end
        | map({id,type,name,content,ttl,proxied,modified_on})
        | sort_by(.name)
    ' "$response" 2>/dev/null) || {
        command rm -f "$response"
        die 78 'Cloudflare API DNS records are missing or ambiguous'
        return
    }
    command rm -f "$response"

    SO_CF_DNS_RECORDS=$records
    # shellcheck disable=SC2034 # Provider outputs are consumed by orchestration.
    SO_CF_ADMIN_RECORD_ID=$(jq -er --arg name "$admin_host" '.[] | select(.name == $name) | .id' <<<"$records") || return 78
    # shellcheck disable=SC2034
    SO_CF_SCHOOL_RECORD_ID=$(jq -er --arg name "$school_host" '.[] | select(.name == $name) | .id' <<<"$records") || return 78
}

cf_preflight() {
    require_command curl || return
    require_command jq || return
    local response count ssl_response
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/zones?name=${SO_CONFIG[base_domain]}&status=active&match=all" "$response"; then
        command rm -f "$response"
        return 69
    fi
    count=$(jq --arg name "${SO_CONFIG[base_domain]}" '[.result[] | select(.name == $name and .status == "active")] | length' "$response") || {
        command rm -f "$response"
        die 69 'Cloudflare returned an invalid zone response'
        return
    }
    if ((count != 1)); then
        command rm -f "$response"
        die 78 'Cloudflare zone lookup must return exactly one active zone'
        return
    fi
    SO_CF_ZONE_ID=$(jq -er --arg name "${SO_CONFIG[base_domain]}" '.result[] | select(.name == $name and .status == "active") | .id' "$response") || {
        command rm -f "$response"
        return 69
    }
    # shellcheck disable=SC2034 # Used by GitHub configuration and VPS runtime rendering.
    SO_CF_ACCOUNT_ID=$(jq -er --arg name "${SO_CONFIG[base_domain]}" '.result[] | select(.name == $name and .status == "active") | .account.id | strings | select(length > 0)' "$response") || {
        command rm -f "$response"
        die 69 'Cloudflare zone account ID is missing'
        return
    }
    command rm -f "$response"

    ssl_response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/zones/$SO_CF_ZONE_ID/settings/ssl" "$ssl_response"; then
        command rm -f "$ssl_response"
        return 69
    fi
    if ! jq -e '.result.value == "strict"' "$ssl_response" >/dev/null; then
        command rm -f "$ssl_response"
        die 78 'Cloudflare zone SSL mode must already be Full (strict)'
        return
    fi
    command rm -f "$ssl_response"
    _cf_refresh_dns
}

cf_issue_origin_certificate() {
    local csr_file=$1 request response admin_host school_host
    [[ -r $csr_file ]] || die 64 'Origin certificate CSR is not readable' || return
    request=$(_cf_temporary_file) || return
    response=$(_cf_temporary_file) || {
        command rm -f "$request"
        return
    }
    admin_host="admin-api.${SO_CONFIG[base_domain]}"
    school_host="school-api.${SO_CONFIG[base_domain]}"
    jq -n \
        --arg admin_host "$admin_host" \
        --arg school_host "$school_host" \
        --rawfile csr "$csr_file" \
        '{hostnames:[$admin_host,$school_host],request_type:"origin-rsa",requested_validity:5475,csr:$csr}' \
        >"$request"
    if ! _cf_request POST /certificates "$response" "$request"; then
        command rm -f "$request" "$response"
        return 69
    fi
    command rm -f "$request"

    if ! jq -e --arg admin "$admin_host" --arg school "$school_host" '
        (.result.hostnames | index($admin)) != null and
        (.result.hostnames | index($school)) != null and
        (.result.certificate | startswith("-----BEGIN CERTIFICATE-----"))
    ' "$response" >/dev/null; then
        command rm -f "$response"
        die 78 'Origin certificate response does not contain both requested hosts'
        return
    fi
    # shellcheck disable=SC2034 # Kept in memory for the VPS TLS installation phase.
    CF_CERTIFICATE=$(jq -er '.result.certificate' "$response") || {
        command rm -f "$response"
        return 69
    }
    # shellcheck disable=SC2034 # Metadata is consumed by orchestration checkpoints.
    SO_CF_CERTIFICATE_ID=$(jq -er '.result.id' "$response") || {
        command rm -f "$response"
        return 69
    }
    # shellcheck disable=SC2034
    SO_CF_CERTIFICATE_EXPIRES=$(jq -er '.result.expires_on' "$response") || {
        command rm -f "$response"
        return 69
    }
    command rm -f "$response"
}

_cf_dns_etag() {
    printf '%s' "$1" | sha256sum | awk '{print $1}'
}

cf_snapshot_dns() {
    [[ -n $SO_CF_DNS_RECORDS ]] || _cf_refresh_dns || return
    if ! SO_DNS_ORIGINAL_IP=$(jq -er 'map(.content) | unique | if length == 1 then .[0] else error("split origin") end' <<<"$SO_CF_DNS_RECORDS"); then
        die 78 'API DNS records must share one original IPv4 address'
        return
    fi
    SO_DNS_SNAPSHOT=$SO_CF_DNS_RECORDS
    SO_DNS_SNAPSHOT_ETAG=$(_cf_dns_etag "$SO_DNS_SNAPSHOT")
    SO_DNS_CURRENT_ETAG=
}

cf_restore_snapshot() {
    local zone_id=$1 account_id=$2 snapshot=$3 snapshot_etag=$4 original_ip=$5
    jq -e --arg base "${SO_CONFIG[base_domain]}" --arg original "$original_ip" '
        type == "array" and length == 2 and
        all(.[];
            (.id | type == "string" and length > 0) and
            .type == "A" and
            (.name == "admin-api.\($base)" or .name == "school-api.\($base)") and
            .content == $original and
            (.ttl | type == "number") and
            (.proxied | type == "boolean") and
            (.modified_on | type == "string")
        ) and
        ([.[].name] | unique | length == 2)
    ' <<<"$snapshot" >/dev/null || die 78 'Checkpoint DNS snapshot is invalid' || return
    [[ -n $zone_id && -n $account_id && -n $snapshot_etag ]] || die 78 'Checkpoint Cloudflare metadata is incomplete' || return
    SO_CF_ZONE_ID=$zone_id
    # shellcheck disable=SC2034 # Restored provider outputs are consumed by later phases.
    SO_CF_ACCOUNT_ID=$account_id
    SO_DNS_SNAPSHOT=$(jq -c 'sort_by(.name)' <<<"$snapshot")
    SO_DNS_SNAPSHOT_ETAG=$snapshot_etag
    # shellcheck disable=SC2034
    SO_DNS_ORIGINAL_IP=$original_ip
    # shellcheck disable=SC2034
    SO_CF_ADMIN_RECORD_ID=$(jq -er --arg name "admin-api.${SO_CONFIG[base_domain]}" '.[] | select(.name == $name) | .id' <<<"$SO_DNS_SNAPSHOT") || return 78
    # shellcheck disable=SC2034
    SO_CF_SCHOOL_RECORD_ID=$(jq -er --arg name "school-api.${SO_CONFIG[base_domain]}" '.[] | select(.name == $name) | .id' <<<"$SO_DNS_SNAPSHOT") || return 78
}

cf_assert_cutover_state() {
    [[ -n $SO_DNS_SNAPSHOT ]] || die 78 'Cloudflare DNS snapshot is missing' || return
    _cf_refresh_dns || return
    local expected current
    expected=$(jq -c --arg target "${SO_CONFIG[target]}" '[.[] | {id,type,name,content:$target,ttl,proxied:true}] | sort_by(.name)' <<<"$SO_DNS_SNAPSHOT")
    current=$(jq -c '[.[] | {id,type,name,content,ttl,proxied}] | sort_by(.name)' <<<"$SO_CF_DNS_RECORDS")
    [[ $current == "$expected" ]] || die 78 'Cloudflare API records do not match the verified cutover state'
}

cf_assert_no_dns_drift() {
    [[ -n $SO_DNS_SNAPSHOT_ETAG ]] || die 78 'Cloudflare DNS snapshot is missing' || return
    if [[ -z $SO_DNS_CURRENT_ETAG ]]; then
        _cf_refresh_dns || return
        SO_DNS_CURRENT_ETAG=$(_cf_dns_etag "$SO_CF_DNS_RECORDS")
    fi
    [[ $SO_DNS_CURRENT_ETAG == "$SO_DNS_SNAPSHOT_ETAG" ]] || die 78 'Cloudflare DNS records changed after the snapshot'
}

cf_apply_dns_batch() {
    local mode=$1 request response patches
    [[ -n $SO_DNS_SNAPSHOT ]] || die 78 'Cloudflare DNS snapshot is missing' || return
    request=$(_cf_temporary_file) || return
    response=$(_cf_temporary_file) || {
        command rm -f "$request"
        return
    }

    case "$mode" in
        cutover)
            SO_DNS_CURRENT_ETAG=
            cf_assert_no_dns_drift || {
                command rm -f "$request" "$response"
                return
            }
            patches=$(jq -c --arg target "${SO_CONFIG[target]}" \
                '[.[] | {id,type,name,content:$target,ttl,proxied:true}]' <<<"$SO_DNS_SNAPSHOT")
            ;;
        rollback)
            patches=$(jq -c '[.[] | {id,type,name,content,ttl,proxied}]' <<<"$SO_DNS_SNAPSHOT")
            ;;
        *)
            command rm -f "$request" "$response"
            die 64 'DNS batch mode must be cutover or rollback'
            return
            ;;
    esac
    jq -n --argjson patches "$patches" '{patches:$patches}' >"$request"
    if ! _cf_request POST "/zones/$SO_CF_ZONE_ID/dns_records/batch" "$response" "$request"; then
        command rm -f "$request" "$response"
        return 69
    fi
    SO_DNS_CURRENT_ETAG=
    command rm -f "$request" "$response"
}

cf_wait_for_record_content() {
    local target_ip=$1 attempt
    local attempts=${SO_PROVIDER_POLL_ATTEMPTS:-20}
    local delay=${SO_PROVIDER_POLL_DELAY:-3}
    for ((attempt = 1; attempt <= attempts; attempt++)); do
        _cf_refresh_dns || return
        if jq -e --arg target "$target_ip" 'length == 2 and all(.[]; .content == $target)' <<<"$SO_CF_DNS_RECORDS" >/dev/null; then
            return 0
        fi
        ((attempt < attempts)) && sleep "$delay"
    done
    return 75
}

_cf_proxy_hosts_resolve() {
    local target_ip=$1 host addresses
    for host in "admin-api.${SO_CONFIG[base_domain]}" "school-api.${SO_CONFIG[base_domain]}"; do
        addresses=$(getent ahostsv4 "$host" | awk '{print $1}' | sort -u) || return 1
        [[ -n $addresses ]] || return 1
        ! grep -Fxq "$target_ip" <<<"$addresses" || return 1
    done
}

cf_wait_for_proxy_resolution() {
    local target_ip=${1:-${SO_CONFIG[target]}}
    retry "${SO_PROVIDER_POLL_ATTEMPTS:-20}" "${SO_PROVIDER_POLL_DELAY:-3}" _cf_proxy_hosts_resolve "$target_ip"
}
