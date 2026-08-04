#!/usr/bin/env bash

SCHOOLORBIT_STATE_HOME=${SCHOOLORBIT_STATE_HOME:-"$HOME/.local/state/schoolorbit-installer"}
SO_RUN_ID=
SO_STATE_FILE=
SO_STATE_OPERATION=

_state_fingerprint_input() {
    local key
    printf 'repository=%s\n' "${SO_CONFIG[repository]-}"
    printf 'target=%s\n' "${SO_CONFIG[target]-}"
    printf 'base_domain=%s\n' "${SO_CONFIG[base_domain]-schoolorbit.app}"
    printf 'ref=%s\n' "${SO_CONFIG[ref]-main}"
    printf 'bootstrap_user=%s\n' "${SO_CONFIG[bootstrap_user]-root}"
    printf 'server_user=%s\n' "${SO_CONFIG[server_user]-schoolorbit}"
    printf 'ssh_port=%s\n' "${SO_CONFIG[ssh_port]-22}"
    for key in "${!SO_CONFIG[@]}"; do
        [[ $key == runtime:* ]] && printf '%s=%s\n' "$key" "${SO_CONFIG[$key]}"
    done | sort
}

state_fingerprint() {
    _state_fingerprint_input | sha256sum | awk '{print $1}'
}

_state_runtime_json() {
    local result='{}' key name
    for key in "${!SO_CONFIG[@]}"; do
        if [[ $key == runtime:* ]]; then
            name=${key#runtime:}
            result=$(jq -c --arg name "$name" --arg value "${SO_CONFIG[$key]}" '. + {($name): $value}' <<<"$result")
        fi
    done
    printf '%s\n' "$result"
}

state_init() {
    local run_id=$1 now runtime fingerprint run_directory temporary operation
    _valid_run_id "$run_id" || die 64 'Invalid state run ID' || return
    umask 077
    mkdir -p "$SCHOOLORBIT_STATE_HOME/runs"
    chmod 0700 "$SCHOOLORBIT_STATE_HOME" "$SCHOOLORBIT_STATE_HOME/runs"
    run_directory="$SCHOOLORBIT_STATE_HOME/runs/$run_id"
    [[ ! -e $run_directory/state.json ]] || die 78 'Installer run ID already has a checkpoint' || return
    mkdir -p "$run_directory"
    chmod 0700 "$run_directory"

    # shellcheck disable=SC2034 # Consumed by orchestration modules once loaded.
    SO_RUN_ID=$run_id
    SO_STATE_FILE="$run_directory/state.json"
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    runtime=$(_state_runtime_json)
    fingerprint=$(state_fingerprint)
    operation=${SO_COMMAND:-migrate-vps}
    [[ $operation == migrate-vps || $operation == configure-cockpit ]] || die 64 'Invalid installer checkpoint operation' || return
    temporary=$(mktemp "$run_directory/state.json.XXXXXX")
    jq -n \
        --arg run_id "$run_id" \
        --arg operation "$operation" \
        --arg created_at "$now" \
        --arg repository "${SO_CONFIG[repository]-}" \
        --arg target "${SO_CONFIG[target]-}" \
        --arg base_domain "${SO_CONFIG[base_domain]-schoolorbit.app}" \
        --arg ref "${SO_CONFIG[ref]-main}" \
        --arg bootstrap_user "${SO_CONFIG[bootstrap_user]-root}" \
        --arg server_user "${SO_CONFIG[server_user]-schoolorbit}" \
        --arg ssh_port "${SO_CONFIG[ssh_port]-22}" \
        --arg fingerprint "$fingerprint" \
        --argjson runtime "$runtime" \
        '{schema_version:1,run_id:$run_id,operation:$operation,created_at:$created_at,updated_at:$created_at,configuration:{repository:$repository,target:$target,base_domain:$base_domain,ref:$ref,bootstrap_user:$bootstrap_user,server_user:$server_user,ssh_port:$ssh_port,runtime:$runtime},configuration_fingerprint:$fingerprint,phases:{}}' \
        >"$temporary"
    chmod 0600 "$temporary"
    mv "$temporary" "$SO_STATE_FILE"
}

state_load() {
    local run_id=$1 key value
    _valid_run_id "$run_id" || die 64 'Invalid state run ID' || return
    # shellcheck disable=SC2034 # Consumed by orchestration modules once loaded.
    SO_RUN_ID=$run_id
    SO_STATE_FILE="$SCHOOLORBIT_STATE_HOME/runs/$run_id/state.json"
    [[ -f $SO_STATE_FILE ]] || die 78 'Installer checkpoint was not found' || return
    jq -e '.schema_version == 1 and (.configuration | type == "object") and (.phases | type == "object")' "$SO_STATE_FILE" >/dev/null || die 78 'Installer checkpoint is invalid' || return
    SO_STATE_OPERATION=$(jq -er '.operation // "migrate-vps" | select(. == "migrate-vps" or . == "configure-cockpit")' "$SO_STATE_FILE") || die 78 'Installer checkpoint operation is invalid' || return

    for key in repository target base_domain ref bootstrap_user server_user ssh_port; do
        value=$(jq -er --arg key "$key" '.configuration[$key] | strings' "$SO_STATE_FILE") || die 78 'Installer checkpoint configuration is incomplete' || return
        SO_CONFIG["$key"]=$value
    done
    while IFS=$'\t' read -r key value; do
        SO_CONFIG["runtime:$key"]=$value
    done < <(jq -r '.configuration.runtime // {} | to_entries[] | [.key, .value] | @tsv' "$SO_STATE_FILE")
}

state_assert_operation() {
    local allowed
    for allowed in "$@"; do
        [[ $SO_STATE_OPERATION == "$allowed" ]] && return 0
    done
    die 78 'Installer checkpoint belongs to a different operation'
}

_state_sanitize_details() {
    jq -c '
        if type != "object" then error("phase details must be an object") else . end
        | with_entries(select(.key | IN(
            "status", "code", "workflow_run_id", "workflow_run_url", "workflow_runs",
            "cloudflare_zone_id", "cloudflare_account_id", "cloudflare_record_ids",
            "cloudflare_certificate_id", "certificate_expiry", "dns_snapshot",
            "dns_snapshot_etag", "original_ip", "target_ip", "deployment_gates",
            "verification_codes", "completed_at", "management_hostname",
            "management_dns_snapshot", "management_record_id",
            "management_record_existed", "management_tunnel_id", "management_tunnel_name"
        )))
    ' <<<"$1"
}

state_mark_phase() {
    local phase=$1 details=$2 temporary sanitized now secret
    [[ -n $SO_STATE_FILE && -f $SO_STATE_FILE ]] || die 78 'Installer checkpoint is not initialized' || return
    [[ $phase =~ ^[a-z0-9][a-z0-9_-]{0,63}$ ]] || die 64 'Invalid phase name' || return
    sanitized=$(_state_sanitize_details "$details") || die 64 'Invalid phase checkpoint details' || return
    for secret in "${SO_SECRETS[@]}"; do
        if [[ -n $secret ]] && jq -e --arg secret "$secret" '[.. | strings | contains($secret)] | any' <<<"$sanitized" >/dev/null; then
            die 64 'Phase checkpoint details contain a secret value'
            return
        fi
    done
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    temporary=$(mktemp "${SO_STATE_FILE}.XXXXXX")
    jq --arg phase "$phase" --arg now "$now" --argjson details "$sanitized" \
        '.phases[$phase] = $details | .updated_at = $now' "$SO_STATE_FILE" >"$temporary"
    chmod 0600 "$temporary"
    mv "$temporary" "$SO_STATE_FILE"
}

state_phase_done() {
    jq -e --arg phase "$1" '.phases[$phase].status == "passed"' "$SO_STATE_FILE" >/dev/null
}

state_assert_fingerprint() {
    local expected actual
    expected=$(jq -er '.configuration_fingerprint' "$SO_STATE_FILE") || die 78 'Checkpoint fingerprint is missing' || return
    actual=$(state_fingerprint)
    [[ $actual == "$expected" ]] || die 78 'Installer configuration changed since the checkpoint was created'
}
