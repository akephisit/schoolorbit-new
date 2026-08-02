#!/usr/bin/env bash

redact_text() {
    local value=${1-}
    local secret

    if declare -p SO_SECRETS >/dev/null 2>&1; then
        for secret in "${SO_SECRETS[@]}"; do
            [[ -n $secret ]] && value=${value//"$secret"/'[REDACTED]'}
        done
    fi

    printf '%s' "$value"
}

info() {
    printf '%s\n' "$(redact_text "$*")"
}

warn() {
    printf 'WARNING: %s\n' "$(redact_text "$*")" >&2
}

die() {
    local code=$1
    shift
    printf 'ERROR: %s\n' "$(redact_text "$*")" >&2
    return "$code"
}

retry() {
    local attempts=$1 delay=$2
    shift 2
    local current=1

    until "$@"; do
        if ((current >= attempts)); then
            return 75
        fi
        sleep "$delay"
        delay=$((delay * 2))
        current=$((current + 1))
    done
}

confirm_exact() {
    local expected=$1 prompt=$2 answer
    read -r -p "$prompt" answer
    [[ $answer == "$expected" ]]
}

require_command() {
    local command_name=$1
    command -v "$command_name" >/dev/null 2>&1 || die 69 "Required command is unavailable: $command_name"
}
