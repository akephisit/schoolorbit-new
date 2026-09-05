#!/usr/bin/env bash

set -euo pipefail

usage() {
    printf '%s\n' 'Usage: prune_runtime_images.sh REPOSITORY KEEP_COUNT' >&2
    exit 64
}

fail() {
    local status=$1
    shift
    printf '%s\n' "$*" >&2
    exit "$status"
}

normalize_image_id() {
    local image_id=${1#sha256:}
    [[ $image_id =~ ^[0-9a-f]{64}$ ]] || return 1
    printf '%s\n' "$image_id"
}

[[ $# -eq 2 ]] || usage

repository=$1
keep_count=$2

case "$repository" in
    ghcr.io/akephisit/schoolorbit-backend-admin | ghcr.io/akephisit/schoolorbit-backend-school) ;;
    *) fail 64 'Unsupported runtime image repository' ;;
esac

if [[ ! $keep_count =~ ^[0-9]+$ ]] || ((keep_count < 1 || keep_count > 20)); then
    fail 64 'Retention count must be an integer from 1 through 20'
fi

if ! inventory=$(podman images --sort created --format '{{.Repository}}|{{.Tag}}|{{.Id}}' "$repository"); then
    fail 69 'Unable to enumerate runtime images'
fi

declare -a release_tags=()
declare -a release_ids=()
while IFS='|' read -r listed_repository tag raw_image_id extra; do
    if [[ -z $listed_repository && -z $tag && -z $raw_image_id && -z $extra ]]; then
        continue
    fi
    if [[ $listed_repository != "$repository" || -z $tag || -n $extra ]]; then
        fail 65 'Runtime image inventory is malformed'
    fi
    if ! image_id=$(normalize_image_id "$raw_image_id"); then
        fail 65 'Runtime image inventory is malformed'
    fi
    if [[ $tag =~ ^[0-9a-f]{40}$ ]]; then
        release_tags+=("$tag")
        release_ids+=("$image_id")
    fi
done <<<"$inventory"

collect_protected_ids() {
    declare -gA protected_image_ids=()

    local active_ids raw_image_id image_id alias reference exists_status
    if ! active_ids=$(podman ps -a --no-trunc --format '{{.ImageID}}'); then
        return 1
    fi
    while IFS= read -r raw_image_id; do
        [[ -n $raw_image_id ]] || continue
        image_id=$(normalize_image_id "$raw_image_id") || return 1
        protected_image_ids["$image_id"]=1
    done <<<"$active_ids"

    for alias in latest rollback; do
        reference="${repository}:${alias}"
        if podman image exists "$reference"; then
            raw_image_id=$(podman image inspect --format '{{.Id}}' "$reference") || return 1
            image_id=$(normalize_image_id "$raw_image_id") || return 1
            protected_image_ids["$image_id"]=1
        else
            exists_status=$?
            ((exists_status == 1)) || return 1
        fi
    done
}

collect_protected_ids || fail 69 'Unable to resolve protected runtime images'

declare -a candidate_tags=()
declare -a candidate_ids=()
retained=0
for index in "${!release_tags[@]}"; do
    tag=${release_tags[$index]}
    image_id=${release_ids[$index]}
    if ((index < keep_count)) || [[ -n ${protected_image_ids[$image_id]:-} ]]; then
        ((retained += 1))
    else
        candidate_tags+=("$tag")
        candidate_ids+=("$image_id")
    fi
done

removed=0
for index in "${!candidate_tags[@]}"; do
    tag=${candidate_tags[$index]}
    image_id=${candidate_ids[$index]}
    collect_protected_ids || fail 69 'Unable to revalidate protected runtime images'
    if [[ -n ${protected_image_ids[$image_id]:-} ]]; then
        fail 75 'Runtime image became protected during cleanup'
    fi
    podman image rm "${repository}:${tag}"
    printf 'runtime_image_removed repository=%s tag=%s\n' "$repository" "$tag"
    ((removed += 1))
done

retained=$((retained + ${#candidate_tags[@]} - removed))
printf 'runtime_image_cleanup repository=%s before=%d retained=%d removed=%d\n' \
    "$repository" "${#release_tags[@]}" "$retained" "$removed"

if ! podman system df --format '{{.Type}} {{.Total}} {{.Size}} {{.Reclaimable}}'; then
    printf 'runtime_image_storage status=unavailable\n'
fi
