#!/usr/bin/env bash

declare -ga SO_PHASES=(
    preflight
    input
    snapshot
    bootstrap
    tls
    deploy
    origin-verify
    cutover-gate
    dns-cutover
    public-verify
    management-provision
    management-publish
    handoff
)
declare -ga SO_COCKPIT_PHASES=(
    preflight
    input
    management-snapshot
    bootstrap
    management-provision
    management-publish
    management-handoff
)
SO_CURRENT_PHASE=
SO_PHASE_DETAILS='{}'
SO_WORKFLOW_RUNS='[]'

generate_run_id() {
    printf '%s-%05d-%05d\n' "$(date -u +'%Y%m%dT%H%M%SZ')" "$RANDOM" "$RANDOM"
}

_phase_record_apply() {
    [[ -z ${PHASE_LOG-} ]] || printf '%s\n' "$1" >>"$PHASE_LOG"
}

_phase_preflight_apply() {
    local command_name
    ((BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4))) || die 69 'Installer requires Bash 4.4 or newer' || return
    if [[ $SO_COMMAND == configure-cockpit ]]; then
        for command_name in jq curl ssh sha256sum; do
            require_command "$command_name" || return
        done
        vps_preflight
        return
    fi
    for command_name in jq curl gh ssh ssh-keygen openssl sha256sum base64 python3; do
        require_command "$command_name" || return
    done
    github_preflight || return
    vps_preflight
}

_phase_input_apply() {
    [[ -n ${SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]-} ]] || die 64 'Installer inputs are not loaded' || return
    if [[ -n $SO_STATE_FILE ]]; then
        state_assert_fingerprint || return
    fi
}

_phase_snapshot_apply() {
    cf_preflight || return
    cf_snapshot_dns || return
    cf_cockpit_preflight || return
    cf_cockpit_snapshot || return
    SO_PHASE_DETAILS=$(jq -cn \
        --arg zone "$SO_CF_ZONE_ID" \
        --arg account "$SO_CF_ACCOUNT_ID" \
        --arg original "$SO_DNS_ORIGINAL_IP" \
        --arg etag "$SO_DNS_SNAPSHOT_ETAG" \
        --argjson dns "$SO_DNS_SNAPSHOT" \
        --arg management_hostname "$SO_CF_COCKPIT_HOSTNAME" \
        --arg management_record_id "$SO_CF_COCKPIT_RECORD_ID" \
        --arg management_record_existed "$SO_CF_COCKPIT_RECORD_EXISTED" \
        --argjson management_dns_snapshot "$SO_CF_COCKPIT_DNS_SNAPSHOT" \
        '{cloudflare_zone_id:$zone,cloudflare_account_id:$account,original_ip:$original,dns_snapshot_etag:$etag,dns_snapshot:$dns,
          management_hostname:$management_hostname,management_dns_snapshot:$management_dns_snapshot,
          management_record_id:$management_record_id,management_record_existed:($management_record_existed == "true") }')
}

_phase_management_snapshot_apply() {
    cf_preflight || return
    cf_cockpit_preflight || return
    cf_cockpit_snapshot || return
    SO_PHASE_DETAILS=$(jq -cn \
        --arg zone "$SO_CF_ZONE_ID" \
        --arg account "$SO_CF_ACCOUNT_ID" \
        --arg management_hostname "$SO_CF_COCKPIT_HOSTNAME" \
        --arg management_record_id "$SO_CF_COCKPIT_RECORD_ID" \
        --arg management_record_existed "$SO_CF_COCKPIT_RECORD_EXISTED" \
        --argjson management_dns_snapshot "$SO_CF_COCKPIT_DNS_SNAPSHOT" \
        '{cloudflare_zone_id:$zone,cloudflare_account_id:$account,
          management_hostname:$management_hostname,management_dns_snapshot:$management_dns_snapshot,
          management_record_id:$management_record_id,management_record_existed:($management_record_existed == "true") }')
}

_phase_bootstrap_apply() {
    vps_bootstrap || return
    [[ $SO_COMMAND == configure-cockpit ]] && return 0
    vps_install_runtime_env
}

_phase_tls_apply() {
    vps_issue_and_install_tls || return
    SO_PHASE_DETAILS=$(jq -cn \
        --arg certificate_id "$SO_CF_CERTIFICATE_ID" \
        --arg expiry "$SO_CF_CERTIFICATE_EXPIRES" \
        '{cloudflare_certificate_id:$certificate_id,certificate_expiry:$expiry}')
}

_phase_deploy_apply() {
    local workflow
    local -a workflows=(
        deploy-backend-admin.yml
        deploy-backend-school.yml
        deploy-frontend-admin.yml
        deploy-all-schools.yml
    )
    SO_WORKFLOW_RUNS='[]'
    vps_create_deployment_key || return
    github_configure_repository || return
    for workflow in "${workflows[@]}"; do
        github_dispatch_and_wait "$workflow" "$SO_RUN_ID" || return
        SO_WORKFLOW_RUNS=$(jq -c \
            --arg workflow "$workflow" \
            --arg id "$SO_GITHUB_RUN_ID" \
            --arg url "$SO_GITHUB_RUN_URL" \
            '. + [{workflow:$workflow,id:($id | tonumber),url:$url}]' <<<"$SO_WORKFLOW_RUNS")
    done
    vps_cleanup_deployment_key || return
    SO_PHASE_DETAILS=$(jq -cn --argjson runs "$SO_WORKFLOW_RUNS" '{workflow_runs:$runs}')
}

_phase_origin_verify_apply() {
    vps_prepare_verified_origin_root || return
    verify_direct_origin || return
    vps_cleanup_tls_material
}

_print_cutover_diff() {
    local mode=$1
    case "$mode" in
        cutover)
            jq -r --arg target "${SO_CONFIG[target]}" '.[] | "\(.name): \(.content) -> \($target) (proxied=true)"' <<<"$SO_DNS_SNAPSHOT"
            ;;
        rollback)
            jq -r --arg target "${SO_CONFIG[target]}" '.[] | "\(.name): \($target) -> \(.content) (proxied=\(.proxied))"' <<<"$SO_DNS_SNAPSHOT"
            ;;
    esac
}

_phase_cutover_gate_apply() {
    _print_cutover_diff cutover
    confirm_exact "CUTOVER ${SO_CONFIG[target]}" "Type CUTOVER ${SO_CONFIG[target]} to continue: " || die 64 'DNS cutover was not confirmed'
}

_phase_dns_cutover_apply() {
    cf_apply_dns_batch cutover || return
    cf_wait_for_record_content "${SO_CONFIG[target]}" || return
    cf_wait_for_proxy_resolution "${SO_CONFIG[target]}" || return
    cf_assert_cutover_state || return
    SO_PHASE_DETAILS=$(jq -cn --arg target "${SO_CONFIG[target]}" '{target_ip:$target,verification_codes:["dns_api_target","proxy_resolved"]}')
}

_phase_public_verify_apply() {
    verify_public_services || return
    SO_PHASE_DETAILS='{"verification_codes":["api_identity","frontend_html","authenticated_smoke"]}'
}

_management_phase_details() {
    jq -cn \
        --arg management_hostname "$SO_CF_COCKPIT_HOSTNAME" \
        --arg management_record_id "$SO_CF_COCKPIT_RECORD_ID" \
        --arg management_record_existed "$SO_CF_COCKPIT_RECORD_EXISTED" \
        --arg management_tunnel_id "$SO_CF_COCKPIT_TUNNEL_ID" \
        --arg management_tunnel_name "$SO_CF_COCKPIT_TUNNEL_NAME" \
        --argjson management_dns_snapshot "$SO_CF_COCKPIT_DNS_SNAPSHOT" \
        '{management_hostname:$management_hostname,management_dns_snapshot:$management_dns_snapshot,
          management_record_id:$management_record_id,management_record_existed:($management_record_existed == "true"),
          management_tunnel_id:$management_tunnel_id,management_tunnel_name:$management_tunnel_name}'
}

_phase_management_provision_apply() {
    cf_cockpit_provision_tunnel || return
    cf_cockpit_get_token || return
    vps_configure_cockpit || return
    vps_reverify_cockpit || return
    cf_cockpit_wait_connector || return
    SO_PHASE_DETAILS=$(_management_phase_details) || return
    SO_PHASE_DETAILS=$(jq -c '. + {verification_codes:["cockpit_loopback","connector_target"]}' <<<"$SO_PHASE_DETAILS")
}

_phase_management_publish_apply() {
    local journal now
    cf_cockpit_publish || return
    journal=$(_management_phase_details) || return
    if [[ -n $SO_STATE_FILE ]]; then
        now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
        journal=$(jq -c --arg now "$now" '. + {status:"published",completed_at:$now}' <<<"$journal") || return
        state_mark_phase management-publish "$journal" || return
    fi
    verify_public_cockpit || return
    SO_PHASE_DETAILS=$(_management_phase_details) || return
    SO_PHASE_DETAILS=$(jq -c '. + {verification_codes:["cockpit_https","cockpit_login","cockpit_not_public_9090"]}' <<<"$SO_PHASE_DETAILS")
}

_print_management_handoff() {
    info "Cockpit: https://$SO_CF_COCKPIT_HOSTNAME (login: ${SO_CONFIG[server_user]})"
    info "Cockpit Tunnel: $SO_CF_COCKPIT_TUNNEL_ID ($SO_CF_COCKPIT_TUNNEL_NAME)"
    warn "Management rollback: scripts/schoolorbit-installer rollback-cockpit --run-id $SO_RUN_ID"
    warn 'Retain the previous management Tunnel and VPS until the rollback window is closed manually.'
}

_phase_management_handoff_apply() {
    SO_PHASE_DETAILS=$(_management_phase_details) || return
    _print_management_handoff
}

_print_handoff() {
    info "Migration run: $SO_RUN_ID"
    info "Checkpoint: $SO_STATE_FILE"
    jq -r '.[] | "Workflow \(.workflow): \(.url)"' <<<"$SO_WORKFLOW_RUNS" | while IFS= read -r line; do info "$line"; done
    info "Origin certificate: $SO_CF_CERTIFICATE_ID (expires $SO_CF_CERTIFICATE_EXPIRES)"
    info "Origin IP: $SO_DNS_ORIGINAL_IP -> ${SO_CONFIG[target]}"
    info 'Deployment gates: runtime=true, frontend=true'
    _print_management_handoff
    warn 'Retain the old VPS until the rollback window has been closed manually.'
}

_phase_handoff_apply() {
    github_set_variable RUNTIME_DEPLOY_ENABLED true || return
    github_set_variable FRONTEND_DEPLOY_ENABLED true || return
    github_variable_equals RUNTIME_DEPLOY_ENABLED true || return
    github_variable_equals FRONTEND_DEPLOY_ENABLED true || return
    SO_PHASE_DETAILS='{"deployment_gates":{"runtime":true,"frontend":true}}'
    _print_handoff
}

_phase_apply() {
    local phase=$1 function_name
    function_name="_phase_${phase//-/_}_apply"
    declare -F "$function_name" >/dev/null || die 78 "Installer phase is not implemented: $phase" || return
    SO_PHASE_DETAILS='{}'
    "$function_name"
}

_restore_checkpoint_outputs() {
    local snapshot_details tls_details deploy_details
    if state_phase_done snapshot; then
        snapshot_details=$(jq -c '.phases.snapshot' "$SO_STATE_FILE")
        cf_restore_snapshot \
            "$(jq -r '.cloudflare_zone_id' <<<"$snapshot_details")" \
            "$(jq -r '.cloudflare_account_id' <<<"$snapshot_details")" \
            "$(jq -c '.dns_snapshot' <<<"$snapshot_details")" \
            "$(jq -r '.dns_snapshot_etag' <<<"$snapshot_details")" \
            "$(jq -r '.original_ip' <<<"$snapshot_details")" || return
    fi
    if state_phase_done tls; then
        tls_details=$(jq -c '.phases.tls' "$SO_STATE_FILE")
        SO_CF_CERTIFICATE_ID=$(jq -r '.cloudflare_certificate_id' <<<"$tls_details")
        SO_CF_CERTIFICATE_EXPIRES=$(jq -r '.certificate_expiry' <<<"$tls_details")
    fi
    if state_phase_done deploy; then
        deploy_details=$(jq -c '.phases.deploy' "$SO_STATE_FILE")
        SO_WORKFLOW_RUNS=$(jq -c '.workflow_runs' <<<"$deploy_details")
    fi
    _restore_management_checkpoint_outputs
}

_management_checkpoint_details() {
    local phase
    for phase in management-publish management-provision management-snapshot snapshot; do
        if jq -e --arg phase "$phase" '
            (.phases[$phase] | type) == "object" and
            (.phases[$phase].management_hostname | type) == "string"
        ' "$SO_STATE_FILE" >/dev/null; then
            jq -c --arg phase "$phase" '.phases[$phase]' "$SO_STATE_FILE"
            return 0
        fi
    done
    return 1
}

_restore_management_checkpoint_outputs() {
    local details provider_details
    details=$(_management_checkpoint_details) || return 0
    if state_phase_done management-snapshot; then
        provider_details=$(jq -c '.phases["management-snapshot"]' "$SO_STATE_FILE")
    elif state_phase_done snapshot; then
        provider_details=$(jq -c '.phases.snapshot' "$SO_STATE_FILE")
    else
        provider_details=$details
    fi
    if [[ -z $SO_CF_ZONE_ID ]]; then
        SO_CF_ZONE_ID=$(jq -r '.cloudflare_zone_id // ""' <<<"$provider_details")
    fi
    if [[ -z $SO_CF_ACCOUNT_ID ]]; then
        SO_CF_ACCOUNT_ID=$(jq -r '.cloudflare_account_id // ""' <<<"$provider_details")
    fi
    cf_cockpit_restore_checkpoint \
        "$(jq -r '.management_hostname' <<<"$details")" \
        "$(jq -c '.management_dns_snapshot' <<<"$details")" \
        "$(jq -r '.management_record_id // ""' <<<"$details")" \
        "$(jq -r '.management_record_existed' <<<"$details")" \
        "$(jq -r '.management_tunnel_id // ""' <<<"$details")" \
        "$(jq -r '.management_tunnel_name // ""' <<<"$details")"
}

_management_was_published() {
    jq -e '
        .phases["management-publish"].status == "passed" or
        .phases["management-publish"].status == "published"
    ' "$SO_STATE_FILE" >/dev/null
}

_phase_snapshot_reverify() {
    local expected_zone=$SO_CF_ZONE_ID expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since the snapshot' || return
    if state_phase_done dns-cutover; then
        cf_assert_cutover_state || return
    else
        # shellcheck disable=SC2034 # Forces the provider drift check to re-read DNS.
        SO_DNS_CURRENT_ETAG=
        cf_assert_no_dns_drift || return
    fi
    cf_cockpit_preflight || return
    if _management_was_published; then
        cf_cockpit_assert_published_state
    else
        cf_cockpit_assert_no_dns_drift
    fi
}

_phase_management_snapshot_reverify() {
    local expected_zone=$SO_CF_ZONE_ID expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since the management snapshot' || return
    cf_cockpit_preflight || return
    if _management_was_published; then
        cf_cockpit_assert_published_state
    else
        cf_cockpit_assert_no_dns_drift
    fi
}

_phase_reverify() {
    local phase=$1
    case "$phase" in
        preflight) _phase_preflight_apply ;;
        input) state_assert_fingerprint ;;
        snapshot) _phase_snapshot_reverify ;;
        bootstrap)
            if [[ $SO_COMMAND == configure-cockpit ]]; then
                vps_reverify_cockpit_bootstrap
            else
                vps_reverify_bootstrap
            fi
            ;;
        tls) vps_reverify_tls ;;
        deploy) github_runs_succeeded "$SO_WORKFLOW_RUNS" ;;
        origin-verify)
            vps_prepare_verified_origin_root && verify_direct_origin && vps_cleanup_tls_material
            ;;
        cutover-gate)
            state_phase_done dns-cutover && return 0
            return 2
            ;;
        dns-cutover) cf_preflight && cf_assert_cutover_state ;;
        public-verify) verify_public_services ;;
        management-snapshot) _phase_management_snapshot_reverify ;;
        management-provision) vps_reverify_cockpit && cf_cockpit_wait_connector ;;
        management-publish) cf_cockpit_assert_published_state && verify_public_cockpit ;;
        management-handoff) return 0 ;;
        handoff)
            github_variable_equals RUNTIME_DEPLOY_ENABLED true &&
                github_variable_equals FRONTEND_DEPLOY_ENABLED true
            ;;
        *) return 78 ;;
    esac
}

_run_phase() {
    local phase=$1 reverify_status now details
    SO_CURRENT_PHASE=$phase
    if state_phase_done "$phase"; then
        if _phase_reverify "$phase"; then
            info "Verified checkpoint phase: $phase"
            return 0
        else
            reverify_status=$?
        fi
        if ((reverify_status != 2)); then
            die 78 "Checkpoint phase failed revalidation: $phase"
            return
        fi
    fi

    _phase_record_apply "$phase"
    _phase_apply "$phase" || return
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    details=$(jq -c --arg now "$now" '. + {status:"passed",completed_at:$now}' <<<"$SO_PHASE_DETAILS") || return 78
    state_mark_phase "$phase" "$details"
}

run_migration_phases() {
    local phase
    for phase in "${SO_PHASES[@]}"; do
        _run_phase "$phase" || return
    done
}

run_cockpit_phases() {
    local phase
    for phase in "${SO_COCKPIT_PHASES[@]}"; do
        _run_phase "$phase" || return
    done
}

run_migration_dry_run() {
    local phase
    for phase in preflight input snapshot; do
        SO_CURRENT_PHASE=$phase
        _phase_record_apply "$phase"
        _phase_apply "$phase" || return
    done
    info 'Dry-run plan (no mutations applied)'
    _print_cutover_diff cutover
    info 'Planned mutations: bootstrap VPS, install TLS/runtime, configure GitHub, dispatch four workflows, confirm DNS cutover, verify public services, configure Cockpit Tunnel, enable rollout gates.'
}

run_cockpit_dry_run() {
    local phase
    for phase in preflight input management-snapshot; do
        SO_CURRENT_PHASE=$phase
        _phase_record_apply "$phase"
        _phase_apply "$phase" || return
    done
    info 'Dry-run plan (no mutations applied)'
    info "Planned management hostname: $SO_CF_COCKPIT_HOSTNAME"
    info 'Planned mutations: bootstrap Cockpit/cloudflared, configure loopback Cockpit, start a target-bound Tunnel connector, then publish the management CNAME.'
}

_report_migration_failure() {
    local status=$1
    warn "Migration stopped in phase $SO_CURRENT_PHASE (exit $status)."
    if [[ -n $SO_STATE_FILE ]] && state_phase_done dns-cutover; then
        warn "DNS was cut over. Review diagnostics, then run: scripts/schoolorbit-installer rollback-dns --run-id $SO_RUN_ID"
    else
        warn "Resume after correcting the issue: scripts/schoolorbit-installer migrate-vps --resume $SO_RUN_ID"
    fi
}

_report_cockpit_failure() {
    local status=$1
    warn "Cockpit setup stopped in phase $SO_CURRENT_PHASE (exit $status)."
    if [[ -n $SO_STATE_FILE ]] && _management_was_published; then
        warn "Management DNS was published. Review diagnostics, then run: scripts/schoolorbit-installer rollback-cockpit --run-id $SO_RUN_ID"
    else
        warn "Resume after correcting the issue: scripts/schoolorbit-installer configure-cockpit --resume $SO_RUN_ID"
    fi
}

_print_management_rollback_diff() {
    local target="${SO_CF_COCKPIT_TUNNEL_ID}.cfargotunnel.com"
    if [[ $SO_CF_COCKPIT_RECORD_EXISTED == true ]]; then
        jq -r --arg target "$target" '"\(.name): \($target) -> \(.content) (proxied=\(.proxied))"' <<<"$SO_CF_COCKPIT_DNS_SNAPSHOT"
    else
        printf '%s: %s -> delete run-owned CNAME\n' "$SO_CF_COCKPIT_HOSTNAME" "$target"
    fi
}

_mark_management_rollback() {
    local now details
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    details=$(jq -cn --arg now "$now" --arg hostname "$SO_CF_COCKPIT_HOSTNAME" \
        '{status:"passed",completed_at:$now,management_hostname:$hostname,verification_codes:["management_dns_restored"]}')
    state_mark_phase management-rollback "$details"
}

run_cockpit_rollback() {
    local run_id=$1 expected_zone expected_account
    state_load "$run_id" || return
    state_assert_operation configure-cockpit migrate-vps || return
    _management_was_published || die 78 'Rollback checkpoint has no published management DNS' || return
    _restore_checkpoint_outputs || return
    load_cloudflare_bootstrap_token || return
    expected_zone=$SO_CF_ZONE_ID
    expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since management cutover' || return
    cf_cockpit_preflight || return
    cf_cockpit_assert_published_state || return
    _print_management_rollback_diff
    confirm_exact "ROLLBACK COCKPIT $SO_CF_COCKPIT_HOSTNAME" \
        "Type ROLLBACK COCKPIT $SO_CF_COCKPIT_HOSTNAME to restore management DNS: " || die 64 'Cockpit rollback was not confirmed' || return
    cf_cockpit_restore_dns || return
    _mark_management_rollback || return
    info "Cockpit DNS rollback completed for run $SO_RUN_ID. Both Tunnels and VPSs were retained."
}

run_dns_rollback() {
    local run_id=$1 expected_zone expected_account now details
    state_load "$run_id" || return
    state_assert_operation migrate-vps || return
    state_phase_done snapshot || die 78 'Rollback checkpoint has no DNS snapshot' || return
    state_phase_done dns-cutover || die 78 'Rollback is unavailable because DNS cutover was not checkpointed' || return
    _restore_checkpoint_outputs || return
    load_cloudflare_bootstrap_token || return
    expected_zone=$SO_CF_ZONE_ID
    expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since cutover' || return
    cf_assert_cutover_state || return
    if _management_was_published; then
        cf_cockpit_preflight || return
        cf_cockpit_assert_published_state || return
        _print_management_rollback_diff
    fi
    _print_cutover_diff rollback
    confirm_exact "ROLLBACK $SO_DNS_ORIGINAL_IP" "Type ROLLBACK $SO_DNS_ORIGINAL_IP to restore DNS: " || die 64 'DNS rollback was not confirmed' || return
    cf_apply_dns_batch rollback || return
    cf_wait_for_record_content "$SO_DNS_ORIGINAL_IP" || return
    cf_wait_for_proxy_resolution "${SO_CONFIG[target]}" || return
    if _management_was_published; then
        cf_cockpit_restore_dns || return
        _mark_management_rollback || return
    fi
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    details=$(jq -cn --arg now "$now" --arg original "$SO_DNS_ORIGINAL_IP" '{status:"passed",completed_at:$now,original_ip:$original,verification_codes:["dns_rollback_applied","proxy_resolved"]}')
    state_mark_phase dns-rollback "$details" || return
    info "DNS rollback completed for run $SO_RUN_ID. The replacement VPS and GitHub configuration were retained."
}

schoolorbit_main() {
    parse_args "$@" || return
    case "$SO_COMMAND" in
        rollback-cockpit)
            run_cockpit_rollback "$SO_COCKPIT_ROLLBACK_RUN_ID"
            ;;
        rollback-dns)
            run_dns_rollback "$SO_ROLLBACK_RUN_ID"
            ;;
        migrate-vps)
            if [[ -n $SO_RESUME_RUN_ID ]]; then
                state_load "$SO_RESUME_RUN_ID" || return
                state_assert_operation migrate-vps || return
                load_inputs || return
                state_assert_fingerprint || return
                _restore_checkpoint_outputs || return
            else
                load_inputs || return
                if [[ $SO_DRY_RUN == true ]]; then
                    SO_STATE_FILE=
                    run_migration_dry_run
                    return
                fi
                state_init "$(generate_run_id)" || return
                info "Installer run: $SO_RUN_ID"
                info "Checkpoint: $SO_STATE_FILE"
            fi

            if run_migration_phases; then
                vps_cleanup_transients
                return 0
            else
                local status=$?
                vps_cleanup_transients
                _report_migration_failure "$status"
                return "$status"
            fi
            ;;
        configure-cockpit)
            if [[ -n $SO_COCKPIT_RESUME_RUN_ID ]]; then
                state_load "$SO_COCKPIT_RESUME_RUN_ID" || return
                state_assert_operation configure-cockpit || return
                load_cockpit_inputs || return
                state_assert_fingerprint || return
                _restore_checkpoint_outputs || return
            else
                load_cockpit_inputs || return
                if [[ $SO_DRY_RUN == true ]]; then
                    SO_STATE_FILE=
                    run_cockpit_dry_run
                    return
                fi
                state_init "$(generate_run_id)" || return
                info "Installer run: $SO_RUN_ID"
                info "Checkpoint: $SO_STATE_FILE"
            fi

            if run_cockpit_phases; then
                vps_cleanup_transients
                return 0
            else
                local status=$?
                vps_cleanup_transients
                _report_cockpit_failure "$status"
                return "$status"
            fi
            ;;
    esac
}
