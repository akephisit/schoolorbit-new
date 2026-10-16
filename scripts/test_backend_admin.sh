#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
container="schoolorbit-admin-test-$$-$RANDOM"
cleanup() { docker rm -f -v "$container" >/dev/null; }
trap cleanup EXIT
docker run -d --name "$container" -p '127.0.0.1::5432' \
    -e POSTGRES_USER=schoolorbit_test -e POSTGRES_PASSWORD=schoolorbit_test -e POSTGRES_DB=schoolorbit_test \
    public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66 >/dev/null
for attempt in {1..120}; do
    if docker exec "$container" pg_isready -h 127.0.0.1 -U schoolorbit_test -q; then break; fi
    [[ $attempt != 120 ]] || exit 70
    sleep 0.25
done
# Compile-time SQL metadata and tests use only an owned local disposable database.
# The utility binary is compiled, never executed.
for migration in backend-admin/migrations/*.sql; do
    docker exec -i "$container" psql -X --single-transaction -v ON_ERROR_STOP=1 -U schoolorbit_test -d schoolorbit_test <"$migration" >/dev/null
done
binding=$(docker port "$container" 5432/tcp)
[[ $binding =~ ^127\.0\.0\.1:[0-9]+$ ]] || exit 70
export DATABASE_URL="postgresql://schoolorbit_test:schoolorbit_test@${binding}/schoolorbit_test?sslmode=disable"
export TEST_DATABASE_URL="$DATABASE_URL"
cd backend-admin
cargo check --all-targets --locked
cargo test --locked
