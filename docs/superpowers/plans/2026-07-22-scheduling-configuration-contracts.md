# Scheduling Configuration Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace six independent Scheduling Configuration mutations with one typed atomic patch endpoint, generate the seven remaining configuration operations into the shared API contract, and move the frontend save flow onto that endpoint.

**Architecture:** Rust request/view/result DTOs are the API source of truth. A thin Axum handler authorizes one aggregate request, and `scheduling_config_service` validates every target before using bulk statements inside one transaction. The Svelte page sends one sparse patch, applies local state only after commit, and never starts auto-scheduling after a failed save.

**Tech Stack:** Rust 2021, Axum, serde, utoipa, sqlx/PostgreSQL, SvelteKit 5, TypeScript, generated OpenAPI, Node static tests.

## Global Constraints

- Keep only the six Scheduling Configuration read routes and one new `PUT /api/academic/scheduling/configuration` mutation route.
- Remove all six old Scheduling Configuration mutation routes and frontend wrappers without a compatibility period.
- Omitted patch fields are unchanged, explicit `null` clears or resets, and concrete values set the field.
- Validate the complete payload before the first mutation and commit all sections in one transaction.
- Reads require `ACADEMIC_COURSE_PLAN_READ_ALL`; the aggregate mutation requires `ACADEMIC_COURSE_PLAN_MANAGE_ALL`.
- Do not add or edit a database migration, permission definition, scheduler algorithm, or visible UI layout.
- Rust/OpenAPI is the wire-contract source of truth; generated OpenAPI and TypeScript files are never edited by hand.
- The final generated API checkpoint is exactly 184 unique operation IDs.

---

### Task 1: Define the scheduling configuration DTO and patch model

**Files:**
- Create: `backend-school/src/modules/academic/models/scheduling_config.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Test: `backend-school/src/modules/academic/models/scheduling_config.rs`

**Interfaces:**
- Produces: `Patch<T>`, `SaveSchedulingConfigurationRequest`, `SchedulerSettingsPatch`, `InstructorConstraintPatch`, `SubjectConstraintPatch`, `ClassroomCourseConstraintPatch`, `ClassroomCoursePreferredRoomsPatch`, `ListClassroomCourseConstraintsQuery`, `SchedulerSettingsView`, `InstructorConstraintView`, `SubjectConstraintView`, `ClassroomCourseConstraintView`, `CcPreferredRoomView`, `SchedulingRoomView`, and `SchedulingConfigurationSaveResult`.
- Consumes: `TimeSlot` from `models/scheduling.rs`, `Uuid`, serde, and utoipa.

- [ ] **Step 1: Add failing tri-state deserialization tests**

Add focused tests that deserialize these three bodies and assert `Patch::Unchanged`, `Patch::Clear`, and `Patch::Set(6)` respectively:

```rust
#[test]
fn scheduler_patch_distinguishes_missing_null_and_value() {
    let missing: SaveSchedulingConfigurationRequest = serde_json::from_str("{}").unwrap();
    let cleared: SaveSchedulingConfigurationRequest = serde_json::from_str(
        r#"{"scheduler_settings":{"default_max_consecutive":null}}"#,
    )
    .unwrap();
    let set: SaveSchedulingConfigurationRequest = serde_json::from_str(
        r#"{"scheduler_settings":{"default_max_consecutive":6}}"#,
    )
    .unwrap();

    assert!(matches!(
        missing.scheduler_settings,
        None
    ));
    assert!(matches!(
        cleared.scheduler_settings.unwrap().default_max_consecutive,
        Patch::Clear
    ));
    assert!(matches!(
        set.scheduler_settings.unwrap().default_max_consecutive,
        Patch::Set(6)
    ));
}
```

Add equivalent assertions for nullable instructor room, subject restrictions, classroom-course pattern, and omitted collections defaulting to empty.

- [ ] **Step 2: Run the model test and verify RED**

Run:

```bash
cd backend-school
cargo test modules::academic::models::scheduling_config::tests --bin backend-school
```

Expected: compilation fails because the scheduling configuration module and DTOs do not exist.

- [ ] **Step 3: Implement the typed patch boundary**

Create the model module with a generic in-memory state and a serde helper that only runs when a property is present:

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Patch<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

pub fn deserialize_patch<'de, D, T>(deserializer: D) -> Result<Patch<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => Patch::Set(value),
        None => Patch::Clear,
    })
}
```

Every patchable field uses `#[serde(default, deserialize_with = "deserialize_patch")]`. Aggregate collections use `#[serde(default)]`. Add `ToSchema` metadata so nullable patches render as optional nullable values, not as an internal enum. Keep response fields snake_case to match the existing wire format. Move the public view structs out of `scheduling_config_service.rs`; DB row structs remain private there.

- [ ] **Step 4: Run the model tests and format check**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::academic::models::scheduling_config::tests --bin backend-school
```

Expected: all scheduling configuration model tests pass.

- [ ] **Step 5: Commit the model boundary**

```bash
git add backend-school/src/modules/academic/models.rs backend-school/src/modules/academic/models/scheduling_config.rs
git commit -m "feat(academic): define scheduling configuration patch contract"
```

### Task 2: Implement the atomic aggregate service with database-backed TDD

**Files:**
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/services/scheduling_config_service.rs`
- Create: `backend-school/src/modules/academic/services/scheduling_config_service_tests.rs`

**Interfaces:**
- Consumes: every Task 1 request/view/result DTO.
- Produces: `save_scheduling_configuration(pool: &PgPool, request: SaveSchedulingConfigurationRequest) -> Result<SchedulingConfigurationSaveResult, AppError>` and the existing six typed read functions.

- [ ] **Step 1: Add the database fixture and failing atomicity test**

Register `#[cfg(test)] mod scheduling_config_service_tests;` in `services.rs`. Follow `course_planning_service_tests.rs` by using `create_test_pool()` and `run_test_migrations()`. The fixture inserts a unique active academic year, semester, period, active staff user, active subject, active room, classroom, and classroom course.

Add a test that patches the scheduler maximum and a valid instructor together with one missing classroom-course ID:

```rust
let result = scheduling_config_service::save_scheduling_configuration(
    &pool,
    SaveSchedulingConfigurationRequest {
        scheduler_settings: Some(settings_patch(7)),
        instructors: vec![instructor_patch(fixture.instructor_id)],
        classroom_courses: vec![classroom_course_patch(Uuid::new_v4())],
        ..Default::default()
    },
)
.await;

assert!(matches!(result, Err(AppError::NotFound(_))));
assert_eq!(load_default_max_consecutive(&pool).await, 4);
assert!(load_instructor_preference(&pool, fixture.instructor_id).await.is_none());
```

- [ ] **Step 2: Add failing validation, clear, and affected-count tests**

Cover these independent behaviors:

- duplicate target IDs return `400` and write nothing;
- inactive/missing instructors, subjects, rooms, and classroom courses return `404`;
- cross-year period and classroom-course IDs return `404`;
- invalid day codes, ranges, duplicate room/rank keys, and inconsistent patterns return `400`;
- explicit `null` resets scheduler maximum to `4`, priority to `100`, assigned room to no row, slot arrays to `[]`, and nullable restrictions/patterns to SQL `NULL`;
- an empty patch returns `changed = false` and zero counts;
- repeating an already-applied patch returns zero state-change counts.

- [ ] **Step 3: Run focused service tests and verify RED**

Run:

```bash
cd backend-school
TEST_DATABASE_URL="${SCHEDULING_CONFIG_TEST_DATABASE_URL:?set a disposable PostgreSQL test URL}" \
  cargo test modules::academic::services::scheduling_config_service_tests --bin backend-school
```

Expected: compilation fails because `save_scheduling_configuration` is not implemented.

- [ ] **Step 4: Refactor read DTO ownership without behavior changes**

Import the Task 1 public view DTOs into `scheduling_config_service.rs`, retain private `sqlx::FromRow` structs, and keep these exact read signatures:

```rust
pub async fn list_instructor_constraints(pool: &PgPool) -> Result<Vec<InstructorConstraintView>, AppError>;
pub async fn list_subject_constraints(pool: &PgPool) -> Result<Vec<SubjectConstraintView>, AppError>;
pub async fn get_scheduler_settings(pool: &PgPool) -> Result<SchedulerSettingsView, AppError>;
pub async fn list_classroom_course_constraints(
    pool: &PgPool,
    instructor_id: Option<Uuid>,
) -> Result<Vec<ClassroomCourseConstraintView>, AppError>;
pub async fn list_cc_preferred_rooms(
    pool: &PgPool,
    classroom_course_id: Uuid,
) -> Result<Vec<CcPreferredRoomView>, AppError>;
pub async fn list_all_rooms(pool: &PgPool) -> Result<Vec<SchedulingRoomView>, AppError>;
```

- [ ] **Step 5: Implement prevalidation and serialization**

Begin one transaction, select the active academic year `FOR UPDATE`, normalize each patch, reject duplicate keys with `HashSet`, and bulk-load all referenced records before writing. Use one canonical helper for the effective periods-per-week calculation used by both reads and pattern validation:

```rust
fn effective_periods_per_week(
    periods_per_week: Option<i32>,
    hours_per_semester: f64,
    credit: f64,
) -> i32 {
    periods_per_week.unwrap_or_else(|| {
        if hours_per_semester > 0.0 {
            (hours_per_semester / 20.0).ceil() as i32
        } else if credit > 0.0 {
            (credit * 2.0).ceil() as i32
        } else {
            2
        }
    })
}
```

Return `BadRequest` for semantic validation, `NotFound` for absent/inactive/cross-year references, and `Conflict` for expected PostgreSQL integrity conflicts.

- [ ] **Step 6: Implement bulk writes and result counts**

Use bulk statements per section. Instructor preference upserts and subject/classroom-course updates use `IS DISTINCT FROM` predicates. Instructor-room assignments delete/insert only for changed room patches. Preferred-room sets are compared canonically, then affected course sets are bulk-deleted and replacement rows inserted with `QueryBuilder` or `UNNEST`. Commit only after all sections succeed.

Return:

```rust
SchedulingConfigurationSaveResult {
    changed: total_changed > 0,
    scheduler_settings_changed,
    instructor_order_updated,
    instructor_constraints_updated,
    subject_constraints_updated,
    classroom_course_constraints_updated,
    preferred_room_sets_updated,
}
```

- [ ] **Step 7: Run the focused model/service suite and Clippy**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="${SCHEDULING_CONFIG_TEST_DATABASE_URL:?set a disposable PostgreSQL test URL}" \
  cargo test modules::academic::services::scheduling_config_service_tests --bin backend-school
```

Expected: all focused tests pass with no warnings.

- [ ] **Step 8: Commit the atomic service**

```bash
git add backend-school/src/modules/academic/services.rs \
  backend-school/src/modules/academic/services/scheduling_config_service.rs \
  backend-school/src/modules/academic/services/scheduling_config_service_tests.rs
git commit -m "feat(academic): save scheduling configuration atomically"
```

### Task 3: Replace mutation routes and export the seven backend contracts

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/scheduling_config.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: Task 1 DTOs and Task 2 service functions.
- Produces: six documented reads, `save_scheduling_configuration` handler, standard error schemas, and exactly 184 unique OpenAPI operations.

- [ ] **Step 1: Add failing architecture and OpenAPI tests**

Add a static test that asserts the router contains only these Scheduling Configuration methods:

```rust
for expected in [
    "get(handlers::scheduling_config::list_instructor_constraints)",
    "get(handlers::scheduling_config::list_subject_constraints)",
    "get(handlers::scheduling_config::get_scheduler_settings)",
    "get(handlers::scheduling_config::list_classroom_course_constraints)",
    "get(handlers::scheduling_config::list_cc_preferred_rooms)",
    "get(handlers::scheduling_config::list_all_rooms)",
    "put(handlers::scheduling_config::save_scheduling_configuration)",
] {
    assert!(router.contains(expected), "missing route: {expected}");
}
```

Assert the six old mutation handler names are absent from the router. Assert each read uses `ACADEMIC_COURSE_PLAN_READ_ALL`, the mutation uses `ACADEMIC_COURSE_PLAN_MANAGE_ALL`, and JSON uses `Result<Json<SaveSchedulingConfigurationRequest>, JsonRejection>`.

Add `api_contract::tests::academic_scheduling_configuration_contracts` for the seven operation IDs, 400/401/403/404/409 envelopes, nullable patch fields, named view/result schemas, and total unique operation count `184`.

- [ ] **Step 2: Run backend contract tests and verify RED**

Run:

```bash
cd backend-school
cargo test api_contract::tests::academic_scheduling_configuration_contracts --bin backend-school
cargo test --test static_architecture scheduling_configuration
```

Expected: tests fail because paths and route changes do not exist.

- [ ] **Step 3: Implement the thin handlers and router replacement**

Delete the six old mutation request structs and handlers. Add one handler with this boundary:

```rust
pub async fn save_scheduling_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload_result: Result<Json<SaveSchedulingConfigurationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let Json(payload) = parse_json_payload(payload_result)?;
    let result = scheduling_config_service::save_scheduling_configuration(
        &context.tenant.pool,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}
```

Keep only GET on the six read paths and register PUT on `/scheduling/configuration`.

- [ ] **Step 4: Add utoipa metadata and schema registration**

Annotate every handler with stable operation IDs from the design. Document success envelopes and relevant standard errors. Register all seven paths and every named scheduling schema in `SchoolApi`.

- [ ] **Step 5: Run backend contract and architecture tests GREEN**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test api_contract::tests::academic_scheduling_configuration_contracts --bin backend-school
cargo test --test static_architecture
```

Expected: contract test passes, all architecture tests pass, and the rendered document contains 184 unique operations.

- [ ] **Step 6: Commit the router and contract source**

```bash
git add backend-school/src/modules/academic/handlers/scheduling_config.rs \
  backend-school/src/modules/academic.rs backend-school/src/api_contract.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(api): export scheduling configuration contracts"
```

### Task 4: Generate TypeScript and migrate the Svelte save flow

**Files:**
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/scheduling.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte`
- Create: `frontend-school/tests/static/academic-scheduling-configuration-contract.test.mjs`

**Interfaces:**
- Consumes: generated `components['schemas']` types and the `saveSchedulingConfiguration` operation.
- Produces: one frontend mutation wrapper and a save function that propagates failure.

- [ ] **Step 1: Add a failing frontend ownership/save-flow test**

The static test loads OpenAPI, generated TypeScript, `scheduling.ts`, and the page. Assert all seven operations exist. Assert scheduling API no longer declares handwritten interfaces for generated scheduling DTOs and no longer exports these wrappers:

```js
for (const removed of [
  'updateInstructorConstraints',
  'reorderInstructorPriority',
  'updateSchoolSettings',
  'updateSubjectConstraints',
  'updateClassroomCourseConstraints',
  'setCcPreferredRooms'
]) {
  assert.doesNotMatch(schedulingApi, new RegExp(`export\\s+async\\s+function\\s+${removed}\\b`));
}
assert.match(schedulingApi, /export async function saveSchedulingConfiguration/);
assert.match(page, /await saveSchedulingConfiguration\(patch\)/);
assert.doesNotMatch(page, /await Promise\.all\(ops\)/);
```

Also assert `runAutoSchedule` checks the save result or lets a thrown save error stop execution before `autoScheduleTimetable`.

- [ ] **Step 2: Run the focused frontend test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-scheduling-configuration-contract.test.mjs
```

Expected: failure because OpenAPI and frontend wrappers still use the old surface.

- [ ] **Step 3: Generate OpenAPI and TypeScript**

Run:

```bash
cd frontend-school
npm run generate:api-contracts
```

Do not hand-edit either generated file.

- [ ] **Step 4: Replace handwritten scheduling DTOs and wrappers**

In `scheduling.ts`, add the standard aliases:

```ts
type Schemas = components['schemas'];
export type SaveSchedulingConfigurationRequest = Schemas['SaveSchedulingConfigurationRequest'];
export type SchedulingConfigurationSaveResult = Schemas['SchedulingConfigurationSaveResult'];
export type InstructorConstraintView = Schemas['InstructorConstraintView'];
export type SubjectConstraintView = Schemas['SubjectConstraintView'];
export type ClassroomCourseConstraintView = Schemas['ClassroomCourseConstraintView'];
export type CcPreferredRoom = Schemas['CcPreferredRoomView'];
export type RoomView = Schemas['SchedulingRoomView'];
```

Keep the six GET wrappers and replace six PUT wrappers with:

```ts
export async function saveSchedulingConfiguration(request: SaveSchedulingConfigurationRequest) {
  return apiClient.put<SchedulingConfigurationSaveResult>(
    '/api/academic/scheduling/configuration',
    request
  );
}
```

- [ ] **Step 5: Migrate `saveAll()` to one sparse request**

Build one `SaveSchedulingConfigurationRequest` from dirty sections. Do not clear local dirty state until the request succeeds. Make `saveAll()` return the successful result or throw; its catch site owns the toast. `runAutoSchedule()` must return immediately on a save exception and must never call `autoScheduleTimetable` after a failed save.

- [ ] **Step 6: Run Svelte analysis and focused tests**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte' \
  --svelte-version 5
node --test tests/static/academic-scheduling-configuration-contract.test.mjs
npm run check:api-contracts
PUBLIC_BACKEND_URL=http://localhost:8080 PUBLIC_VAPID_KEY=test npm run check
```

Expected: autofixer reports no issues, focused static test passes, generated files are current, and Svelte reports zero errors/warnings.

- [ ] **Step 7: Commit generated and frontend changes**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/scheduling.ts \
  'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte' \
  frontend-school/tests/static/academic-scheduling-configuration-contract.test.mjs
git commit -m "refactor(frontend): save scheduling configuration atomically"
```

### Task 5: Update the checkpoint and run the full phase gate

**Files:**
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`

**Interfaces:**
- Consumes: the final seven-operation contract and test commands.
- Produces: a documented 184-operation checkpoint and the next bounded rollout target.

- [ ] **Step 1: Update developer documentation**

Record the seven Scheduling Configuration operations, the removed mutation routes, atomic patch semantics, standard errors, 184-operation checkpoint, and the exact generation/test commands. Mark Auto-scheduling Jobs and Locked Slots as the next separate contract candidates. Do not claim those routes are implemented in OpenAPI.

- [ ] **Step 2: Run the complete backend gate**

Run against the isolated test database:

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test static_architecture
TEST_DATABASE_URL="${SCHEDULING_CONFIG_TEST_DATABASE_URL:?set a disposable PostgreSQL test URL}" \
  cargo test --bin backend-school
```

Expected: zero failures and zero Clippy warnings.

- [ ] **Step 3: Run the complete frontend/contract gate**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:8080 PUBLIC_VAPID_KEY=test npm run check
```

Expected: all Node tests pass and Svelte reports zero errors/warnings.

- [ ] **Step 4: Audit scope and generated inventory**

Run:

```bash
git diff --check
git status --short
git diff -- backend-school/migrations
node -e "const d=require('./contracts/openapi/school-api.json'); const ops=Object.values(d.paths).flatMap(p=>['get','post','put','patch','delete'].filter(m=>p[m]).map(m=>p[m].operationId)); if(ops.length!==184||new Set(ops).size!==184) process.exit(1); console.log('184 unique operations');"
```

Expected: clean diff syntax, no migration diff, and `184 unique operations`.

- [ ] **Step 5: Request final review and commit the checkpoint**

Review the implementation against the design, fix every Critical/Important finding with a failing regression test first, rerun the affected gate, then commit:

```bash
git add IMPROVEMENT_PLAN.md docs/TESTING.md docs/backend-school/API_DEVELOPMENT.md
git commit -m "docs(api): record scheduling configuration checkpoint"
```

- [ ] **Step 6: Integrate the verified branch**

Fast-forward the verified feature branch into `main`, rerun the relevant merged-state gates, push `main`, verify local and remote SHA equality, then remove only the clean merged worktree and its temporary test database container.
