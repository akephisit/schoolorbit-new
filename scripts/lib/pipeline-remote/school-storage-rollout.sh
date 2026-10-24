#!/usr/bin/env bash
# No server, provider I/O or tenant SQL here: the exact image owns those operations.
set -euo pipefail
mode=${1:?preflight or cutover is required}
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
root="$stack/deployment"
: "${RELEASE_SHA:?}"
: "${BACKEND_IMAGE_DIGEST:?}"
[[ $RELEASE_SHA =~ ^[0-9a-f]{40}$ && $BACKEND_IMAGE_DIGEST =~ ^sha256:[0-9a-f]{64}$ ]] || exit 64
image="ghcr.io/akephisit/schoolorbit-backend-school@$BACKEND_IMAGE_DIGEST"
journal="$root/releases/$RELEASE_SHA/storage-namespace"
backend=schoolorbit-backend-school
oneshot=schoolorbit-school-rollout
# shellcheck source=scripts/lib/pipeline-remote/runtime-env.sh
. "$root/scripts/lib/pipeline-remote/runtime-env.sh"
# shellcheck source=scripts/lib/pipeline-remote/storage-namespace-input.sh
. "$root/scripts/lib/pipeline-remote/storage-namespace-input.sh"
attestation=$(storage_namespace_input) || exit $?
umask 077
mkdir -p "$journal"
[[ ! -L $journal && $(stat -c %a "$journal") == 700 && $(stat -c %u "$journal") == "$(id -u)" ]] || exit 78
scratch=$(mktemp -d "$journal/.run.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
fail() {
    printf 'storage_rollout_error code=%s\n' "$1" >&2
    exit 1
}
private_file() {
    [[ -f $1 && ! -L $1 && $(stat -c %a "$1") == 600 && $(stat -c %u "$1") == "$(id -u)" ]]
}
sync_path() {
    # Publish only after both file data and containing directory metadata are durable.
    python3 - "$1" 2>"$scratch/sync-error" <<'PYFS' || fail journal_sync_failed
import os, sys
path = sys.argv[1]
for target in (path, os.path.dirname(path)):
    fd = os.open(target, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
PYFS
}
create_file() {
    chmod 600 "$1"
    sync_path "$1"
    if [[ -e $2 || -L $2 ]]; then
        if ! private_file "$2" || ! cmp -s "$1" "$2"; then
            fail journal_conflict
        fi
        sync_path "$2"
        return 0
    fi
    ln "$1" "$2" 2>/dev/null || fail journal_conflict
    sync_path "$2"
}
write_marker() {
    local marker
    marker=$(mktemp "$scratch/marker.XXXXXX")
    printf '%s\n' "$2" >"$marker"
    create_file "$marker" "$journal/$1"
}
report_aggregate() {
    [[ -s $1 ]] || return 0
    # Never promote unknown fields, multiline text or malformed partial output.
    jq -ces '
        if length == 1 and (.[0] | type == "object" and .schema_version == 1) then
            .[0] | with_entries(select(.key | IN("schema_version","tenants","migrated","failed","latest_version","checked","watches","unresolved","mismatched","applied","ready"))) |
            if all(to_entries[]; if .key == "ready" then (.value | type == "boolean") else (.value | type == "number" and . >= 0 and floor == .) end)
            then . else error("invalid") end
        else error("invalid") end
    ' "$1" 2>/dev/null || return 1
}
valid_routing() {
    jq -e 'def digest: type == "string" and test("^[0-9a-f]{64}$");
        type == "object" and .schema_version == 1 and
        (.public_namespace_sha256 | digest) and (.private_namespace_sha256 | digest)' "$1" >/dev/null 2>&1
}
report_interlock() {
    # Existing name ownership is enough to refuse; untrusted labels are diagnostics only.
    if podman inspect --format '{{json .Config.Labels}}' "$oneshot" >"$scratch/interlock-labels" 2>"$scratch/interlock-error"; then
        jq -ces --arg name "$oneshot" '
            if length == 1 and (.[0] | type == "object") then .[0] else {} end |
            {name:$name,
             release:(.["org.schoolorbit.rollout.release"] | if type == "string" and test("^[0-9a-f]{40}$") then . else "unknown" end),
             operation:(.["org.schoolorbit.rollout.operation"] | if IN("migrate-tenants","context-admin","routing-fingerprints","check-admin","attest-admin") then . else "unknown" end)}
        ' "$scratch/interlock-labels" >&2 2>/dev/null && return 0
    fi
    printf '{"name":"%s","release":"unknown","operation":"unknown"}\n' "$oneshot" >&2
}
run_image() {
    local output=$1 network=$2 operation
    shift 2
    case "$*" in
        migrate-tenants) operation=migrate-tenants ;;
        'storage-namespace context-admin') operation=context-admin ;;
        'storage-namespace routing-fingerprints') operation=routing-fingerprints ;;
        'storage-namespace check-admin') operation=check-admin ;;
        'storage-namespace attest-admin') operation=attest-admin ;;
        *) fail invalid_headless_operation ;;
    esac
    # Podman reserves the name atomically for the daemon-side lifetime, including
    # after SSH/controller loss. Never remove another operation to make progress.
    local -a options=(--rm --name "$oneshot"
        --label "org.schoolorbit.rollout.release=$RELEASE_SHA"
        --label "org.schoolorbit.rollout.image=$BACKEND_IMAGE_DIGEST"
        --label "org.schoolorbit.rollout.operation=$operation"
        --network "$network" --read-only --cap-drop=all --security-opt=no-new-privileges
        --userns=keep-id --user "$(id -u):$(id -g)"
        --env R2_ACCOUNT_ID --env R2_PUBLIC_BUCKET_NAME --env R2_PRIVATE_BUCKET_NAME)
    if [[ $network != none ]]; then
        [[ -f $storage_postgres_ca_file && -r $storage_postgres_ca_file ]] || fail postgres_ca_missing
        options+=(--env BACKEND_ADMIN_URL --env INTERNAL_API_SECRET
            --volume "$storage_postgres_ca_file:/run/secrets/postgresql-ca.crt:ro")
        for key in BACKEND_ADMIN_REQUEST_TIMEOUT_MS BACKEND_ADMIN_RETRY_MAX_ATTEMPTS BACKEND_ADMIN_RETRY_BASE_DELAY_MS; do
            [[ -z ${!key:-} ]] || options+=(--env "$key")
        done
    fi
    if [[ ${1:-} == storage-namespace && ${2:-} == attest-admin ]]; then
        mkdir -p "$journal/manifests"
        [[ ! -L $journal/manifests && $(stat -c %a "$journal/manifests") == 700 ]] || fail manifest_directory_invalid
        export FILE_STORAGE_NAMESPACE_ATTESTATION_JSON="$attestation"
        export FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY=/run/storage-namespace-manifests
        sync_path "$journal/manifests"
        options+=(--env FILE_STORAGE_NAMESPACE_ATTESTATION_JSON --env FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY
            --volume "$journal/manifests:/run/storage-namespace-manifests:rw")
    fi
    if ! podman run "${options[@]}" "$image" /app/backend-school "$@" >"$output" 2>"$scratch/command-error"; then
        report_aggregate "$output" || true
        if podman container exists "$oneshot"; then
            report_interlock
            fail headless_operation_exists
        fi
        # BindingError is a static allowlist; never print arbitrary process stderr.
        if grep -Eq '^(storage_binding_manifest_invalid|storage_watch_schema_missing|storage_binding_database_failed|storage_binding_namespace_invalid|storage_binding_owner_required|storage_binding_watch_changed|storage_binding_active_claim)$' "$scratch/command-error"; then
            grep -E '^(storage_binding_manifest_invalid|storage_watch_schema_missing|storage_binding_database_failed|storage_binding_namespace_invalid|storage_binding_owner_required|storage_binding_watch_changed|storage_binding_active_claim)$' "$scratch/command-error" >&2
        fi
        fail headless_command_failed
    fi
    jq -e 'type == "object" and .schema_version == 1' "$output" >/dev/null 2>&1 || fail invalid_command_output
    case "$*" in
        migrate-tenants | 'storage-namespace check-admin' | 'storage-namespace attest-admin')
            report_aggregate "$output" || fail invalid_command_output
            ;;
    esac
}
configured() (
    local key value storage_postgres_ca_file="$stack/postgresql/ca.crt"
    if grep -q '^POSTGRES_CA_FILE=' "$stack/.env"; then
        value=$(schoolorbit_runtime_env_value POSTGRES_CA_FILE "$stack/.env") || fail runtime_configuration_invalid
        if [[ -n $value ]]; then
            storage_postgres_ca_file=$value
            [[ $storage_postgres_ca_file == /* ]] || storage_postgres_ca_file="$stack/$storage_postgres_ca_file"
        fi
    fi
    for key in BACKEND_ADMIN_URL INTERNAL_API_SECRET R2_ACCOUNT_ID R2_PUBLIC_BUCKET_NAME R2_PRIVATE_BUCKET_NAME; do
        value=$(schoolorbit_runtime_env_value "$key" "$stack/.env") || fail runtime_configuration_invalid
        export "$key=$value"
    done
    for key in BACKEND_ADMIN_REQUEST_TIMEOUT_MS BACKEND_ADMIN_RETRY_MAX_ATTEMPTS BACKEND_ADMIN_RETRY_BASE_DELAY_MS; do
        if grep -q "^${key}=" "$stack/.env"; then
            value=$(schoolorbit_runtime_env_value "$key" "$stack/.env") || fail runtime_configuration_invalid
            export "$key=$value"
        fi
    done
    run_image "$@"
)
old_routing() (
    local key value
    # Only these three noncredential values leave inspect's private pipe.
    podman inspect --format '{{json .Config.Env}}' "$backend" 2>"$scratch/inspect-error" |
        jq -e 'map(select(startswith("R2_ACCOUNT_ID=") or startswith("R2_PUBLIC_BUCKET_NAME=") or startswith("R2_PRIVATE_BUCKET_NAME=")))' >"$scratch/old-routing-env" || fail old_routing_unavailable
    for key in R2_ACCOUNT_ID R2_PUBLIC_BUCKET_NAME R2_PRIVATE_BUCKET_NAME; do
        value=$(jq -er --arg key "$key=" 'map(select(startswith($key))) | if length == 1 then .[0] | ltrimstr($key) else error("invalid") end' "$scratch/old-routing-env" 2>/dev/null) || fail old_routing_unavailable
        export "$key=$value"
    done
    run_image "$scratch/old-routing" none storage-namespace routing-fingerprints
)
validate_journal() {
    private_file "$journal/old-context.json" || fail old_context_missing
    jq -e --arg release "$RELEASE_SHA" --arg digest "$BACKEND_IMAGE_DIGEST" '
        .schema_version == 1 and .release_sha == $release and .prepared_image_digest == $digest
        and ((.old_container_id == null and .old_image == null and .routing == null)
          or ((.old_container_id | test("^[0-9a-f]{64}$")) and (.old_image | test("^sha256:[0-9a-f]{64}$"))
            and (.routing.schema_version == 1)
            and (.routing.public_namespace_sha256 | test("^[0-9a-f]{64}$"))
            and (.routing.private_namespace_sha256 | test("^[0-9a-f]{64}$"))
            and (.old_config_sha256 | test("^[0-9a-f]{64}$"))))
    ' "$journal/old-context.json" >/dev/null 2>&1 || fail old_context_mismatch
    if jq -e '.old_container_id != null' "$journal/old-context.json" >/dev/null; then
        private_file "$journal/old-routing-config.json" || fail old_configuration_missing
        actual_config_sha=$(sha256sum "$journal/old-routing-config.json")
        actual_config_sha=${actual_config_sha%% *}
        [[ $actual_config_sha == "$(jq -r '.old_config_sha256' "$journal/old-context.json")" ]] || fail old_configuration_changed
    fi
}
verify_expected() {
    [[ -n $attestation ]] || return 0
    jq -e --argjson evidence "$attestation" --slurpfile old "$journal/old-context.json" '
        .inventory_sha256 == $evidence.expected_inventory_sha256 and
        .public_namespace_sha256 == $evidence.expected_public_namespace_sha256 and
        .private_namespace_sha256 == $evidence.expected_private_namespace_sha256 and
        $old[0].routing.public_namespace_sha256 == $evidence.expected_public_namespace_sha256 and
        $old[0].routing.private_namespace_sha256 == $evidence.expected_private_namespace_sha256
    ' "$scratch/context" >/dev/null 2>&1 || fail attestation_context_mismatch
}
sync_path "$journal"
if [[ $mode == preflight ]]; then
    podman image exists "$image" || fail prepared_image_unavailable
    configured "$scratch/context" schoolorbit-web storage-namespace context-admin
    valid_routing "$scratch/context" || fail invalid_context
    jq -ces 'def digest: type == "string" and test("^[0-9a-f]{64}$");
        if length == 1 and (.[0] | keys == ["inventory_sha256","private_namespace_sha256","public_namespace_sha256","schema_version","tenants"] and
          .schema_version == 1 and (.tenants | type == "number" and . >= 0 and floor == .) and
          (.inventory_sha256 | digest) and (.public_namespace_sha256 | digest) and (.private_namespace_sha256 | digest))
        then .[0] else error("invalid") end' "$scratch/context" 2>/dev/null || fail invalid_context
    if [[ ! -e $journal/old-context.json ]]; then
        if podman container exists "$backend"; then
            old_id=$(podman inspect --format '{{.Id}}' "$backend") || fail old_identity_unavailable
            old_image=$(podman inspect --format '{{.Image}}' "$backend") || fail old_identity_unavailable
            old_image=${old_image#sha256:}
            [[ $old_id =~ ^[0-9a-f]{64}$ && $old_image =~ ^[0-9a-f]{64}$ ]] || fail old_identity_invalid
            old_image="sha256:$old_image"
            old_routing
            valid_routing "$scratch/old-routing" || fail old_routing_invalid
            # Preserve only the three routing values, never credential/session configuration.
            cp "$scratch/old-routing-env" "$scratch/old-config"
            create_file "$scratch/old-config" "$journal/old-routing-config.json"
            config_sha=$(sha256sum "$journal/old-routing-config.json")
            config_sha=${config_sha%% *}
            jq -n --arg release "$RELEASE_SHA" --arg digest "$BACKEND_IMAGE_DIGEST" --arg id "$old_id" --arg image "$old_image" --arg config_sha "$config_sha" --slurpfile routing "$scratch/old-routing" \
                '{schema_version:1,release_sha:$release,prepared_image_digest:$digest,old_container_id:$id,old_image:$image,old_config_sha256:$config_sha,routing:$routing[0]}' >"$scratch/old-context"
        else
            [[ ! -e $journal/migration-attempted ]] || fail old_context_missing
            jq -n --arg release "$RELEASE_SHA" --arg digest "$BACKEND_IMAGE_DIGEST" \
                '{schema_version:1,release_sha:$release,prepared_image_digest:$digest,old_container_id:null,old_image:null,old_config_sha256:null,routing:null}' >"$scratch/old-context"
        fi
        create_file "$scratch/old-context" "$journal/old-context.json"
    fi
    validate_journal
    verify_expected
    if jq -e '.routing != null' "$journal/old-context.json" >/dev/null; then
        jq -c '.routing | {schema_version,public_namespace_sha256,private_namespace_sha256}' "$journal/old-context.json"
    fi
    # Capture current container identity separately; the historical proof is immutable.
    current=absent
    if podman container exists "$backend"; then
        current=$(podman inspect --format '{{.Id}}' "$backend")
        [[ $current =~ ^[0-9a-f]{64}$ ]] || fail old_identity_invalid
        original=$(jq -r '.old_container_id // "absent"' "$journal/old-context.json")
        [[ $current == "$original" || -e $journal/migration-attempted ]] || fail old_identity_changed
    fi
    if [[ -e $journal/drain-container || -L $journal/drain-container ]]; then
        private_file "$journal/drain-container" || fail drain_context_invalid
    fi
    printf '%s\n' "$current" >"$scratch/current-drain"
    chmod 600 "$scratch/current-drain"
    sync_path "$scratch/current-drain"
    mv "$scratch/current-drain" "$journal/drain-container"
    sync_path "$journal/drain-container"
    echo 'storage_rollout stage=preflight_verified'
elif [[ $mode == cutover ]]; then
    validate_journal
    private_file "$journal/drain-container" || fail drain_context_missing
    # Maintenance is entered/verified by the caller immediately before this command.
    [[ -f $root/pending-release && $(cat "$root/pending-release") == "$RELEASE_SHA" ]] || fail maintenance_required
    expected=$(cat "$journal/drain-container")
    if podman container exists "$backend"; then
        current=$(podman inspect --format '{{.Id}}' "$backend")
        [[ $current == "$expected" ]] || fail drain_identity_changed
        podman stop "$backend" >"$scratch/stop" 2>"$scratch/stop-error" || fail old_writer_stop_failed
        [[ $(podman inspect --format '{{.State.Running}}' "$backend") == false ]] || fail old_writer_still_running
        write_marker "stopped-$current" "$current"
        podman rm "$backend" >"$scratch/remove" 2>"$scratch/remove-error" || fail old_writer_remove_failed
        podman container exists "$backend" && fail old_writer_still_exists
    elif [[ $expected != absent ]]; then
        if ! private_file "$journal/stopped-$expected" || [[ $(cat "$journal/stopped-$expected") != "$expected" ]]; then
            fail old_writer_disappeared
        fi
    fi
    write_marker migration-attempted "$RELEASE_SHA"
    configured "$scratch/migration" schoolorbit-web migrate-tenants
    echo 'storage_rollout stage=canonical_migrations_complete'
    if [[ -n $attestation ]]; then
        configured "$scratch/context" schoolorbit-web storage-namespace context-admin
        verify_expected
        configured "$scratch/attestation" schoolorbit-web storage-namespace attest-admin
        echo 'storage_rollout stage=explicit_attestation_complete'
    fi
    configured "$scratch/check" schoolorbit-web storage-namespace check-admin
    echo 'storage_rollout stage=namespace_readiness_complete'
else
    exit 64
fi
