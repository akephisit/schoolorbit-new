#!/usr/bin/env bash

declare -gA SO_CONFIG=()
declare -gA SO_SECRETS=()
declare -ga SO_REQUIRED_SECRETS=(
    SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN
    SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN
    SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN
    SCHOOLORBIT_SERVER_PASSWORD
    DATABASE_URL
    JWT_SECRET
    SESSION_HMAC_KEY
    SCHOOL_ROLLBACK_JWT_SECRET
    INTERNAL_API_SECRET
    ENCRYPTION_KEY
    BLIND_INDEX_KEY
    DEPLOY_KEY
    NEON_API_KEY
    NEON_DB_PASSWORD
    R2_ACCESS_KEY_ID
    R2_SECRET_ACCESS_KEY
    VAPID_PRIVATE_KEY
    SCHOOLORBIT_RUNTIME_GITHUB_TOKEN
    SMOKE_SUBDOMAIN
    SMOKE_USERNAME
    SMOKE_PASSWORD
)
declare -ga SO_REQUIRED_COCKPIT_SECRETS=(
    SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN
    SCHOOLORBIT_SERVER_PASSWORD
)
declare -ga SO_REQUIRED_RUNTIME_VALUES=(
    NEON_PROJECT_ID
    NEON_BRANCH_ID
    NEON_HOST
    R2_ACCOUNT_ID
    R2_PUBLIC_BUCKET_NAME
    R2_PRIVATE_BUCKET_NAME
    R2_PUBLIC_URL
    VAPID_PUBLIC_KEY
)

SO_COMMAND=
SO_DRY_RUN=false
SO_SECRETS_STDIN=false
SO_RESUME_RUN_ID=
SO_ROLLBACK_RUN_ID=
SO_COCKPIT_RESUME_RUN_ID=
SO_COCKPIT_ROLLBACK_RUN_ID=

# shellcheck disable=SC2034 # Resume IDs are consumed by the orchestration module after parsing.
_config_reset() {
    SO_CONFIG=()
    SO_SECRETS=()
    SO_COMMAND=
    SO_DRY_RUN=false
    SO_SECRETS_STDIN=false
    SO_RESUME_RUN_ID=
    SO_ROLLBACK_RUN_ID=
    SO_COCKPIT_RESUME_RUN_ID=
    SO_COCKPIT_ROLLBACK_RUN_ID=
}

_valid_domain() {
    [[ $1 =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]]
}

_valid_ipv4() {
    local address=$1 octet
    local -a octets
    IFS=. read -r -a octets <<<"$address"
    ((${#octets[@]} == 4)) || return 1
    for octet in "${octets[@]}"; do
        [[ $octet =~ ^[0-9]{1,3}$ ]] || return 1
        ((10#$octet <= 255)) || return 1
    done
}

_valid_repository() {
    [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$ ]]
}

_valid_ref() {
    local ref=$1
    [[ $ref =~ ^[A-Za-z0-9][A-Za-z0-9._/-]{0,254}$ ]] || return 1
    [[ $ref != *..* && $ref != *@\{* && $ref != *//* && $ref != */ && $ref != *. ]]
}

_valid_run_id() {
    [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]]
}

_parse_value_option() {
    local option=$1
    shift
    (($# > 0)) || die 64 "Missing value for $option"
}

parse_args() {
    _config_reset
    (($# > 0)) || die 64 'A command is required' || return
    SO_COMMAND=$1
    shift

    case "$SO_COMMAND" in
        migrate-vps)
            _parse_migrate_args "$@" || return
            ;;
        configure-cockpit)
            _parse_cockpit_args "$@" || return
            ;;
        rollback-dns)
            _parse_rollback_args "$@" || return
            ;;
        rollback-cockpit)
            _parse_cockpit_rollback_args "$@" || return
            ;;
        *)
            die 64 'Unsupported installer command'
            return
            ;;
    esac
}

_parse_migrate_args() {
    _parse_connection_args SO_RESUME_RUN_ID migrate-vps "$@"
}

_parse_cockpit_args() {
    _parse_connection_args SO_COCKPIT_RESUME_RUN_ID configure-cockpit "$@"
}

_parse_connection_args() {
    local resume_variable=$1 command=$2
    local -n resume_run_id=$resume_variable
    shift 2

    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22

    while (($# > 0)); do
        case "$1" in
            --repository | --target | --base-domain | --ref | --bootstrap-user | --server-user | --ssh-port)
                local option=$1 value key
                shift
                _parse_value_option "$option" "$@" || return
                value=$1
                shift
                key=${option#--}
                key=${key//-/_}
                SO_CONFIG[$key]=$value
                ;;
            --resume)
                shift
                _parse_value_option --resume "$@" || return
                resume_run_id=$1
                shift
                ;;
            --dry-run)
                SO_DRY_RUN=true
                shift
                ;;
            --secrets-stdin)
                SO_SECRETS_STDIN=true
                shift
                ;;
            --*secret* | --*token* | --*password* | --*key* | --database-url)
                die 64 'Secrets are not accepted as command-line options'
                return
                ;;
            *)
                die 64 "Unsupported $command option"
                return
                ;;
        esac
    done

    if [[ -n $resume_run_id ]]; then
        _valid_run_id "$resume_run_id" || die 64 'Invalid resume run ID' || return
        ((${#SO_CONFIG[@]} == 5)) || die 64 'Resume accepts only --resume RUN_ID' || return
        [[ $SO_DRY_RUN == false && $SO_SECRETS_STDIN == false ]] || die 64 'Resume accepts only --resume RUN_ID' || return
        return 0
    fi

    [[ -n ${SO_CONFIG[repository]-} ]] || die 64 'Repository is required' || return
    [[ -n ${SO_CONFIG[target]-} ]] || die 64 'Target IPv4 address is required' || return
    _valid_repository "${SO_CONFIG[repository]}" || die 64 'Invalid OWNER/REPOSITORY' || return
    _valid_ipv4 "${SO_CONFIG[target]}" || die 64 'Invalid target IPv4 address' || return
    _valid_domain "${SO_CONFIG[base_domain]}" || die 64 'Invalid base domain' || return
    _valid_ref "${SO_CONFIG[ref]}" || die 64 'Invalid Git ref' || return
    [[ ${SO_CONFIG[bootstrap_user]} =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || die 64 'Invalid bootstrap user' || return
    [[ ${SO_CONFIG[server_user]} =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || die 64 'Invalid server user' || return
    [[ ${SO_CONFIG[ssh_port]} =~ ^[0-9]{1,5}$ ]] || die 64 'Invalid SSH port' || return
    ((10#${SO_CONFIG[ssh_port]} >= 1 && 10#${SO_CONFIG[ssh_port]} <= 65535)) || die 64 'Invalid SSH port'
}

_parse_rollback_args() {
    _parse_run_id_only_args SO_ROLLBACK_RUN_ID rollback-dns "$@"
}

_parse_cockpit_rollback_args() {
    _parse_run_id_only_args SO_COCKPIT_ROLLBACK_RUN_ID rollback-cockpit "$@"
}

_parse_run_id_only_args() {
    local run_id_variable=$1 command=$2
    local -n rollback_run_id=$run_id_variable
    shift 2

    while (($# > 0)); do
        case "$1" in
            --run-id)
                shift
                _parse_value_option --run-id "$@" || return
                rollback_run_id=$1
                shift
                ;;
            *)
                die 64 "$command accepts only --run-id RUN_ID"
                return
                ;;
        esac
    done

    [[ -n $rollback_run_id ]] || die 64 'Rollback run ID is required' || return
    _valid_run_id "$rollback_run_id" || die 64 'Invalid rollback run ID'
}

_contains_unsafe_input() {
    local value=$1
    [[ $value == *$'\n'* || $value == *$'\r'* ]] && return 0
    [[ ${value,,} =~ (change.?me|replace.?me|placeholder|test-only|dummy|your-secret) ]]
}

_validate_secret() {
    local name=$1 value=$2 minimum=16
    if _contains_unsafe_input "$value"; then
        die 64 "Unsafe value supplied for $name"
        return
    fi

    case "$name" in
        DATABASE_URL)
            minimum=24
            [[ $value =~ ^postgres(ql)?:// ]] || die 64 'DATABASE_URL must be a PostgreSQL URL' || return
            ;;
        ENCRYPTION_KEY | BLIND_INDEX_KEY | SESSION_HMAC_KEY | SCHOOL_ROLLBACK_JWT_SECRET)
            minimum=32
            ;;
        NEON_DB_PASSWORD)
            minimum=12
            ;;
        SCHOOLORBIT_SERVER_PASSWORD)
            minimum=10
            ;;
        SMOKE_PASSWORD)
            minimum=1
            ;;
        SMOKE_SUBDOMAIN)
            minimum=2
            [[ $value =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ ]] || die 64 'Invalid smoke subdomain' || return
            ;;
        SMOKE_USERNAME)
            minimum=3
            ;;
    esac

    ((${#value} >= minimum)) || die 64 "Value for $name is too short"
}

_validate_runtime_value() {
    local name=$1 value=$2
    if _contains_unsafe_input "$value"; then
        die 64 "Unsafe value supplied for $name"
        return
    fi

    case "$name" in
        NEON_PROJECT_ID)
            [[ $value =~ ^[A-Za-z0-9][A-Za-z0-9_-]{2,127}$ ]] || die 64 'Invalid Neon project ID'
            ;;
        NEON_BRANCH_ID)
            [[ $value =~ ^br-[a-z0-9-]{1,57}$ ]] || die 64 'Invalid Neon branch ID'
            ;;
        NEON_HOST)
            _valid_domain "${value,,}" || die 64 'Invalid Neon host'
            ;;
        R2_ACCOUNT_ID)
            [[ $value =~ ^[A-Za-z0-9]{16,64}$ ]] || die 64 'Invalid R2 account ID'
            ;;
        R2_PUBLIC_BUCKET_NAME | R2_PRIVATE_BUCKET_NAME)
            [[ $value =~ ^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$ ]] || die 64 "Invalid bucket name for $name"
            ;;
        R2_PUBLIC_URL)
            [[ $value =~ ^https://[^/[:space:]]+/?$ ]] || die 64 'Invalid R2 public URL'
            ;;
        VAPID_PUBLIC_KEY)
            ((${#value} >= 32)) || die 64 'VAPID public key is too short'
            ;;
    esac
}

_read_prompted_value() {
    local name=$1 kind=$2 value
    if [[ $kind == secret ]]; then
        read -r -s -p "$name: " value
        printf '\n' >&2
    else
        read -r -p "$name: " value
    fi
    printf '%s' "$value"
}

_read_input_json() {
    local input_json=''
    if [[ $SO_SECRETS_STDIN == true ]]; then
        input_json=$(cat)
        jq -e 'type == "object"' <<<"$input_json" >/dev/null 2>&1 || die 64 'Secret input must be one JSON object' || return
    fi
    printf '%s' "$input_json"
}

_load_named_secrets() {
    local input_json=$1 name value
    shift
    for name in "$@"; do
        value=${!name-}
        [[ -n $value ]] || value=${SO_SECRETS[$name]-}
        if [[ -z $value && $SO_SECRETS_STDIN == true ]]; then
            value=$(jq -er --arg name "$name" '.[$name] | strings | select(length > 0)' <<<"$input_json" 2>/dev/null) || die 64 "Missing required input: $name" || return
        elif [[ -z $value ]]; then
            value=$(_read_prompted_value "$name" secret) || return
        fi
        _validate_secret "$name" "$value" || return
        # shellcheck disable=SC2034 # Consumed by later installer modules and redaction.
        SO_SECRETS["$name"]=$value
    done
}

load_inputs() {
    local input_json name value
    input_json=$(_read_input_json) || return
    _load_named_secrets "$input_json" "${SO_REQUIRED_SECRETS[@]}" || return
    if [[ ${SO_SECRETS[SESSION_HMAC_KEY]} == "${SO_SECRETS[SCHOOL_ROLLBACK_JWT_SECRET]}" ||
        ${SO_SECRETS[SESSION_HMAC_KEY]} == "${SO_SECRETS[JWT_SECRET]}" ||
        ${SO_SECRETS[SCHOOL_ROLLBACK_JWT_SECRET]} == "${SO_SECRETS[JWT_SECRET]}" ]]; then
        die 64 'School session secrets must be distinct from each other and admin JWT_SECRET'
        return
    fi

    for name in "${SO_REQUIRED_RUNTIME_VALUES[@]}"; do
        value=${!name-}
        [[ -n $value ]] || value=${SO_CONFIG["runtime:$name"]-}
        if [[ -z $value && $SO_SECRETS_STDIN == true ]]; then
            value=$(jq -er --arg name "$name" '.[$name] | strings | select(length > 0)' <<<"$input_json" 2>/dev/null) || die 64 "Missing required input: $name" || return
        elif [[ -z $value ]]; then
            value=$(_read_prompted_value "$name" public) || return
        fi
        _validate_runtime_value "$name" "$value" || return
        SO_CONFIG["runtime:$name"]=$value
    done

    if [[ ${SO_CONFIG["runtime:R2_PUBLIC_BUCKET_NAME"]} == "${SO_CONFIG["runtime:R2_PRIVATE_BUCKET_NAME"]}" ]]; then
        die 64 'Public and private R2 buckets must be different'
    fi
}

load_cockpit_inputs() {
    local input_json
    input_json=$(_read_input_json) || return
    _load_named_secrets "$input_json" "${SO_REQUIRED_COCKPIT_SECRETS[@]}"
}

load_cloudflare_bootstrap_token() {
    local name=SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN value
    value=${!name-}
    [[ -n $value ]] || value=${SO_SECRETS[$name]-}
    if [[ -z $value ]]; then
        value=$(_read_prompted_value "$name" secret) || return
    fi
    _validate_secret "$name" "$value" || return
    # shellcheck disable=SC2034 # Consumed by the Cloudflare provider boundary.
    SO_SECRETS[$name]=$value
}
