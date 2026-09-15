#!/usr/bin/env bash

set -euo pipefail

requested_scope=${1:-}
release_sha=${2:-}
frontend_accepted_sha=${3:-}
backend_accepted_sha=${4:-}
force_full=${5:-true}

if ! printf '%s' "$release_sha" | grep -Eq '^[0-9a-f]{40}$'; then
    echo "Release ID must be a 40-character lowercase Git SHA" >&2
    exit 64
fi

needs_frontend=false
needs_backend=false
scope=$requested_scope

if [[ $scope == auto ]]; then
    if [[ $force_full != false ]] ||
        ! printf '%s' "$frontend_accepted_sha" | grep -Eq '^[0-9a-f]{40}$' ||
        ! printf '%s' "$backend_accepted_sha" | grep -Eq '^[0-9a-f]{40}$' ||
        ! git cat-file -e "${frontend_accepted_sha}^{commit}" 2>/dev/null ||
        ! git cat-file -e "${backend_accepted_sha}^{commit}" 2>/dev/null ||
        ! git merge-base --is-ancestor "$frontend_accepted_sha" "$release_sha" ||
        ! git merge-base --is-ancestor "$backend_accepted_sha" "$release_sha"; then
        # No trustworthy accepted predecessor means a partial release is unsafe.
        needs_frontend=true
        needs_backend=true
    elif [[ $frontend_accepted_sha == "$release_sha" && $backend_accepted_sha == "$release_sha" ]]; then
        # An explicit replay of an already accepted SHA still uses the complete release gates.
        needs_frontend=true
        needs_backend=true
    else
        frontend_changed_files=$(git diff --name-only "$frontend_accepted_sha" "$release_sha")
        while IFS= read -r changed; do
            case "$changed" in
                frontend-school/* | scripts/discover_school_tenants.sh | \
                    scripts/find_worker_release_candidates.mjs | \
                    scripts/lib/schoolorbit-installer/configure_pre_cutover_origin.sh)
                    needs_frontend=true
                    ;;
                .github/workflows/deploy-school-release.yml | \
                    scripts/resolve_school_release_scope.sh | \
                    scripts/resolve_school_release_replay.mjs)
                    needs_frontend=true
                    needs_backend=true
                    ;;
            esac
        done <<<"$frontend_changed_files"

        backend_changed_files=$(git diff --name-only "$backend_accepted_sha" "$release_sha")
        while IFS= read -r changed; do
            case "$changed" in
                backend-school/* | podman-compose.yml | nginx-configs/school-api.* | \
                    scripts/render_nginx_config.sh | scripts/smoke_test.sh | \
                    scripts/prune_runtime_images.sh | scripts/clamd_runtime_matches.sh | \
                    scripts/reconcile_r2_cors.sh | \
                    scripts/lib/schoolorbit-installer/remote/deployment_timing.sh)
                    needs_backend=true
                    ;;
                .github/workflows/deploy-school-release.yml | \
                    scripts/resolve_school_release_scope.sh | \
                    scripts/resolve_school_release_replay.mjs)
                    needs_frontend=true
                    needs_backend=true
                    ;;
            esac
        done <<<"$backend_changed_files"
    fi

    if [[ $needs_frontend == true && $needs_backend == true ]]; then
        scope=full
    elif [[ $needs_frontend == true ]]; then
        scope=frontend
    elif [[ $needs_backend == true ]]; then
        scope=backend
    else
        echo "No school release inputs changed since the last accepted push" >&2
        exit 65
    fi
fi

case "$scope" in
    frontend)
        needs_frontend=true
        needs_backend=false
        ;;
    backend)
        needs_frontend=false
        needs_backend=true
        ;;
    full)
        needs_frontend=true
        needs_backend=true
        ;;
    *)
        echo "release_scope must be auto, frontend, backend, or full" >&2
        exit 64
        ;;
esac

printf 'scope=%s\nneeds_frontend=%s\nneeds_backend=%s\nrelease_id=%s\n' \
    "$scope" "$needs_frontend" "$needs_backend" "$release_sha"
