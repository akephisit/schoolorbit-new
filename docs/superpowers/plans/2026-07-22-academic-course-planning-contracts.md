# Academic Course Planning Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export all 12 Course Planning and Teaching Assignment JSON operations through the generated Rust/OpenAPI/TypeScript contract and correct their verified validation, missing-target, and instructor-synchronization behavior.

**Architecture:** Keep handlers thin and preserve existing routes. Rust serde DTOs and `utoipa` metadata become the API source of truth; `course_planning_service` owns validation, existence checks, transactions, and DB-facing rows. Generated schemas replace handwritten frontend wire DTOs while UI-specific presentation logic remains local.

**Tech Stack:** Rust, Axum, sqlx, PostgreSQL, utoipa, SvelteKit 5, TypeScript, Node test runner

## Global Constraints

- Do not edit an applied migration; this batch requires no migration.
- Use `actor_tenant_context` once per handler and the existing generated permission constants.
- Reads require `ACADEMIC_COURSE_PLAN_READ_ALL`; mutations require `ACADEMIC_COURSE_PLAN_MANAGE_ALL`.
- Use standard `ApiResponse<T>` success envelopes and `AppError` mappings.
- Never log plaintext PII, credentials, tokens, database URLs, or raw request bodies.
- Run the Svelte autofixer for every changed `.svelte` file.
- Keep SSE, WebSocket, health/readiness, file/binary, scheduling configuration, and timetable mutations outside this batch.

---

### Task 1: Lock Course Planning authorization and request behavior

**Files:**

- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/src/modules/academic/handlers/course_planning.rs`

**Interfaces:**

- Consumes: `actor_tenant_context`, `codes::ACADEMIC_COURSE_PLAN_READ_ALL`, `codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL`
- Produces: 12 thin handlers with exact read/manage permission mappings and a strict comma-separated UUID query parser

- [ ] **Step 1: Add failing static tests for all handler mappings**

Add a focused test that extracts each function body and asserts the five read operations use `READ_ALL`, the seven mutation operations use `MANAGE_ALL`, every handler calls `actor_tenant_context`, and no handler contains `sqlx::query`, `.execute(`, or `.fetch_`.

Also add a unit test for a helper with this contract:

```rust
fn parse_course_ids(value: &str) -> Result<Vec<Uuid>, AppError>;

assert_eq!(parse_course_ids("")?, Vec::<Uuid>::new());
assert!(matches!(parse_course_ids("not-a-uuid"), Err(AppError::BadRequest(_))));
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cd backend-school
cargo test --test static_architecture course_planning
cargo test --bin backend-school parse_course_ids
```

Expected: the architecture inventory and parser test fail because the exact guard/helper do not exist.

- [ ] **Step 3: Implement the minimal handler/parser changes**

Move handler-local query DTOs into the course-planning model when needed, parse every non-empty comma-separated item, deduplicate IDs while preserving first-seen order, and return:

```rust
Err(AppError::BadRequest("course_ids must contain valid UUIDs".to_string()))
```

Do not silently discard malformed IDs. Keep handlers limited to context, permission, service call, response, and existing WebSocket broadcast.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run the commands from Step 2. Expected: all focused tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend-school/tests/static_architecture.rs \
  backend-school/src/modules/academic/handlers/course_planning.rs
git commit -m "test(academic): lock course planning authorization"
```

---

### Task 2: Correct service validation and target behavior

**Files:**

- Modify: `backend-school/src/modules/academic/models/course_planning.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/services/course_planning_service.rs`
- Create: `backend-school/src/modules/academic/services/course_planning_service_tests.rs`

**Interfaces:**

- Consumes: current classroom-course, course-instructor, activity-slot-classroom, and timetable-entry-instructor tables
- Produces: `CourseInstructorRole`, nullable primary-instructor patch semantics, actual inserted counts, and `AppError::BadRequest`/`NotFound` outcomes

- [ ] **Step 1: Add failing pure and database-backed service tests**

Cover these behaviors independently:

```rust
assert!(validate_course_instructor_role("primary").is_ok());
assert!(validate_course_instructor_role("secondary").is_ok());
assert!(matches!(
    validate_course_instructor_role("assistant"),
    Err(AppError::BadRequest(_))
));
```

Database-backed tests must verify:

- assigning to a missing classroom or semester returns `NotFound`;
- any missing subject makes the whole assignment fail before insert;
- duplicate subject IDs are normalized and the count equals rows actually inserted;
- update/delete on a missing course returns `NotFound`;
- listing instructors for a missing course returns `NotFound`;
- add rejects a missing instructor and unsupported role;
- remove/update of a missing assignment returns `NotFound` without changing the existing primary;
- explicit primary assignment updates the junction and existing timetable-entry instructors in one transaction;
- explicit `null` removes the current primary assignment and derived timetable-entry assignments;
- omitted primary field leaves the current team unchanged;
- missing classroom/semester parents return `NotFound` for activity listing;
- removing a missing classroom-slot assignment returns `NotFound`.

- [ ] **Step 2: Run the service tests and verify RED**

Run:

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school course_planning_service_tests
```

Expected: missing-target, role-validation, count, and synchronization assertions fail against current behavior.

- [ ] **Step 3: Implement typed DTO and service behavior**

Add `utoipa::ToSchema` derives and a bounded schema enum:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CourseInstructorRole {
    Primary,
    Secondary,
}
```

Keep DB-facing `role: String` fields but annotate them with `#[schema(value_type = CourseInstructorRole)]`. Add an explicit-null deserializer so `UpdateCourseRequest.primary_instructor_id` distinguishes absent, UUID, and null. Keep `settings` as the existing flexible JSON object.

Implement service helpers that:

- propagate existence-query errors with `?`;
- normalize subject IDs before validating and inserting;
- compare the number of found subjects with the distinct request count;
- check `rows_affected()` for every single-target mutation;
- lock the course and instructor assignment before primary-role changes;
- validate the target user before assigning;
- synchronize `classroom_course_instructors`, `classroom_courses.primary_instructor_id`, and `timetable_entry_instructors` inside one transaction;
- validate classroom and semester before activity listing;
- return `NotFound` for a missing classroom-slot row.

- [ ] **Step 4: Run service tests and verify GREEN**

Run the command from Step 2 plus:

```bash
cargo test --bin backend-school course_planning_service::tests
```

Expected: all new and existing course-planning service tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/modules/academic/models/course_planning.rs \
  backend-school/src/modules/academic/services.rs \
  backend-school/src/modules/academic/services/course_planning_service.rs \
  backend-school/src/modules/academic/services/course_planning_service_tests.rs
git commit -m "fix(academic): validate course planning mutations"
```

---

### Task 3: Export the 177-operation OpenAPI checkpoint

**Files:**

- Modify: `backend-school/src/modules/academic/handlers/course_planning.rs`
- Modify: `backend-school/src/modules/academic/models/course_planning.rs`
- Modify: `backend-school/src/modules/academic/services/course_planning_service.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`

**Interfaces:**

- Consumes: the 12 runtime routes and standard `ApiResponse<T>`/`ApiErrorResponse` schemas
- Produces: operation IDs from the design spec and exactly 177 unique documented operations

- [ ] **Step 1: Add failing contract inventory assertions**

Assert every method/path/operation-ID triple from the design spec and change the expected unique operation count from 165 to 177. Require the router-derived handler inventory to include every Course Planning handler.

- [ ] **Step 2: Run the contract test and verify RED**

Run:

```bash
cd backend-school
cargo test --test static_architecture api_contract
```

Expected: failures report undocumented Course Planning handlers and the old 165-operation count.

- [ ] **Step 3: Add `utoipa::path` metadata and schema registration**

Document only statuses the implementation can emit:

- success: `200` with the exact standard envelope;
- `400` for malformed UUID list, unsupported role, or invalid body;
- `401`/`403` for protected routes;
- `404` for missing parents/targets;
- `409` only where a verified domain conflict exists;
- `500` for unexpected server failures.

Register all handlers in `paths(...)` and every request/query/response/enum in `components(schemas(...))` in `api_contract.rs`.

- [ ] **Step 4: Generate artifacts and verify GREEN**

Run:

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
cd ../backend-school
cargo test --test static_architecture api_contract
```

Expected: generation is deterministic, 177 operation IDs are present and unique, and all contract tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/api_contract.rs \
  backend-school/src/modules/academic/handlers/course_planning.rs \
  backend-school/src/modules/academic/models/course_planning.rs \
  backend-school/src/modules/academic/services/course_planning_service.rs \
  backend-school/tests/static_architecture.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(api): export course planning contracts"
```

---

### Task 4: Consume generated frontend contracts

**Files:**

- Modify: `frontend-school/src/lib/api/academic.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte`
- Modify only if type adaptation is required: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Create: `frontend-school/tests/static/academic-course-planning-contract.test.mjs`

**Interfaces:**

- Consumes: generated `components['schemas']` request/response schemas
- Produces: stable API wrappers with one object-based `listClassroomCourses` parameter and no duplicate handwritten wire DTOs

- [ ] **Step 1: Add the failing frontend contract test**

The test must require generated aliases for course, instructor, role, assignment request, update request, and classroom activity schemas. It must reject local interface declarations named `ClassroomCourse`, `ClassroomCourseSettings`, or `CourseInstructor`, and require all 12 wrapper URLs/methods.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-course-planning-contract.test.mjs
```

Expected: failure reports handwritten DTOs and missing generated aliases.

- [ ] **Step 3: Replace handwritten wire DTOs and positional compatibility**

Alias generated schemas through the established pattern:

```ts
type Schemas = components['schemas'];
export type ClassroomCourse = Schemas['ClassroomCourse'];
export type CourseInstructor = Schemas['CourseInstructor'];
export type CourseInstructorRole = Schemas['CourseInstructorRole'];
```

Type mutation bodies before serialization. Change `listClassroomCourses` to accept one filters object and update the remaining positional call to:

```ts
listClassroomCourses({
  classroomId: selectedClassroomId,
  semesterId: selectedTermId
});
```

Keep UI fallbacks explicit for generated optional/nullable display fields.

- [ ] **Step 4: Run frontend checks and Svelte analysis**

Run:

```bash
cd frontend-school
node --test tests/static/academic-course-planning-contract.test.mjs
npm run check:api-contracts
npm run test:static
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/planning/+page.svelte' --svelte-version 5
PUBLIC_BACKEND_URL=http://127.0.0.1:3000 PUBLIC_VAPID_KEY=test npm run check
```

If the timetable page changes, run the autofixer for that file as well. Expected: no autofixer issues, no static failures, and zero Svelte errors/warnings.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/api/academic.ts \
  'frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte' \
  frontend-school/tests/static/academic-course-planning-contract.test.mjs
git commit -m "refactor(frontend): consume course planning schemas"
```

---

### Task 5: Document, verify, review, merge, and push

**Files:**

- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

**Interfaces:**

- Consumes: completed 177-operation checkpoint
- Produces: current developer guidance and a clean, reviewed `main` synchronized with `origin/main`

- [ ] **Step 1: Update documentation and static checkpoint tests**

Record the 177-operation count, the 12 new operation IDs, nullable primary-instructor behavior, role/missing-target rules, and scheduling configuration as the next batch. Correct the stale `L-4` row in `IMPROVEMENT_PLAN.md` to reflect that backend-admin role access is already enforced by `require_auth` and `AdminRole::can_access_admin_backend`.

- [ ] **Step 2: Run the complete backend gate**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test static_architecture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school
```

Expected: every command exits zero with no warnings or failed tests.

- [ ] **Step 3: Run the complete frontend gate**

Run:

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://127.0.0.1:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: every command exits zero and Svelte reports zero errors/warnings.

- [ ] **Step 4: Audit the final diff**

Run:

```bash
git diff --check main...HEAD
git diff --name-only main...HEAD -- backend-school/migrations backend-school/migrations_legacy
git status --short
```

Require no migration output, no uncommitted files, no duplicate operation IDs, no raw permission strings in changed production code, and no plaintext secret/PII logging.

- [ ] **Step 5: Commit documentation**

```bash
git add .rules IMPROVEMENT_PLAN.md docs/TESTING.md \
  docs/backend-school/API_DEVELOPMENT.md \
  frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "docs(api): record course planning checkpoint"
```

- [ ] **Step 6: Review, merge, push, and clean up**

Review the complete branch for Critical/Important findings, fix any findings with a red test first, then fast-forward merge to `main`, push `origin/main`, and verify:

```bash
git rev-list --left-right --count main...origin/main
```

Expected: `0 0`. Remove only this feature's clean worktree and merged branch, and stop the temporary test database without removing unrelated containers.
