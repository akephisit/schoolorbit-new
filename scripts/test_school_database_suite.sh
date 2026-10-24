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
    public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66 \
    postgres -c fsync=off -c synchronous_commit=off -c full_page_writes=off -c max_connections=200 >/dev/null
for attempt in {1..120}; do
    if docker exec "$SCHOOLORBIT_TEST_CONTAINER" pg_isready -h 127.0.0.1 -U schoolorbit_test -q; then break; fi
    if [[ $attempt == 120 ]]; then exit 70; fi
    sleep 0.25
done
./scripts/test_backend_school.sh curriculum_revision_schema_tests -- --test-threads=1
./scripts/test_backend_school.sh --package school-navigation -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::school::public_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::core::services_tests -- --test-threads=4
./scripts/test_backend_school.sh activation_context_tests -- --test-threads=4
./scripts/test_backend_school.sh promotion_targets::tests -- --test-threads=4
./scripts/test_backend_school.sh opening_readiness_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::lifecycle -- --test-threads=4
./scripts/test_backend_school.sh curriculum_preview_apply_is_hash_checked_and_closed_terms_reject_writes -- --test-threads=1
./scripts/test_backend_school.sh delivery_management_options_are_scoped_and_human_readable -- --test-threads=1
./scripts/test_backend_school.sh --integration delivery_versions -- --test-threads=4
./scripts/test_backend_school.sh --package school-certificates purge_rejects_admission_logo_and_question_bank_file_consumers -- --test-threads=1
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh canonical_seed_is_idempotent_across_student_year_and_placement -- --test-threads=1

# Security boundaries remain in the same compiled target and owned server.
./scripts/test_backend_school.sh --package school-auth --lib -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::auth::session_http_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::auth::profile_integration_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::auth::staff_integration_tests -- --test-threads=4
# WebSocket security tests now run in the school-academic-http unit owner.

# Attendance facts, guardian fan-out, special-group uniqueness and verified archival.
ENCRYPTION_KEY=$(openssl rand -hex 32) ./scripts/test_backend_school.sh integration_tests::attendance -- --test-threads=4

# Supply catalog versions, duplicates, proposal decisions and private file ownership.
./scripts/test_backend_school.sh integration_tests::supply_catalog -- --test-threads=4

# Behavior mutations, replay, scope isolation, reset and physical detail cleanup.
./scripts/test_backend_school.sh --package school-behavior -- --test-threads=4

# Compatibility regressions now run on the same owned PostgreSQL 18.6 server.
./scripts/test_backend_school.sh --package school-staff personnel_simplification -- --test-threads=1
./scripts/test_backend_school.sh --package school-file-platform schema_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::core::schema_tests::migration_060 -- --test-threads=4
./scripts/test_backend_school.sh system::handlers::migration::tests::gradebook_results_status -- --test-threads=4
./scripts/test_backend_school.sh --integration delivery_versions zero_course_periods_preserve_delivery_and_require_removing_existing_lessons -- --exact
./scripts/test_backend_school.sh --package school-authorization --lib -- --test-threads=4
./scripts/test_backend_school.sh --package school-tenancy --lib -- --test-threads=4

# Actual bound-session, managed admission and policy regressions require the owned server.
./scripts/test_backend_school.sh integration_tests::database -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::notification_dispatch -- --test-threads=4
./scripts/test_backend_school.sh services::cleaner::phase_tests -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::policies -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::services -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::gradebook -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::learner_evaluation -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::results -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::certificates -- --test-threads=4

# Domain transport moves retain every affected provider and cross-domain boundary fixture.
for package in school-test-db school-academic-core school-academic-delivery school-academic-timetable school-academic-assessment school-academic-results school-academic-lifecycle school-budget school-student-care school-certificates school-file-platform school-students school-staff school-calendar school-supervision school-admission school-question-bank school-fonts school-workflow school-administration school-files-http; do
    ./scripts/test_backend_school.sh --package "$package" -- --test-threads=4
done
for domain in students staff calendar supervision admission question_bank; do
    ./scripts/test_backend_school.sh "integration_tests::$domain" -- --test-threads=4
done
