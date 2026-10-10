#!/usr/bin/env bash
set -euo pipefail
: "${TEST_DATABASE_URL:?Use an owned disposable direct-endpoint test database}"
compatibility_log="$(mktemp)"
trap 'rm -f "$compatibility_log"' EXIT
run_nonempty_tests() {
    "$@" 2>&1 | tee "$compatibility_log"
    if ! grep -Eq 'test result: ok\. [1-9][0-9]* passed;' "$compatibility_log"; then
        echo 'ERROR: Neon compatibility command selected no passing tests' >&2
        return 1
    fi
}
case "${NEON_COMPATIBILITY_SCOPE:-full}" in
    full)
        run_nonempty_tests cargo test -p school-staff personnel_simplification -- --test-threads=1
        run_nonempty_tests cargo test -p school-auth session_schema_tests -- --nocapture
        run_nonempty_tests cargo test -p school-file-platform schema_tests -- --nocapture
        run_nonempty_tests cargo test modules::academic::core::schema_tests::migration_060 --bin backend-school -- --nocapture --test-threads=4
        run_nonempty_tests cargo test modules::system::handlers::migration::tests::gradebook_results_status --bin backend-school -- --nocapture --test-threads=4
        ;;
    timetable-subject-groups)
        run_nonempty_tests cargo test modules::academic::core::schema_tests::migration_060_subject_group_projections_support_timetable_load --bin backend-school -- --exact --nocapture
        ;;
    course-zero-periods)
        run_nonempty_tests cargo test --test delivery_versions zero_course_periods_preserve_delivery_and_require_removing_existing_lessons -- --exact --nocapture
        ;;
    *)
        echo 'ERROR: unknown Neon compatibility selection' >&2
        exit 64
        ;;
esac
