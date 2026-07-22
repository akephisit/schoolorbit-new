# Testing

This project uses a sandbox tenant for production-like smoke tests without touching real school data.

## Before Commit

Always run a diff sanity check from the repository root:

```bash
git diff --check
git status --short
```

Use focused tests for the code you changed. Do not run broad formatting or cleanup commands unless the task is specifically about formatting.

## Backend Checks

For `backend-school` changes:

```bash
cd backend-school
cargo check
```

For backend architecture guard changes (module roots, service-layer handler boundaries,
permission guards, tenant resolver rules, internal auth rules):

```bash
cd backend-school
cargo test --test static_architecture
```

For encryption or `national_id` / admission PII changes, also run:

```bash
cd backend-school
cargo test utils::field_encryption::tests --bin backend-school
cargo test modules::admission::services::pii::tests --bin backend-school
```

These tests require no real secrets; they set test-only `ENCRYPTION_KEY` and `BLIND_INDEX_KEY` internally.

For `backend-admin` changes:

```bash
cd backend-admin
cargo check
```

Existing backend warnings are tracked separately. Do not run `cargo fix` as part of unrelated work.

## Generated API contracts

Rust request/response DTOs and OpenAPI handler metadata are the source of truth.
`contracts/openapi/school-api.json` and files under
`frontend-school/src/lib/api/generated/` are generated files; do not edit them
directly.

After changing a documented DTO or endpoint:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Commit Rust source, OpenAPI, generated TypeScript, and focused tests together.
Frontend API modules import generated wire DTOs and may map them to separate
domain/view models. Generation must not require database credentials or start
the backend server.

The generated document currently contains 177 unique operations: 32
auth/authorization operations, 36 read-oriented JSON operations from the prior
checkpoint, the completed Phase 1 people batch (12 mutations plus the dependent
achievement list read), and the first Phase 2 academic structure batch (15
mutations plus four dependent reads), the Phase 2 curriculum core batch (15
mutations plus nine dependent reads), the Phase 2 activity-template batch
(10 mutations plus three dependent reads), the Phase 2 activity workspace
batch (21 mutations plus seven dependent reads), and the Phase 2 course
planning batch (seven mutations plus five dependent reads). The people
operations are:

- staff: `createStaff`, `updateStaff`, `deleteStaff`
- student/parent-link: `updateStudentProfile`, `createStudent`, `updateStudent`,
  `deleteStudent`, `addStudentParent`, `removeStudentParent`
- achievement: `listAchievements`, `createAchievement`, `updateAchievement`,
  `deleteAchievement`

The academic structure operations are `getAcademicStructure`,
`createGradeLevel`, `deleteGradeLevel`, `createAcademicYear`,
`updateAcademicYear`, `setActiveAcademicYear`, `getAcademicYearLevels`,
`updateAcademicYearLevels`, `createSemester`, `updateSemester`,
`deleteSemester`, `listClassrooms`, `createClassroom`, `updateClassroom`,
`enrollStudents`, `listClassEnrollments`, `removeEnrollment`,
`updateEnrollmentNumber`, and `autoAssignClassNumbers`.

The curriculum core operations are `listSubjectGroups`,
`batchListSubjectDefaultInstructors`, `listSubjects`, `createSubject`,
`updateSubject`, `deleteSubject`, `listSubjectDefaultInstructors`,
`addSubjectDefaultInstructor`, `removeSubjectDefaultInstructor`,
`updateSubjectDefaultInstructorRole`, `listStudyPlans`, `createStudyPlan`,
`getStudyPlan`, `updateStudyPlan`, `deleteStudyPlan`, `listStudyPlanVersions`,
`createStudyPlanVersion`, `getStudyPlanVersion`, `updateStudyPlanVersion`,
`deleteStudyPlanVersion`, `listStudyPlanSubjects`,
`addSubjectsToStudyPlanVersion`, `deleteStudyPlanSubject`, and
`generateCoursesFromStudyPlan`.

The activity-template operations are `listStudyPlanActivities`,
`addStudyPlanActivity`, `updateStudyPlanActivity`, `deleteStudyPlanActivity`,
`generateActivitiesFromStudyPlan`, `listActivityCatalog`,
`createActivityCatalog`, `updateActivityCatalog`, `deleteActivityCatalog`,
`listActivityCatalogDefaultInstructors`,
`addActivityCatalogDefaultInstructor`,
`removeActivityCatalogDefaultInstructor`, and
`updateActivityCatalogDefaultInstructorRole`.

The activity workspace operations are `listActivitySlots`, `updateActivitySlot`,
`deleteActivitySlot`, `listActivitySlotInstructors`, `addActivitySlotInstructor`,
`addActivitySlotInstructorsBatch`, `removeActivitySlotInstructor`,
`removeAllActivitySlotInstructors`, `deleteAllActivitySlotGroups`,
`deleteActivitySlotTimetableEntries`, `listActivitySlotClassroomAssignments`,
`upsertActivitySlotClassroomAssignments`,
`deleteAllActivitySlotClassroomAssignments`,
`deleteActivitySlotClassroomAssignment`, `listActivityGroups`,
`createActivityGroup`, `updateActivityGroup`, `deleteActivityGroup`,
`listActivityGroupMembers`, `addActivityGroupMembers`,
`removeActivityGroupMember`, `updateActivityGroupMemberResult`,
`listActivityGroupInstructors`, `addActivityGroupInstructor`,
`removeActivityGroupInstructor`, `listMyActivityEnrollments`,
`selfEnrollActivityGroup`, and `selfUnenrollActivityGroup`.

The course planning operations are `listClassroomCourses`, `assignCourses`,
`updateClassroomCourse`, `removeClassroomCourse`,
`batchListCourseInstructors`, `batchListCourseInstructorsFromQuery`,
`listCourseInstructors`, `addCourseInstructor`,
`updateCourseInstructorRole`, `removeCourseInstructor`,
`listClassroomActivities`, and `removeClassroomFromActivitySlot`.

Activity workspace tests require frontend/backend permission scopes to match,
missing parents and mutation targets to return not-found, bounded role/result
values to be validated, bulk counts to report actual writes, and group capacity
checks plus enrollment writes to share a transaction.

Course planning reads use `ACADEMIC_COURSE_PLAN_READ_ALL`; mutations use
`ACADEMIC_COURSE_PLAN_MANAGE_ALL`. Missing parents and mutation targets return
not-found, instructor roles are limited to `primary`/`secondary`, and assignment
counts report actual inserts. In `UpdateCourseRequest`, an omitted
`primary_instructor_id` leaves the team unchanged while explicit `null` clears
the primary team assignment and derived timetable-entry instructors.

This is the current Phase 4 mutation-contract rollout checkpoint. The next
Phase 2 batch is scheduling configuration. The document tracks implemented
backend routes only; frontend-only helpers are not exported. SSE, WebSocket,
health/readiness, and file/binary endpoints remain explicitly outside this
OpenAPI contract.

Authorization regression tests require effective permissions to be empty when
`users.status != 'active'`. Student soft deletion invalidates that user's
permission cache and emits `permission_changed`. Missing student targets for
update, delete, and add-parent return not-found before dependent writes occur.

Academic structure authorization tests require each structure, classroom, and
enrollment handler to load `actor_tenant_context` and enforce its generated
backend permission. Database-backed
tests verify missing academic years, semesters, grade levels, classrooms, and
enrollments return not-found, and a missing active-year target does not
deactivate the current year.

Curriculum-core authorization tests require subject and study-plan handlers to
enforce the same generated permission policy used by the frontend. Study-plan
CRUD accepts the matching all-scope or organization-scope curriculum permission;
course generation requires `ACADEMIC_COURSE_PLAN_MANAGE_ALL`. Database-backed
tests verify missing subjects, instructor assignments, study plans, versions,
and plan-subject rows return not-found. They also verify that a missing
instructor assignment does not demote the current primary instructor, the path
version owns the plan-subject query scope, and add-subject counts report rows
actually inserted.

Activity-template authorization tests require every study-plan activity,
activity-catalog, and default-instructor handler to load `actor_tenant_context`
and enforce the matching curriculum policy. Activity generation additionally
requires `ACTIVITY_MANAGE_ALL` plus curriculum read access. Database-backed tests
verify missing study-plan versions, semesters, catalogs, grade levels,
instructors, plan-activity rows, and default-instructor assignments return
not-found, and that updating a missing assignment cannot demote the current
primary instructor.

`DELETE /api/roles/{id}` and `DELETE /api/organization/units/{id}` are implemented
as reversible deactivation (`is_active = false`), never physical deletion. They
preserve assignments, memberships, permission grants, delegations, and audit
history. `is_system` is a read-only migration/provisioning flag that protects the
`ADMIN` role and `SCHOOL` unit. Management lists can request
`include_inactive=true`; assignment lists remain active-only by default.

Database-backed lifecycle coverage can be run with:

```bash
cd backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test modules::staff::services::status_tests --bin backend-school
```

These tests cover idempotent deactivate/reactivate audit rows, active-child and
inactive-parent conflicts, inactive authorization sources, and rejection of new
assignments to inactive records.

The phase gate is:

```bash
cd backend-school
cargo test api_contract::tests -- --nocapture
cargo test --test static_architecture

cd ../frontend-school
npm run test:api-contracts
npm run check:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

## Permission Contract

`contracts/permissions.json` is the only handwritten registry for backend and frontend
permission definitions. After adding a permission or changing its display metadata, run:

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Commit `contracts/permissions.lock.json`,
`backend-school/src/permissions/registry_generated.rs`, and
`frontend-school/src/lib/permissions/registry.generated.ts` with the contract change.
Never edit either generated registry directly.

The lock permits additions and metadata updates but rejects removed or renamed permission
codes. Version 1 intentionally has no removal bypass: an intentional removal or rename
requires a separately reviewed contract-version and migration plan.
`npm run check:permissions` is read-only and fails when any generated artifact is stale.

## Migration Safety

Never edit a migration that may already have been applied, even for comments. `sqlx` stores migration checksums and tenant startup will fail if an applied migration file changes.

Correct workflow:

1. Add a new sequential migration file.
2. Put schema, index, data, or comment changes in the new migration.
3. Run `git diff --check`.
4. Let backend-school apply the new migration during tenant startup or run the migration flow explicitly in a controlled environment.

If a checksum mismatch appears, restore the original migration file content. Do not edit the database migration checksum to hide the mismatch.

## Sandbox Smoke Test

Run the smoke test from the repository root:

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='your-sandbox-password' \
./scripts/smoke_test.sh
```

Or copy the local env template and run the script without inline secrets:

```bash
cp .env.smoke.example .env.smoke.local
# edit .env.smoke.local and set SMOKE_PASSWORD
./scripts/smoke_test.sh
```

`scripts/smoke_test.sh` automatically loads `.env.smoke.local` by default. Override with
`SMOKE_ENV_FILE=/path/to/file` when needed.

The script checks:

- tenant frontend page loads
- backend-admin `/health`
- backend-school `/health`
- CORS from the tenant origin
- unauthenticated `/api/auth/me` returns `401`
- login preflight returns `204`
- login returns an `auth_token` cookie
- authenticated `/api/auth/me` returns the logged-in user

Production tenant browser requests use `Origin` for tenant resolution by default. Smoke tenant API requests still send both `Origin` and `X-School-Subdomain`; the preflight check includes `x-school-subdomain` so CORS config drift for explicit override clients is caught before browser E2E.

If `SMOKE_USERNAME` or `SMOKE_PASSWORD` is omitted, authenticated login checks are skipped and the script only validates public endpoints, CORS, and login request validation.

## Clean Migration Baseline

Active tenant migrations are intentionally clean-cut to a single baseline:
`backend-school/migrations/001_baseline.sql`. Historical migrations live in
`backend-school/migrations_legacy/` for audit/reference and are not used by the
runtime migrator.

Check the active migration directory and a clean tenant database with:

```bash
./scripts/check_migration_rebaseline_ready.sh

MIGRATION_AUDIT_DATABASE_URL='postgresql://...' \
  ./scripts/check_migration_rebaseline_ready.sh
```

The database check is read-only. It verifies the tenant has applied the current
active migration count/version, has no failed migration rows, and has no legacy
permission codes from the pre-clean permission contract.

Do not point this clean migration set at an existing tenant database that still
has old `_sqlx_migrations` rows from the legacy 1-127 timeline. For production
cutover, provision a clean database, apply `001_baseline.sql`, copy required
tenant data, validate, then switch the tenant database URL.

Prepare a brand-new clean tenant database with the guarded script:

```bash
PREPARE_CLEAN_TENANT_DATABASE_URL='postgresql://.../schoolorbit_snwsb_v2?...' \
PREPARE_CLEAN_TENANT_CONFIRM=public \
PREPARE_CLEAN_TENANT_ALLOW_NON_TEST=1 \
  ./scripts/prepare_clean_tenant_db.sh
```

The script refuses legacy `_sqlx_migrations` histories and non-empty schemas
without clean migration history. The target should validate with users `0`,
permissions `81`, organization units `30`, and exactly one successful SQLx
migration row at version `1`.

To verify the same path without touching a real database `public` schema, use a
temporary schema in `schoolorbit_test`:

```bash
schema="schoolorbit_prepare_$(date +%s)_$$"
PREPARE_CLEAN_TENANT_DATABASE_URL='postgresql://.../schoolorbit_test?...' \
PREPARE_CLEAN_TENANT_SCHEMA="$schema" \
PREPARE_CLEAN_TENANT_CONFIRM="$schema" \
PREPARE_CLEAN_TENANT_RESET_SCHEMA=1 \
PREPARE_CLEAN_TENANT_DROP_SCHEMA_ON_EXIT=1 \
  ./scripts/prepare_clean_tenant_db.sh
```

## Tenant Data Cutover Dry Run

Before pointing an existing school at a clean-baseline database, run a data-only
cutover dry run against `schoolorbit_test`. The script creates a temporary target
schema, applies `001_baseline.sql`, truncates target application tables, copies
source tenant data excluding `_sqlx_migrations`, runs permission sync, compares
row counts across every application table, and drops the temporary schema unless
`CUTOVER_KEEP_SCHEMA=1` is set.

```bash
CUTOVER_SOURCE_DATABASE_URL='postgresql://.../schoolorbit_snwsb?...' \
CUTOVER_TARGET_DATABASE_URL='postgresql://.../schoolorbit_test?...' \
  ./scripts/cutover_tenant_data.sh
```

For a real cutover, provision a new clean database first and apply the baseline.
Only then run with `CUTOVER_MODE=apply`, explicit `CUTOVER_TARGET_SCHEMA`, and
the required confirmation variables. Never copy source `_sqlx_migrations`; the
target must keep exactly one successful migration row at version `1`.

## Environment Variables

Optional overrides:

```bash
SMOKE_SUBDOMAIN=sandbox
SMOKE_TENANT_URL=https://sandbox.schoolorbit.app
SMOKE_ORIGIN=https://sandbox.schoolorbit.app
SMOKE_API_URL=https://school-api.schoolorbit.app
SMOKE_ADMIN_API_URL=https://admin-api.schoolorbit.app
SMOKE_TIMEOUT_SECONDS=20
SMOKE_REMEMBER_ME=true
```

Do not commit sandbox passwords or production credentials. Pass them as environment variables only.

## GitHub Actions

The `Smoke Test Sandbox` workflow can be run manually from GitHub Actions. It uses the same `scripts/smoke_test.sh` script and defaults to `sandbox.schoolorbit.app`.

For authenticated checks, configure repository secrets:

```bash
SMOKE_USERNAME
SMOKE_PASSWORD
```

Run it from Actions with `run_authenticated=true` to test login and authenticated `/api/auth/me`. Use `run_authenticated=false` for public endpoint and CORS checks only.

## Browser E2E

### Tenant-authenticated timetable realtime

1. Deploy backend first; existing school JWTs are intentionally rejected and users log in once.
2. Verify `GET /api/auth/me` succeeds on the login tenant and returns `401` when the same cookie is sent with another tenant header/origin.
3. Set `E2E_SEMESTER_ID` to a sandbox semester UUID. Verify `/ws/timetable?semester_id=$E2E_SEMESTER_ID` returns `401` without the HttpOnly cookie, `403` for an authenticated user without read/manage permission, and `404` when the value is changed to a valid UUID that is absent from that tenant.
4. Open two authorized tabs and verify one joined presence, no leave event until the final tab closes, cursor collaboration for readers, and edit collaboration only for managers.
5. Disable the network, restore it, and verify one reconnect attempt resumes with no reconnect loop after page teardown.
6. Confirm logs contain no JWT, password, national ID, database URL, malformed WebSocket payload, or legacy query identity (`user_id`, `name`, `school_key`).

The `frontend-school` app has a minimal Playwright test for the live sandbox login flow.

Install the browser once on a local machine:

```bash
cd frontend-school
npx playwright install chromium
```

On Ubuntu 26.04, Playwright may need the Ubuntu 24.04 browser fallback until official 26.04 browser builds are available:

```bash
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install chromium
```

If Chromium launches with missing shared libraries such as `libnspr4.so`, install the native dependencies:

```bash
sudo apt install -y libnspr4 libnss3 libasound2t64 libxss1 fonts-liberation
```

Then verify the installed Playwright browser has no missing shared libraries:

```bash
ldd ~/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell | grep "not found" || echo "deps ok"
```

Run the test:

```bash
E2E_BASE_URL=https://sandbox.schoolorbit.app \
E2E_USERNAME=T0001 \
E2E_PASSWORD='your-sandbox-password' \
npm run test:e2e
```

On Ubuntu 26.04, include the same platform override when running the test:

```bash
E2E_BASE_URL=https://sandbox.schoolorbit.app \
E2E_USERNAME=T0001 \
E2E_PASSWORD='your-sandbox-password' \
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
npm run test:e2e
```

The test also accepts `SMOKE_USERNAME` and `SMOKE_PASSWORD`, so the same secrets can be reused in GitHub Actions.

The `E2E Sandbox` workflow runs the same Playwright test manually from GitHub Actions. It expects repository secrets named `SMOKE_USERNAME` and `SMOKE_PASSWORD`, and is pinned to `ubuntu-24.04` for Playwright browser support.

## Sandbox Seed

Use the seed script when the sandbox tenant needs deterministic test data. The script is idempotent and only manages the sandbox fixtures it owns:

- staff/admin login user: `T0001` by default, or `SANDBOX_ADMIN_USERNAME`
- student login user: `SBX0001`
- parent login user: `P0001`
- active academic year and two semesters
- one `ม.1/1` classroom with the seeded student enrolled
- minimal study plan/version required by classroom creation

The script refuses to run against a non-`sandbox` subdomain or a database URL that does not look like sandbox unless `SANDBOX_ALLOW_NON_SANDBOX=1` is set.

Resolve the tenant database URL through backend-admin:

```bash
SANDBOX_SUBDOMAIN=sandbox \
SANDBOX_ADMIN_API_URL=https://admin-api.schoolorbit.app \
INTERNAL_API_SECRET='your-internal-secret' \
SANDBOX_SEED_PASSWORD='your-sandbox-password' \
./scripts/seed_sandbox.sh
```

Or pass the tenant database URL directly:

```bash
SANDBOX_DATABASE_URL='postgresql://...' \
SANDBOX_SEED_PASSWORD='your-sandbox-password' \
./scripts/seed_sandbox.sh
```

Optional overrides:

```bash
SANDBOX_ADMIN_USERNAME=T0001
SANDBOX_ACADEMIC_YEAR=2569
SANDBOX_STUDENT_PASSWORD='student-password'
SANDBOX_PARENT_PASSWORD='parent-password'
SANDBOX_SKIP_MIGRATIONS=1
```
