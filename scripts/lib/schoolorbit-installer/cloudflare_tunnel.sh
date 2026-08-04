#!/usr/bin/env bash

SO_CF_COCKPIT_HOSTNAME=
SO_CF_COCKPIT_RECORD_ID=
SO_CF_COCKPIT_RECORD_EXISTED=false
SO_CF_COCKPIT_CURRENT_RECORD=
SO_CF_COCKPIT_DNS_SNAPSHOT=
SO_CF_COCKPIT_SNAPSHOT_READY=false
SO_CF_COCKPIT_TUNNEL_ID=
SO_CF_COCKPIT_TUNNEL_NAME=
SO_CF_COCKPIT_TUNNELS=

_cf_cockpit_valid_uuid() {
    [[ $1 =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$ ]]
}

_cf_cockpit_valid_record_id() {
    [[ $1 =~ ^[A-Za-z0-9_-]{1,128}$ ]]
}

_cf_cockpit_refresh_dns() {
    local response records count
    [[ -n $SO_CF_ZONE_ID ]] || die 78 'Cloudflare zone is not loaded' || return
    [[ -n $SO_CF_COCKPIT_HOSTNAME ]] || die 78 'Cockpit hostname is not initialized' || return
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/zones/$SO_CF_ZONE_ID/dns_records?name=$SO_CF_COCKPIT_HOSTNAME&per_page=100" "$response"; then
        command rm -f "$response"
        return 69
    fi
    records=$(jq -cer --arg hostname "$SO_CF_COCKPIT_HOSTNAME" '
        if (.result | type) != "array" then error("invalid DNS result") else . end
        | [.result[] | select(.name == $hostname)]
        | map({
            id, type, name, content, ttl, proxied,
            comment:(.comment // ""), tags:(.tags // []), settings:(.settings // {}),
            modified_on:(.modified_on // "")
        })
    ' "$response" 2>/dev/null) || {
        command rm -f "$response"
        die 69 'Cloudflare returned an invalid Cockpit DNS response'
        return
    }
    command rm -f "$response"
    count=$(jq 'length' <<<"$records") || return 69
    ((count <= 1)) || die 78 'Cockpit DNS name is duplicated' || return
    if ((count == 1)); then
        jq -e '
            .[0].type == "CNAME" and .[0].proxied == true and
            (.[0].id | type == "string" and length > 0) and
            (.[0].content | type == "string" and length > 0) and
            (.[0].ttl | type == "number") and
            (.[0].comment | type == "string") and
            (.[0].tags | type == "array") and
            (.[0].settings | type == "object")
        ' <<<"$records" >/dev/null || die 78 'Cockpit DNS must be one proxied CNAME' || return
        SO_CF_COCKPIT_CURRENT_RECORD=$(jq -c '.[0]' <<<"$records")
        if [[ $SO_CF_COCKPIT_SNAPSHOT_READY != true ]]; then
            SO_CF_COCKPIT_RECORD_ID=$(jq -er '.[0].id' <<<"$records") || return 69
            SO_CF_COCKPIT_RECORD_EXISTED=true
        fi
    else
        SO_CF_COCKPIT_CURRENT_RECORD=null
        if [[ $SO_CF_COCKPIT_SNAPSHOT_READY != true ]]; then
            SO_CF_COCKPIT_RECORD_ID=
            SO_CF_COCKPIT_RECORD_EXISTED=false
        fi
    fi
}

_cf_cockpit_list_tunnels() {
    local response
    [[ -n $SO_CF_ACCOUNT_ID ]] || die 78 'Cloudflare account is not loaded' || return
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel?is_deleted=false&per_page=1000" "$response"; then
        command rm -f "$response"
        return 69
    fi
    SO_CF_COCKPIT_TUNNELS=$(jq -cer '
        if (.result | type) != "array" then error("invalid Tunnel result") else . end
        | [.result[] | {id,name,deleted_at:(.deleted_at // null),status:(.status // "unknown")}]
    ' "$response" 2>/dev/null) || {
        command rm -f "$response"
        die 69 'Cloudflare returned an invalid Tunnel response'
        return
    }
    command rm -f "$response"
}

cf_cockpit_preflight() {
    require_command curl || return
    require_command jq || return
    [[ -n $SO_CF_ZONE_ID && -n $SO_CF_ACCOUNT_ID ]] || die 78 'Cloudflare zone and account must be loaded first' || return
    SO_CF_COCKPIT_HOSTNAME="server.${SO_CONFIG[base_domain]}"
    _cf_cockpit_refresh_dns || return
    _cf_cockpit_list_tunnels
}

cf_cockpit_snapshot() {
    [[ -n $SO_CF_COCKPIT_CURRENT_RECORD ]] || _cf_cockpit_refresh_dns || return
    SO_CF_COCKPIT_DNS_SNAPSHOT=$SO_CF_COCKPIT_CURRENT_RECORD
    SO_CF_COCKPIT_SNAPSHOT_READY=true
}

_cf_cockpit_create_tunnel() {
    local request response
    request=$(_cf_temporary_file) || return
    response=$(_cf_temporary_file) || {
        command rm -f "$request"
        return
    }
    jq -n --arg name "$SO_CF_COCKPIT_TUNNEL_NAME" '{name:$name,config_src:"cloudflare"}' >"$request"
    if ! _cf_request POST "/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel" "$response" "$request"; then
        command rm -f "$request" "$response"
        return 69
    fi
    command rm -f "$request"
    SO_CF_COCKPIT_TUNNEL_ID=$(jq -er --arg name "$SO_CF_COCKPIT_TUNNEL_NAME" '
        .result | select(.name == $name and .config_src == "cloudflare") | .id
    ' "$response" 2>/dev/null) || {
        command rm -f "$response"
        die 69 'Cloudflare returned an invalid created Tunnel'
        return
    }
    command rm -f "$response"
    _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 69 'Cloudflare returned an invalid Tunnel ID'
}

_cf_cockpit_adopt_or_create_tunnel() {
    local matches
    [[ -n $SO_CF_COCKPIT_TUNNELS ]] || _cf_cockpit_list_tunnels || return
    matches=$(jq -c --arg name "$SO_CF_COCKPIT_TUNNEL_NAME" '[.[] | select(.name == $name and .deleted_at == null)]' <<<"$SO_CF_COCKPIT_TUNNELS") || return 69
    case "$(jq 'length' <<<"$matches")" in
        0)
            _cf_cockpit_create_tunnel
            ;;
        1)
            SO_CF_COCKPIT_TUNNEL_ID=$(jq -er '.[0].id' <<<"$matches") || return 69
            _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Existing Cockpit Tunnel has an invalid ID'
            ;;
        *)
            die 78 'Cockpit Tunnel name is duplicated'
            ;;
    esac
}

cf_cockpit_provision_tunnel() {
    local request response
    [[ -n $SO_CF_COCKPIT_HOSTNAME ]] || die 78 'Cockpit preflight has not run' || return
    SO_CF_COCKPIT_TUNNEL_NAME="schoolorbit-cockpit-${SO_RUN_ID}"
    if [[ -z $SO_CF_COCKPIT_TUNNEL_ID ]]; then
        _cf_cockpit_adopt_or_create_tunnel || return
    else
        _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Checkpoint Cockpit Tunnel ID is invalid' || return
    fi

    request=$(_cf_temporary_file) || return
    response=$(_cf_temporary_file) || {
        command rm -f "$request"
        return
    }
    jq -n --arg hostname "$SO_CF_COCKPIT_HOSTNAME" '{config:{ingress:[
        {hostname:$hostname,service:"http://127.0.0.1:9090",originRequest:{}},
        {service:"http_status:404"}
    ]}}' >"$request"
    if ! _cf_request PUT "/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel/$SO_CF_COCKPIT_TUNNEL_ID/configurations" "$response" "$request"; then
        command rm -f "$request" "$response"
        return 69
    fi
    command rm -f "$request" "$response"
}

cf_cockpit_get_token() {
    local response token
    _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Cockpit Tunnel is not provisioned' || return
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel/$SO_CF_COCKPIT_TUNNEL_ID/token" "$response"; then
        command rm -f "$response"
        return 69
    fi
    token=$(jq -er '.result | strings | select(length >= 32 and (contains("\n") | not))' "$response" 2>/dev/null) || {
        command rm -f "$response"
        die 69 'Cloudflare returned an invalid Tunnel token'
        return
    }
    command rm -f "$response"
    # shellcheck disable=SC2034 # Consumed by the VPS configuration boundary and redaction.
    SO_SECRETS[SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN]=$token
}

_cf_cockpit_connector_ready() {
    local response ready=false
    response=$(_cf_temporary_file) || return
    if ! _cf_request GET "/accounts/$SO_CF_ACCOUNT_ID/cfd_tunnel/$SO_CF_COCKPIT_TUNNEL_ID/connections" "$response"; then
        command rm -f "$response"
        return 69
    fi
    if jq -e --arg target "${SO_CONFIG[target]}" '
        (.result | type) == "array" and
        any(.result[]; .origin_ip == $target and (.is_pending_reconnect // false) == false)
    ' "$response" >/dev/null 2>&1; then
        ready=true
    fi
    command rm -f "$response"
    [[ $ready == true ]]
}

cf_cockpit_wait_connector() {
    _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Cockpit Tunnel is not provisioned' || return
    retry "${SO_PROVIDER_POLL_ATTEMPTS:-20}" "${SO_PROVIDER_POLL_DELAY:-3}" _cf_cockpit_connector_ready
}

cf_cockpit_assert_no_dns_drift() {
    local current
    [[ $SO_CF_COCKPIT_SNAPSHOT_READY == true ]] || die 78 'Cockpit DNS snapshot is missing' || return
    _cf_cockpit_refresh_dns || return
    current=$SO_CF_COCKPIT_CURRENT_RECORD
    [[ $current == "$SO_CF_COCKPIT_DNS_SNAPSHOT" ]] || die 78 'Cockpit DNS changed after the snapshot'
}

_cf_cockpit_publish_body() {
    local target="${SO_CF_COCKPIT_TUNNEL_ID}.cfargotunnel.com"
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == true ]]; then
        jq -c --arg target "$target" '
            {type,name,content:$target,ttl,proxied:true,comment,tags,settings}
        ' <<<"$SO_CF_COCKPIT_DNS_SNAPSHOT"
    else
        jq -cn --arg hostname "$SO_CF_COCKPIT_HOSTNAME" --arg target "$target" '{
            type:"CNAME",name:$hostname,content:$target,ttl:1,proxied:true,
            comment:"SchoolOrbit Cockpit Cloudflare Tunnel",tags:["schoolorbit:management"],settings:{}
        }'
    fi
}

_cf_cockpit_adopt_unjournaled_published_record() {
    local current expected record_id
    [[ $SO_CF_COCKPIT_RECORD_EXISTED == false && -z $SO_CF_COCKPIT_RECORD_ID ]] || return 1
    [[ $SO_CF_COCKPIT_CURRENT_RECORD != null ]] || return 1
    current=$(jq -c '{type,name,content,ttl,proxied,comment,tags,settings}' \
        <<<"$SO_CF_COCKPIT_CURRENT_RECORD") || return 69
    expected=$(_cf_cockpit_publish_body) || return 69
    [[ $current == "$expected" ]] || return 1
    record_id=$(jq -er '.id | strings | select(length > 0)' \
        <<<"$SO_CF_COCKPIT_CURRENT_RECORD") || return 69
    _cf_cockpit_valid_record_id "$record_id" || die 69 'Cloudflare returned an invalid Cockpit DNS record ID' || return
    SO_CF_COCKPIT_RECORD_ID=$record_id
}

cf_cockpit_publish() {
    local request response method path adoption_status
    _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Cockpit Tunnel is not provisioned' || return
    if _cf_cockpit_valid_record_id "$SO_CF_COCKPIT_RECORD_ID"; then
        _cf_cockpit_refresh_dns || return
        if _cf_cockpit_assert_published_record; then
            return 0
        fi
    elif [[ $SO_CF_COCKPIT_RECORD_EXISTED == false ]]; then
        _cf_cockpit_refresh_dns || return
        if _cf_cockpit_adopt_unjournaled_published_record; then
            return 0
        else
            adoption_status=$?
        fi
        ((adoption_status == 1)) || return "$adoption_status"
    fi
    cf_cockpit_assert_no_dns_drift || return
    request=$(_cf_temporary_file) || return
    response=$(_cf_temporary_file) || {
        command rm -f "$request"
        return
    }
    _cf_cockpit_publish_body >"$request" || {
        command rm -f "$request" "$response"
        return 69
    }
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == true ]]; then
        method=PATCH
        path="/zones/$SO_CF_ZONE_ID/dns_records/$SO_CF_COCKPIT_RECORD_ID"
    else
        method=POST
        path="/zones/$SO_CF_ZONE_ID/dns_records"
    fi
    if ! _cf_request "$method" "$path" "$response" "$request"; then
        command rm -f "$request" "$response"
        return 69
    fi
    command rm -f "$request"
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == false ]]; then
        SO_CF_COCKPIT_RECORD_ID=$(jq -er '.result.id | strings | select(length > 0)' "$response" 2>/dev/null) || {
            command rm -f "$response"
            die 69 'Cloudflare did not return the created Cockpit DNS record ID'
            return
        }
        _cf_cockpit_valid_record_id "$SO_CF_COCKPIT_RECORD_ID" || {
            command rm -f "$response"
            die 69 'Cloudflare returned an invalid Cockpit DNS record ID'
            return
        }
    fi
    command rm -f "$response"
}

_cf_cockpit_assert_published_record() {
    local target="${SO_CF_COCKPIT_TUNNEL_ID}.cfargotunnel.com"
    local current expected
    [[ $SO_CF_COCKPIT_CURRENT_RECORD != null ]] || return 1
    current=$(jq -c '{id,type,name,content,ttl,proxied,comment,tags,settings}' \
        <<<"$SO_CF_COCKPIT_CURRENT_RECORD") || return
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == true ]]; then
        expected=$(jq -c --arg id "$SO_CF_COCKPIT_RECORD_ID" --arg target "$target" '
            {id:$id,type,name,content:$target,ttl,proxied:true,comment,tags,settings}
        ' <<<"$SO_CF_COCKPIT_DNS_SNAPSHOT") || return
    else
        expected=$(jq -cn \
            --arg id "$SO_CF_COCKPIT_RECORD_ID" \
            --arg hostname "$SO_CF_COCKPIT_HOSTNAME" \
            --arg target "$target" '{
                id:$id,type:"CNAME",name:$hostname,content:$target,ttl:1,proxied:true,
                comment:"SchoolOrbit Cockpit Cloudflare Tunnel",
                tags:["schoolorbit:management"],settings:{}
            }') || return
    fi
    [[ $current == "$expected" ]]
}

cf_cockpit_assert_published_state() {
    [[ $SO_CF_COCKPIT_SNAPSHOT_READY == true ]] || die 78 'Cockpit DNS snapshot is missing' || return
    _cf_cockpit_valid_uuid "$SO_CF_COCKPIT_TUNNEL_ID" || die 78 'Cockpit Tunnel is not provisioned' || return
    _cf_cockpit_valid_record_id "$SO_CF_COCKPIT_RECORD_ID" || die 78 'Cockpit DNS record ID is invalid' || return
    _cf_cockpit_refresh_dns || return
    _cf_cockpit_assert_published_record || die 78 'Cockpit DNS no longer matches this run'
}

cf_cockpit_restore_dns() {
    local response request=
    [[ $SO_CF_COCKPIT_SNAPSHOT_READY == true ]] || die 78 'Cockpit DNS snapshot is missing' || return
    _cf_cockpit_valid_record_id "$SO_CF_COCKPIT_RECORD_ID" || die 78 'Cockpit DNS record ID is invalid' || return
    _cf_cockpit_refresh_dns || return
    _cf_cockpit_assert_published_record || die 78 'Cockpit DNS no longer matches this run' || return
    response=$(_cf_temporary_file) || return
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == true ]]; then
        request=$(_cf_temporary_file) || {
            command rm -f "$response"
            return
        }
        jq -c '{type,name,content,ttl,proxied,comment,tags,settings}' \
            <<<"$SO_CF_COCKPIT_DNS_SNAPSHOT" >"$request"
        if ! _cf_request PATCH "/zones/$SO_CF_ZONE_ID/dns_records/$SO_CF_COCKPIT_RECORD_ID" "$response" "$request"; then
            command rm -f "$request" "$response"
            return 69
        fi
        command rm -f "$request"
    elif ! _cf_request DELETE "/zones/$SO_CF_ZONE_ID/dns_records/$SO_CF_COCKPIT_RECORD_ID" "$response"; then
        command rm -f "$response"
        return 69
    fi
    command rm -f "$response"
}

cf_cockpit_restore_checkpoint() {
    local hostname=$1 snapshot=$2 record_id=$3 record_existed=$4 tunnel_id=$5 tunnel_name=$6
    local expected_hostname="server.${SO_CONFIG[base_domain]}"
    [[ $hostname == "$expected_hostname" ]] || die 78 'Checkpoint Cockpit hostname is invalid' || return
    [[ $record_existed == true || $record_existed == false ]] || die 78 'Checkpoint Cockpit record ownership is invalid' || return
    if [[ -n $tunnel_id || -n $tunnel_name ]]; then
        _cf_cockpit_valid_uuid "$tunnel_id" || die 78 'Checkpoint Cockpit Tunnel ID is invalid' || return
        [[ $tunnel_name == "schoolorbit-cockpit-${SO_RUN_ID}" ]] || die 78 'Checkpoint Cockpit Tunnel name is invalid' || return
    fi

    if [[ $record_existed == true ]]; then
        _cf_cockpit_valid_record_id "$record_id" || die 78 'Checkpoint Cockpit record ID is invalid' || return
        jq -e --arg hostname "$expected_hostname" --arg id "$record_id" '
            type == "object" and .id == $id and .type == "CNAME" and .name == $hostname and
            (.content | type == "string" and length > 0) and
            (.ttl | type == "number") and (.proxied | type == "boolean") and
            (.comment | type == "string") and (.tags | type == "array") and
            (.settings | type == "object") and (.modified_on | type == "string")
        ' <<<"$snapshot" >/dev/null || die 78 'Checkpoint Cockpit DNS snapshot is invalid' || return
    else
        [[ $snapshot == null ]] || die 78 'Checkpoint Cockpit DNS snapshot is invalid' || return
        [[ -z $record_id ]] || _cf_cockpit_valid_record_id "$record_id" || die 78 'Checkpoint Cockpit record ID is invalid' || return
    fi

    SO_CF_COCKPIT_HOSTNAME=$hostname
    SO_CF_COCKPIT_DNS_SNAPSHOT=$(jq -c . <<<"$snapshot") || return 78
    SO_CF_COCKPIT_SNAPSHOT_READY=true
    SO_CF_COCKPIT_RECORD_ID=$record_id
    SO_CF_COCKPIT_RECORD_EXISTED=$record_existed
    SO_CF_COCKPIT_TUNNEL_ID=$tunnel_id
    SO_CF_COCKPIT_TUNNEL_NAME=$tunnel_name
}
