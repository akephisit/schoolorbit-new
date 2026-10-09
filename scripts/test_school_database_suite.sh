#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
SCHOOLORBIT_TEST_CONTAINER="schoolorbit-db-suite-$$-$RANDOM"
SCHOOLORBIT_TEST_OWNER="${GITHUB_RUN_ID:-local}-$$-$RANDOM"
export SCHOOLORBIT_TEST_CONTAINER SCHOOLORBIT_TEST_OWNER
cleanup() {
    docker rm --force --volumes "$SCHOOLORBIT_TEST_CONTAINER" >/dev/null
}
trap cleanup EXIT
# One server, a new owned disposable database for each invocation. Tenant test helpers
# retain their schema isolation and migration locks inside each database.
docker run -d --name "$SCHOOLORBIT_TEST_CONTAINER" \
    --label "schoolorbit.test-owner=$SCHOOLORBIT_TEST_OWNER" \
    -p '127.0.0.1::5432' --shm-size 1g \
    -e POSTGRES_USER=schoolorbit_test -e POSTGRES_PASSWORD=schoolorbit_test \
    docker.io/library/postgres:18.4-alpine@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15 \
    postgres -c fsync=off -c synchronous_commit=off -c full_page_writes=off -c max_connections=200 >/dev/null
for attempt in {1..120}; do
    if docker exec "$SCHOOLORBIT_TEST_CONTAINER" pg_isready -h 127.0.0.1 -U schoolorbit_test -q; then break; fi
    if [[ $attempt == 120 ]]; then exit 70; fi
    sleep 0.25
done
./scripts/test_backend_school.sh curriculum_revision_schema_tests -- --test-threads=1
./scripts/test_backend_school.sh --package school-navigation -- --test-threads=4
./scripts/test_backend_school.sh modules::school::public_tests -- --test-threads=4
./scripts/test_backend_school.sh modules::academic::core::services_tests -- --test-threads=4
./scripts/test_backend_school.sh activation_context_tests -- --test-threads=4
./scripts/test_backend_school.sh promotion_targets::tests -- --test-threads=4
./scripts/test_backend_school.sh opening_readiness_tests -- --test-threads=4
./scripts/test_backend_school.sh promotion_execution_tests -- --test-threads=4
./scripts/test_backend_school.sh curriculum_preview_apply_is_hash_checked_and_closed_terms_reject_writes -- --test-threads=1
./scripts/test_backend_school.sh delivery_management_options_are_scoped_and_human_readable -- --test-threads=1
./scripts/test_backend_school.sh --integration delivery_versions -- --test-threads=4
./scripts/test_backend_school.sh --package school-certificates purge_rejects_admission_logo_and_question_bank_file_consumers -- --test-threads=1
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh canonical_seed_is_idempotent_across_student_year_and_placement -- --test-threads=1

# Security boundaries remain in the same compiled target and owned server.
./scripts/test_backend_school.sh --package school-auth --lib -- --test-threads=4
./scripts/test_backend_school.sh modules::auth::session_http_tests -- --test-threads=4
./scripts/test_backend_school.sh modules::auth::profile_integration_tests -- --test-threads=4
./scripts/test_backend_school.sh modules::auth::staff_integration_tests -- --test-threads=4
./scripts/test_backend_school.sh modules::academic::websockets::security_tests -- --test-threads=4

# Attendance facts, guardian fan-out, special-group uniqueness and verified archival.
ENCRYPTION_KEY=$(openssl rand -hex 32) ./scripts/test_backend_school.sh modules::attendance -- --test-threads=4
