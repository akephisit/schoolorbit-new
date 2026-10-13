#!/usr/bin/env bash

schoolorbit_container_network_state() {
    local container=${1:?Container name is required}
    podman inspect --format '{{json .NetworkSettings.Networks}}' "$container"
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

schoolorbit_network_alias_resolves() {
    local probe_container=${1:?DNS probe container is required}
    local alias=${2:?DNS alias is required}
    local expected_ip=${3:?Expected address is required}
    local answer
    if ! answer=$(timeout 5 podman exec "$probe_container" getent hosts "$alias" 2>/dev/null); then
        return 1
    fi
    printf '%s\n' "$answer" | awk -v expected="$expected_ip" '$1 == expected { found=1 } END { exit !found }'
}

# Runtime connections may already exist when this gate runs. Never detach, connect,
# restart or recreate a running service here. Compose owns aliases before startup.
schoolorbit_validate_container_network_aliases() {
    local network=${1:?Network name is required}
    local container=${2:?Container name is required}
    local service_alias=${3:?Service alias is required}
    local container_alias=${4:?Container alias is required}
    local probe_container=${5:?DNS probe container is required}
    local network_state expected_ip
    local -i attempt

    podman container exists "$container" || {
        printf 'Required container is unavailable: %s\n' "$container" >&2
        return 69
    }
    if ! network_state=$(schoolorbit_container_network_state "$container"); then
        printf 'Unable to inspect network state for %s\n' "$container" >&2
        return 1
    fi
    if ! schoolorbit_network_has_aliases "$network_state" "$network" "$service_alias" "$container_alias" ||
        ! expected_ip=$(printf '%s\n' "$network_state" | jq -er --arg network "$network" '.[$network].IPAddress // empty'); then
        printf 'Network attachment/aliases are incomplete for %s; recreate it through the release with declarative aliases\n' "$container" >&2
        return 1
    fi

    for ((attempt = 1; attempt <= 5; attempt++)); do
        if timeout 5 podman exec "$probe_container" true >/dev/null 2>&1 &&
            schoolorbit_network_alias_resolves "$probe_container" "$service_alias" "$expected_ip" &&
            schoolorbit_network_alias_resolves "$probe_container" "$container_alias" "$expected_ip"; then
            return 0
        fi
        if ((attempt < 5)); then
            sleep 2
        fi
    done
    printf 'Network DNS does not resolve both aliases to the current address for %s; maintenance remains active\n' "$container" >&2
    return 1
}
