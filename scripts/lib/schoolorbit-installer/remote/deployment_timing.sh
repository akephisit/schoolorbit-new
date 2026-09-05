#!/usr/bin/env bash

schoolorbit_timer_now() {
    local now=${SCHOOLORBIT_TIMER_NOW:-}

    if [[ -z $now ]]; then
        now=$(date +%s)
    fi
    if [[ ! $now =~ ^[0-9]+$ ]]; then
        printf '%s\n' 'Invalid deployment timing clock' >&2
        return 64
    fi

    printf '%s\n' "$now"
}

schoolorbit_timer_report() {
    local phase=${1:-}
    local start=${2:-}
    local end

    if [[ ! $phase =~ ^[a-z0-9_]+$ ]]; then
        printf '%s\n' 'Invalid deployment timing phase' >&2
        return 64
    fi
    if [[ ! $start =~ ^[0-9]+$ ]]; then
        printf '%s\n' 'Invalid deployment timing start' >&2
        return 64
    fi

    end=$(schoolorbit_timer_now) || return
    if ((end < start)); then
        printf '%s\n' 'Deployment timing clock moved backwards' >&2
        return 65
    fi

    printf 'deployment_timing phase=%s seconds=%d\n' "$phase" "$((end - start))"
}
