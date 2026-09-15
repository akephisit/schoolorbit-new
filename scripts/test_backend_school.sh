#!/usr/bin/env bash
set -uo pipefail

if ! SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"; then
    printf '%s\n' 'ERROR: unable to resolve the test runner directory' >&2
    exit 70
fi
readonly SCRIPT_DIR
if ! REPOSITORY_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"; then
    printf '%s\n' 'ERROR: unable to resolve the repository root' >&2
    exit 70
fi
readonly REPOSITORY_ROOT
readonly BACKEND_DIR="$REPOSITORY_ROOT/backend-school"
readonly POSTGRES_IMAGE='docker.io/library/postgres:18.4-alpine@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15'
readonly POSTGRES_USER='schoolorbit_test'
readonly POSTGRES_PASSWORD='schoolorbit_test'
readonly POSTGRES_DATABASE='schoolorbit_test'
readonly POSTGRES_SHM_SIZE='1g'
readonly TEST_EXTENSION_SQL='CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public; CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;'
readonly CONTAINER_NAME="schoolorbit-backend-school-test-$$-${RANDOM}"
readonly TEST_BINARY="${BACKEND_SCHOOL_TEST_BIN:-backend-school}"
cleanup_armed=false
cargo_output=''

cleanup() {
    local original_status=$?
    local cleanup_status=0
    local exists_status

    trap - EXIT INT TERM HUP

    if [[ -n $cargo_output ]]; then
        rm -f -- "$cargo_output"
    fi

    if [[ $cleanup_armed == true ]]; then
        if podman container exists "$CONTAINER_NAME"; then
            if ! podman rm --force --volumes "$CONTAINER_NAME" >/dev/null; then
                printf 'ERROR: failed to remove disposable PostgreSQL container %s\n' \
                    "$CONTAINER_NAME" >&2
                cleanup_status=1
            fi
        else
            exists_status=$?
            if ((exists_status != 1)); then
                printf 'ERROR: failed to inspect disposable PostgreSQL container %s\n' \
                    "$CONTAINER_NAME" >&2
                cleanup_status=1
            fi
        fi
    fi

    if ((original_status != 0)); then
        exit "$original_status"
    fi
    exit "$cleanup_status"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

target_arguments=(--bin "$TEST_BINARY")
if [[ ${1-} == --package ]]; then
    if (($# < 2)); then
        printf '%s\n' 'ERROR: --package requires an internal workspace package name' >&2
        exit 64
    fi
    package_name=$2
    shift 2
    if [[ ! $package_name =~ ^[a-z0-9][a-z0-9-]*$ ]] ||
        [[ ! -f $BACKEND_DIR/crates/$package_name/Cargo.toml ]] ||
        ! grep -Fqx "name = \"$package_name\"" "$BACKEND_DIR/crates/$package_name/Cargo.toml"; then
        printf 'ERROR: unknown internal workspace package: %s\n' "$package_name" >&2
        exit 64
    fi
    target_arguments=(-p "$package_name")
fi

focused_filter=''
argument_takes_value=false
for argument in "$@"; do
    if [[ $argument == -- ]]; then
        break
    fi
    if [[ $argument_takes_value == true ]]; then
        argument_takes_value=false
        continue
    fi
    case "$argument" in
        --bench | --bin | --color | --example | --exclude | --features | --jobs | -j | \
            --manifest-path | --message-format | --package | -p | --profile | --target | \
            --target-dir | --test)
            argument_takes_value=true
            ;;
        -*) ;;
        *)
            focused_filter=$argument
            break
            ;;
    esac
done

if ! command -v podman >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: Podman is required for backend-school database tests' >&2
    exit 127
fi

if [[ -n ${CONTAINER_HOST-} || -n ${CONTAINER_CONNECTION-} ]]; then
    printf '%s\n' 'ERROR: backend-school tests require a local Podman engine' >&2
    exit 64
fi

if ! podman_rootless="$(podman info --format '{{.Host.Security.Rootless}}' 2>/dev/null)"; then
    printf '%s\n' 'ERROR: the local Podman engine is not reachable' >&2
    exit 69
fi

if [[ $podman_rootless != true ]]; then
    printf '%s\n' 'ERROR: backend-school tests require rootless Podman' >&2
    exit 64
fi

cleanup_armed=true
if ! podman run --detach \
    --name "$CONTAINER_NAME" \
    --publish '127.0.0.1::5432' \
    --mount type=volume,destination=/var/lib/postgresql \
    --shm-size "$POSTGRES_SHM_SIZE" \
    --env "POSTGRES_USER=$POSTGRES_USER" \
    --env "POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
    --env "POSTGRES_DB=$POSTGRES_DATABASE" \
    "$POSTGRES_IMAGE" \
    postgres \
    -c fsync=off \
    -c synchronous_commit=off \
    -c full_page_writes=off \
    -c max_connections=200 \
    >/dev/null; then
    printf '%s\n' 'ERROR: failed to start disposable PostgreSQL' >&2
    exit 70
fi

postgres_ready=false
for _attempt in {1..120}; do
    if podman exec "$CONTAINER_NAME" \
        pg_isready --quiet --host 127.0.0.1 \
        --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE" \
        >/dev/null 2>&1; then
        postgres_ready=true
        break
    fi

    if ! container_running="$(
        podman container inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null
    )" || [[ $container_running != true ]]; then
        podman logs --tail 50 "$CONTAINER_NAME" >&2 || true
        printf '%s\n' 'ERROR: disposable PostgreSQL exited before becoming ready' >&2
        exit 70
    fi
    sleep 0.25
done

if [[ $postgres_ready != true ]]; then
    podman logs --tail 50 "$CONTAINER_NAME" >&2 || true
    printf '%s\n' 'ERROR: disposable PostgreSQL did not become ready within 30 seconds' >&2
    exit 70
fi

if ! podman exec "$CONTAINER_NAME" \
    psql --no-psqlrc --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE" \
    --set ON_ERROR_STOP=1 --command "$TEST_EXTENSION_SQL" \
    >/dev/null; then
    printf '%s\n' 'ERROR: failed to provision PostgreSQL test extensions' >&2
    exit 70
fi

if ! port_binding="$(podman port "$CONTAINER_NAME" 5432/tcp)"; then
    printf '%s\n' 'ERROR: unable to resolve the disposable PostgreSQL port' >&2
    exit 70
fi
postgres_port="${port_binding##*:}"
if [[ ! $port_binding =~ ^127\.0\.0\.1:[0-9]+$ || ! $postgres_port =~ ^[0-9]+$ ]]; then
    printf 'ERROR: unexpected PostgreSQL port binding: %s\n' "$port_binding" >&2
    exit 70
fi

readonly LOCAL_TEST_DATABASE_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${postgres_port}/${POSTGRES_DATABASE}?sslmode=disable"
printf 'Running backend-school tests with disposable local PostgreSQL (%s)\n' \
    "$CONTAINER_NAME"

if ! cd "$BACKEND_DIR"; then
    printf 'ERROR: backend-school directory is unavailable: %s\n' "$BACKEND_DIR" >&2
    exit 70
fi

if ! cargo_output="$(mktemp "${TMPDIR:-/tmp}/schoolorbit-backend-tests.XXXXXX")"; then
    printf '%s\n' 'ERROR: unable to create focused-test result capture' >&2
    exit 70
fi

TEST_DATABASE_URL="$LOCAL_TEST_DATABASE_URL" CARGO_TERM_COLOR=never \
    cargo test "${target_arguments[@]}" "$@" 2>&1 | tee "$cargo_output"
cargo_status=${PIPESTATUS[0]}
if ((cargo_status != 0)); then
    exit "$cargo_status"
fi

if [[ -n $focused_filter ]] &&
    ! grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$cargo_output"; then
    printf 'ERROR: focused filter matched zero tests: %s\n' "$focused_filter" >&2
    exit 65
fi
