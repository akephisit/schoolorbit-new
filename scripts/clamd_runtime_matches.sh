#!/usr/bin/env bash
set -euo pipefail

drift() {
    local reason=${1:?Drift reason is required}
    printf 'clamd_drift reason=%s\n' "$reason"
    exit 1
}

normalize_image_id() {
    local image_id=${1:-}
    image_id=${image_id#sha256:}
    [[ $image_id =~ ^[0-9a-f]{64}$ ]] || return 1
    printf '%s\n' "$image_id"
}

image=${1:-}
container=${2:-}
if [[ $image != docker.io/clamav/clamav-debian:1.5.3 || $container != schoolorbit-clamd ]]; then
    printf '%s\n' 'Unsupported ClamAV runtime target' >&2
    exit 64
fi

if podman container exists "$container" >/dev/null 2>&1; then
    :
else
    container_status=$?
    if ((container_status == 1)); then
        drift missing_container
    fi
    drift container_inspect
fi

if ! desired_image_id=$(podman image inspect --format '{{.Id}}' "$image" 2>/dev/null); then
    drift image_inspect
fi
if ! desired_image_id=$(normalize_image_id "$desired_image_id"); then
    drift image_inspect
fi
if ! running_image_id=$(podman inspect --format '{{.Image}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
if ! running_image_id=$(normalize_image_id "$running_image_id"); then
    drift container_inspect
fi
[[ $running_image_id == "$desired_image_id" ]] || drift image

if ! memory=$(podman inspect --format '{{.HostConfig.Memory}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $memory == 3221225472 ]] || drift memory

if ! nano_cpus=$(podman inspect --format '{{.HostConfig.NanoCpus}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $nano_cpus == 1000000000 ]] || drift cpu

if ! pids_limit=$(podman inspect --format '{{.HostConfig.PidsLimit}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $pids_limit == 256 ]] || drift pids

if ! restart_policy=$(podman inspect --format '{{.HostConfig.RestartPolicy.Name}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $restart_policy == unless-stopped ]] || drift restart

if ! security_opt=$(podman inspect --format '{{json .HostConfig.SecurityOpt}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
if ! jq -e '
    type == "array" and
    any(.[]; . == "no-new-privileges" or . == "no-new-privileges:true")
' >/dev/null 2>&1 <<<"$security_opt"; then
    drift security
fi

if ! port_bindings=$(podman inspect --format '{{json .HostConfig.PortBindings}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
if ! jq -e '(. == null) or (type == "object" and length == 0)' \
    >/dev/null 2>&1 <<<"$port_bindings"; then
    drift published_port
fi

if ! mounts=$(podman inspect --format '{{json .Mounts}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
if ! jq -e '
    type == "array" and
    ([.[] | select(.Destination == "/var/lib/clamav")] | length == 1) and
    any(.[];
        .Type == "volume" and
        .Name == "schoolorbit-clamav-signatures" and
        .Destination == "/var/lib/clamav"
    )
' >/dev/null 2>&1 <<<"$mounts"; then
    drift signature_volume
fi

if ! networks=$(podman inspect --format '{{json .NetworkSettings.Networks}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
if ! jq -e '
    type == "object" and
    length == 2 and
    has("schoolorbit-file-platform-internal") and
    has("schoolorbit-clamav-egress")
' >/dev/null 2>&1 <<<"$networks"; then
    drift network
fi

if ! running=$(podman inspect --format '{{.State.Running}}' "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $running == true ]] || drift running

if ! health=$(podman inspect --format \
    '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' \
    "$container" 2>/dev/null); then
    drift container_inspect
fi
[[ $health == healthy ]] || drift health

printf '%s\n' 'clamd_action=reused'
