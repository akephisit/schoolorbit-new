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
readonly TEST_EXTENSION_SQL='CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public; CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;'
readonly CONTAINER_NAME="schoolorbit-backend-school-test-$$-${RANDOM}"
cleanup_armed=false

cleanup() {
    local original_status=$?
    local cleanup_status=0
    local existing_container

    trap - EXIT INT TERM HUP

    if [[ $cleanup_armed == true ]]; then
        if ! existing_container="$(
            docker container ls --all \
                --filter "name=^/${CONTAINER_NAME}$" \
                --format '{{.Names}}'
        )"; then
            printf 'ERROR: failed to inspect disposable PostgreSQL container %s\n' \
                "$CONTAINER_NAME" >&2
            cleanup_status=1
        elif [[ $existing_container == "$CONTAINER_NAME" ]] &&
            ! docker rm --force "$CONTAINER_NAME" >/dev/null; then
            printf 'ERROR: failed to remove disposable PostgreSQL container %s\n' \
                "$CONTAINER_NAME" >&2
            cleanup_status=1
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

if ! command -v docker >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: Docker Desktop is required for backend-school database tests' >&2
    exit 127
fi

if [[ -n ${DOCKER_HOST-} ]]; then
    docker_endpoint=$DOCKER_HOST
elif ! docker_endpoint="$(
    docker context inspect --format '{{(index .Endpoints "docker").Host}}'
)"; then
    printf '%s\n' 'ERROR: unable to inspect the active Docker context' >&2
    exit 69
fi

case "$docker_endpoint" in
    unix://* | npipe://*) ;;
    *)
        printf 'ERROR: backend-school tests require a local Docker engine; got %s\n' \
            "$docker_endpoint" >&2
        exit 64
        ;;
esac

if ! docker info >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: the local Docker engine is not reachable' >&2
    exit 69
fi

cleanup_armed=true
if ! docker run --detach \
    --name "$CONTAINER_NAME" \
    --publish '127.0.0.1::5432' \
    --tmpfs '/var/lib/postgresql:rw,size=1g' \
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
    if docker exec "$CONTAINER_NAME" \
        pg_isready --quiet --host 127.0.0.1 \
        --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE" \
        >/dev/null 2>&1; then
        postgres_ready=true
        break
    fi

    if ! container_running="$(
        docker container inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null
    )" || [[ $container_running != true ]]; then
        docker logs --tail 50 "$CONTAINER_NAME" >&2 || true
        printf '%s\n' 'ERROR: disposable PostgreSQL exited before becoming ready' >&2
        exit 70
    fi
    sleep 0.25
done

if [[ $postgres_ready != true ]]; then
    docker logs --tail 50 "$CONTAINER_NAME" >&2 || true
    printf '%s\n' 'ERROR: disposable PostgreSQL did not become ready within 30 seconds' >&2
    exit 70
fi

if ! docker exec "$CONTAINER_NAME" \
    psql --no-psqlrc --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE" \
    --set ON_ERROR_STOP=1 --command "$TEST_EXTENSION_SQL" \
    >/dev/null; then
    printf '%s\n' 'ERROR: failed to provision PostgreSQL test extensions' >&2
    exit 70
fi

if ! port_binding="$(docker port "$CONTAINER_NAME" 5432/tcp)"; then
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

TEST_DATABASE_URL="$LOCAL_TEST_DATABASE_URL" cargo test --bin backend-school "$@"
