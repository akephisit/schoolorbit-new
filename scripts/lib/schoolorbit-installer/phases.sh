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
    handoff
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
    SO_PHASE_DETAILS=$(jq -cn \
        --arg zone "$SO_CF_ZONE_ID" \
        --arg account "$SO_CF_ACCOUNT_ID" \
        --arg original "$SO_DNS_ORIGINAL_IP" \
        --arg etag "$SO_DNS_SNAPSHOT_ETAG" \
        --argjson dns "$SO_DNS_SNAPSHOT" \
        '{cloudflare_zone_id:$zone,cloudflare_account_id:$account,original_ip:$original,dns_snapshot_etag:$etag,dns_snapshot:$dns}')
}

_phase_bootstrap_apply() {
    vps_bootstrap || return
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

_print_handoff() {
    info "Migration run: $SO_RUN_ID"
    info "Checkpoint: $SO_STATE_FILE"
    jq -r '.[] | "Workflow \(.workflow): \(.url)"' <<<"$SO_WORKFLOW_RUNS" | while IFS= read -r line; do info "$line"; done
    info "Origin certificate: $SO_CF_CERTIFICATE_ID (expires $SO_CF_CERTIFICATE_EXPIRES)"
    info "Origin IP: $SO_DNS_ORIGINAL_IP -> ${SO_CONFIG[target]}"
    info 'Deployment gates: runtime=true, frontend=true'
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
}

_phase_snapshot_reverify() {
    local expected_zone=$SO_CF_ZONE_ID expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since the snapshot' || return
    if state_phase_done dns-cutover; then
        cf_assert_cutover_state
    else
        # shellcheck disable=SC2034 # Forces the provider drift check to re-read DNS.
        SO_DNS_CURRENT_ETAG=
        cf_assert_no_dns_drift
    fi
}

_phase_reverify() {
    local phase=$1
    case "$phase" in
        preflight) _phase_preflight_apply ;;
        input) state_assert_fingerprint ;;
        snapshot) _phase_snapshot_reverify ;;
        bootstrap) vps_reverify_bootstrap ;;
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

run_migration_dry_run() {
    local phase
    for phase in preflight input snapshot; do
        SO_CURRENT_PHASE=$phase
        _phase_record_apply "$phase"
        _phase_apply "$phase" || return
    done
    info 'Dry-run plan (no mutations applied)'
    _print_cutover_diff cutover
    info 'Planned mutations: bootstrap VPS, install TLS/runtime, configure GitHub, dispatch four workflows, confirm DNS cutover, verify public services, enable rollout gates.'
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

run_dns_rollback() {
    local run_id=$1 expected_zone expected_account now details
    state_load "$run_id" || return
    state_phase_done snapshot || die 78 'Rollback checkpoint has no DNS snapshot' || return
    state_phase_done dns-cutover || die 78 'Rollback is unavailable because DNS cutover was not checkpointed' || return
    _restore_checkpoint_outputs || return
    load_cloudflare_bootstrap_token || return
    expected_zone=$SO_CF_ZONE_ID
    expected_account=$SO_CF_ACCOUNT_ID
    cf_preflight || return
    [[ $SO_CF_ZONE_ID == "$expected_zone" && $SO_CF_ACCOUNT_ID == "$expected_account" ]] || die 78 'Cloudflare zone changed since cutover' || return
    cf_assert_cutover_state || return
    _print_cutover_diff rollback
    confirm_exact "ROLLBACK $SO_DNS_ORIGINAL_IP" "Type ROLLBACK $SO_DNS_ORIGINAL_IP to restore DNS: " || die 64 'DNS rollback was not confirmed' || return
    cf_apply_dns_batch rollback || return
    cf_wait_for_record_content "$SO_DNS_ORIGINAL_IP" || return
    cf_wait_for_proxy_resolution "${SO_CONFIG[target]}" || return
    now=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
    details=$(jq -cn --arg now "$now" --arg original "$SO_DNS_ORIGINAL_IP" '{status:"passed",completed_at:$now,original_ip:$original,verification_codes:["dns_rollback_applied","proxy_resolved"]}')
    state_mark_phase dns-rollback "$details" || return
    info "DNS rollback completed for run $SO_RUN_ID. The replacement VPS and GitHub configuration were retained."
}

schoolorbit_main() {
    parse_args "$@" || return
    case "$SO_COMMAND" in
        rollback-dns)
            run_dns_rollback "$SO_ROLLBACK_RUN_ID"
            ;;
        migrate-vps)
            if [[ -n $SO_RESUME_RUN_ID ]]; then
                state_load "$SO_RESUME_RUN_ID" || return
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
    esac
}
