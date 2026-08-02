#!/usr/bin/env bash

SO_GITHUB_RUN_ID=
SO_GITHUB_RUN_URL=

_github_valid_name() {
    [[ $1 =~ ^[A-Z][A-Z0-9_]{0,127}$ ]]
}

github_preflight() {
    require_command gh || return
    require_command jq || return
    gh auth status --hostname github.com >/dev/null 2>&1 || die 69 'GitHub CLI is not authenticated for github.com' || return

    local permission
    permission=$(gh api "repos/${SO_CONFIG[repository]}/actions/permissions/workflow" --jq '.default_workflow_permissions' 2>/dev/null) || die 69 'Unable to read GitHub Actions permissions' || return
    [[ $permission == write ]] || die 69 'GitHub Actions must have read/write workflow permission'
}

github_set_variable() {
    local name=$1 value=$2
    _github_valid_name "$name" || die 64 'Invalid GitHub variable name' || return
    [[ $value != *$'\n'* && $value != *$'\r'* ]] || die 64 "GitHub variable $name contains a newline" || return
    gh variable set "$name" --body "$value" --repo "${SO_CONFIG[repository]}" >/dev/null || die 69 "Unable to set GitHub variable $name"
}

github_set_secret() {
    local name=$1 value=$2
    _github_valid_name "$name" || die 64 'Invalid GitHub secret name' || return
    [[ -n $value ]] || die 64 "GitHub secret $name is empty" || return
    printf '%s' "$value" | gh secret set "$name" --repo "${SO_CONFIG[repository]}" >/dev/null || die 69 "Unable to set GitHub secret $name"
}

github_configure_repository() {
    github_preflight || return
    [[ -n ${SO_CF_ACCOUNT_ID-} ]] || die 78 'Cloudflare account ID is not loaded' || return

    github_set_variable BASE_DOMAIN "${SO_CONFIG[base_domain]}" || return
    github_set_variable BACKEND_ADMIN_URL "https://admin-api.${SO_CONFIG[base_domain]}" || return
    github_set_variable BACKEND_SCHOOL_URL "https://school-api.${SO_CONFIG[base_domain]}" || return
    github_set_variable CLOUDFLARE_ACCOUNT_ID "$SO_CF_ACCOUNT_ID" || return
    github_set_variable VAPID_PUBLIC_KEY "${SO_CONFIG["runtime:VAPID_PUBLIC_KEY"]}" || return
    github_set_variable RUNTIME_DEPLOY_ENABLED false || return
    github_set_variable FRONTEND_DEPLOY_ENABLED false || return

    github_set_secret SERVER_IP "${SO_CONFIG[target]}" || return
    github_set_secret SERVER_USER "${SO_CONFIG[server_user]}" || return
    github_set_secret SSH_PRIVATE_KEY "${SO_SECRETS[SSH_PRIVATE_KEY]-}" || return
    github_set_secret CLOUDFLARE_API_TOKEN "${SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN]-}" || return
    github_set_secret INTERNAL_API_SECRET "${SO_SECRETS[INTERNAL_API_SECRET]-}" || return
    github_set_secret DEPLOY_KEY "${SO_SECRETS[DEPLOY_KEY]-}" || return
    github_set_secret SMOKE_USERNAME "${SO_SECRETS[SMOKE_USERNAME]-}" || return
    github_set_secret SMOKE_PASSWORD "${SO_SECRETS[SMOKE_PASSWORD]-}"
}

_github_expected_title() {
    local workflow=$1 deployment_id=$2
    case "$workflow" in
        deploy-backend-admin.yml) printf 'Deploy Backend Admin (%s)\n' "$deployment_id" ;;
        deploy-backend-school.yml) printf 'Deploy Backend School (%s)\n' "$deployment_id" ;;
        deploy-frontend-admin.yml) printf 'Deploy Frontend Admin (%s)\n' "$deployment_id" ;;
        deploy-all-schools.yml) printf 'Deploy Frontend Schools (%s)\n' "$deployment_id" ;;
        *) die 64 'Unsupported deployment workflow' ;;
    esac
}

github_dispatch_and_wait() {
    local workflow=$1 deployment_id=$2 expected_title runs matches count attempt
    local attempts=${SO_PROVIDER_POLL_ATTEMPTS:-20}
    local delay=${SO_PROVIDER_POLL_DELAY:-3}
    _valid_run_id "$deployment_id" || die 64 'Invalid deployment ID' || return
    expected_title=$(_github_expected_title "$workflow" "$deployment_id") || return

    gh workflow run "$workflow" --repo "${SO_CONFIG[repository]}" \
        --ref "${SO_CONFIG[ref]}" -f "deployment_id=$deployment_id" >/dev/null || die 69 "Unable to dispatch $workflow" || return

    for ((attempt = 1; attempt <= attempts; attempt++)); do
        runs=$(gh run list --repo "${SO_CONFIG[repository]}" --workflow "$workflow" \
            --event workflow_dispatch --json databaseId,displayTitle,status,conclusion,url 2>/dev/null) || die 69 "Unable to list runs for $workflow" || return
        matches=$(jq -c --arg title "$expected_title" '[.[] | select(.displayTitle == $title)]' <<<"$runs") || die 69 'GitHub returned an invalid workflow run response' || return
        count=$(jq 'length' <<<"$matches")
        if ((count == 1)); then
            break
        fi
        ((count == 0)) || die 78 'More than one correlated GitHub workflow run was found' || return
        ((attempt < attempts)) && sleep "$delay"
    done

    ((count == 1)) || die 75 'Timed out waiting for the correlated GitHub workflow run' || return
    SO_GITHUB_RUN_ID=$(jq -er '.[0].databaseId | numbers' <<<"$matches") || die 69 'GitHub workflow run ID is invalid' || return
    SO_GITHUB_RUN_URL=$(jq -er '.[0].url | strings' <<<"$matches") || die 69 'GitHub workflow run URL is invalid' || return

    if ! gh run watch "$SO_GITHUB_RUN_ID" --repo "${SO_CONFIG[repository]}" --exit-status; then
        warn "GitHub workflow failed: $SO_GITHUB_RUN_URL"
        return 1
    fi
}
