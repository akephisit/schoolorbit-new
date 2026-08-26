#!/usr/bin/env bash

schoolorbit_container_network_state() {
    local container=${1:?Container name is required}
    podman inspect --format '{{json .NetworkSettings.Networks}}' "$container"
}

schoolorbit_network_is_attached() {
    local network_state=${1:?Network state is required}
    local network=${2:?Network name is required}
    printf '%s\n' "$network_state" |
        jq -e --arg network "$network" '.[$network] != null' >/dev/null
}

schoolorbit_network_has_aliases() {
    local network_state=${1:?Network state is required}
    local network=${2:?Network name is required}
    local service_alias=${3:?Service alias is required}
    local container_alias=${4:?Container alias is required}
    printf '%s\n' "$network_state" |
        jq -e \
            --arg network "$network" \
            --arg service_alias "$service_alias" \
            --arg container_alias "$container_alias" \
            '
              (.[$network].Aliases // []) as $aliases |
              ($aliases | index($service_alias)) != null and
              ($aliases | index($container_alias)) != null
            ' >/dev/null
}

schoolorbit_ensure_container_network_aliases() {
    local network=${1:?Network name is required}
    local container=${2:?Container name is required}
    local service_alias=${3:?Service alias is required}
    local container_alias=${4:?Container alias is required}
    local network_state="" was_attached=false
    local -i attempt=1

    podman container exists "$container" || {
        printf 'Required container is unavailable: %s\n' "$container" >&2
        return 69
    }
    if ! network_state=$(schoolorbit_container_network_state "$container"); then
        printf 'Unable to inspect network state for %s\n' "$container" >&2
        return 1
    fi
    if schoolorbit_network_has_aliases \
        "$network_state" "$network" "$service_alias" "$container_alias"; then
        return 0
    fi

    if schoolorbit_network_is_attached "$network_state" "$network"; then
        was_attached=true
        if ! podman network disconnect -f "$network" "$container" >/dev/null 2>&1; then
            printf 'Unable to detach stale network aliases for %s\n' "$container" >&2
            return 1
        fi
    fi

    while ((attempt <= 5)); do
        podman network connect \
            --alias "$service_alias" \
            --alias "$container_alias" \
            "$network" "$container" >/dev/null 2>&1 || true
        if network_state=$(schoolorbit_container_network_state "$container"); then
            if schoolorbit_network_has_aliases \
                "$network_state" "$network" "$service_alias" "$container_alias"; then
                return 0
            fi
            if schoolorbit_network_is_attached "$network_state" "$network"; then
                podman network disconnect -f "$network" "$container" >/dev/null 2>&1 || break
            fi
        fi
        if ((attempt < 5)); then
            sleep 2
        fi
        ((attempt += 1))
    done

    if [[ $was_attached == true ]]; then
        if network_state=$(schoolorbit_container_network_state "$container") &&
            ! schoolorbit_network_is_attached "$network_state" "$network"; then
            attempt=1
            while ((attempt <= 5)); do
                if podman network connect "$network" "$container" >/dev/null 2>&1; then
                    break
                fi
                if ((attempt < 5)); then
                    sleep 2
                fi
                ((attempt += 1))
            done
        fi
        if ! network_state=$(schoolorbit_container_network_state "$container") ||
            ! schoolorbit_network_is_attached "$network_state" "$network"; then
            printf 'Unable to restore network membership for %s\n' "$container" >&2
            return 1
        fi
    fi

    printf 'Unable to apply required network aliases for %s\n' "$container" >&2
    return 1
}
