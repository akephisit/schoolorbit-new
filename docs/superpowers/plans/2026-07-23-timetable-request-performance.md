# Timetable Request Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace timetable activity-slot request fan-out with one typed batch endpoint, prevent stale frontend loads, and add a timetable index only if representative query plans justify it.

**Architecture:** Add a semester-scoped activity timetable-context endpoint backed by three fixed-count service queries. Extend the API client with `AbortSignal`, add a pure in-flight request coordinator, and update the Svelte timetable page to derive sidebar state from one context response. Keep per-slot management APIs intact.

**Tech Stack:** Rust/Axum/sqlx/PostgreSQL, Utoipa/OpenAPI, TypeScript, SvelteKit 5 runes, Node tests, Svelte MCP/autofixer.

**Approved design:** `docs/superpowers/specs/2026-07-23-timetable-request-performance-design.md`

## Global Constraints

- Preserve visible timetable workflows, manual editing, optimistic updates, WebSocket protocol, and existing per-slot management APIs.
- The new endpoint requires timetable read permission plus the existing activity list access policy.
- Return typed DTOs through the standard `ApiResponse<T>` envelope.
- Do not add Redis, caching services, Prometheus, Grafana, mutation retries, or auto-scheduling.
- Do not edit an applied migration. If evidence justifies the candidate index, create `backend-school/migrations/029_timetable_active_semester_slot.sql`.
- An aborted request is not a user-visible error; all other errors retain current handling.
- Generated OpenAPI/TypeScript artifacts are updated only through `npm run generate:api-contracts`.
- Use Svelte official documentation/autofixer for every timetable component edit.
- Each green implementation slice ends with focused tests and a commit; red tests stay
  uncommitted until the matching implementation is green.

## Target File Map

**Create:**

- `frontend-school/src/lib/utils/request-coordinator.ts`
- `frontend-school/src/lib/utils/timetable-activity-context.ts`
- `frontend-school/tests/static/request-coordinator.test.mjs`
- `frontend-school/tests/static/timetable-request-performance.test.mjs`
- `scripts/analyze-timetable-index.sql`
- `docs/performance/TIMETABLE_QUERY_PLANS_2026-07-23.md`
- Conditional: `backend-school/migrations/029_timetable_active_semester_slot.sql`

**Modify:**

- `backend-school/src/modules/academic/models/activity.rs`
- `backend-school/src/modules/academic/services/activity_service.rs`
- `backend-school/src/modules/academic/services/activity_service_tests.rs`
- `backend-school/src/modules/academic/handlers/activity.rs`
- `backend-school/src/modules/academic.rs`
- `backend-school/src/api_contract.rs`
- `backend-school/tests/static_architecture.rs`
- `contracts/openapi/school-api.json`
- `frontend-school/src/lib/api/generated/school-api.ts`
- `frontend-school/src/lib/api/client.ts`
- `frontend-school/src/lib/api/academic.ts`
- `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- `frontend-school/tests/static/academic-activity-workspace-contract.test.mjs`

## Interfaces

Backend response:

```rust
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySlotTimetableContextResponse {
    pub slots: Vec<ActivitySlot>,
    pub instructors_by_slot: HashMap<Uuid, Vec<SlotInstructorInfo>>,
    pub classroom_assignments_by_slot: HashMap<Uuid, Vec<SlotClassroomAssignment>>,
}
```

Backend query:

```rust
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActivityTimetableContextQuery {
    pub semester_id: Uuid,
}
```

Frontend request options:

```ts
export interface ApiRequestOptions {
    signal?: AbortSignal;
}
```

Request coordinator:

```ts
export interface RequestCoordinator {
    run<T>(
        scope: string,
        key: string,
        operation: (signal: AbortSignal) => Promise<T>
    ): Promise<T>;
    abort(scope: string): void;
    abortAll(): void;
}
```

---

### Task 1: Add Failing Backend Context Tests

**Files:**

- Modify: `backend-school/src/modules/academic/services/activity_service_tests.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Produces: expected service grouping and route/contract boundary.

- [ ] **Step 1: Add the service test before the service exists**

Add:

```rust
#[tokio::test]
async fn timetable_context_groups_all_semester_slots_instructors_and_assignments() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    let fixture = insert_activity_timetable_context_fixture(&pool).await;

    let context = activity_service::get_timetable_context(
        &pool,
        fixture.semester_id,
        UserResourceListAccess::School,
    )
    .await
    .unwrap();

    assert_eq!(context.slots.len(), 2);
    assert_eq!(context.instructors_by_slot[&fixture.synchronized_slot_id].len(), 2);
    assert_eq!(
        context.classroom_assignments_by_slot[&fixture.independent_slot_id].len(),
        2
    );
    assert!(context
        .instructors_by_slot
        .contains_key(&fixture.independent_slot_id));
}
```

The fixture inserts two slots in the requested semester, one slot in another semester, two slot
instructors, and two classroom assignments. Assert the other-semester IDs are absent.

Run:

```bash
cd backend-school
cargo test timetable_context_groups_all_semester_slots_instructors_and_assignments --no-run
```

Expected: FAIL because `get_timetable_context` and the response type do not exist.

- [ ] **Step 2: Add empty-semester behavior**

Add:

```rust
#[tokio::test]
async fn timetable_context_returns_empty_collections_for_an_empty_semester() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    let context = activity_service::get_timetable_context(
        &pool,
        Uuid::new_v4(),
        UserResourceListAccess::School,
    )
    .await
    .unwrap();
    assert!(context.slots.is_empty());
    assert!(context.instructors_by_slot.is_empty());
    assert!(context.classroom_assignments_by_slot.is_empty());
}
```

- [ ] **Step 3: Add a failing architecture contract test**

Require:

```text
GET /activity-slots/timetable-context
handlers::activity::get_timetable_context
operation_id = "getActivitySlotTimetableContext"
ACADEMIC_COURSE_PLAN_READ_ALL
resolve_activity_list_access
```

Run the focused static test and confirm failure because the route is absent.

- [ ] **Step 4: Keep the red tests for Task 2**

Do not commit a failing build. Leave the red tests uncommitted and implement Task 2 immediately.

---

### Task 2: Implement the Fixed-Query Backend Endpoint

**Files:**

- Modify: `backend-school/src/modules/academic/models/activity.rs`
- Modify: `backend-school/src/modules/academic/services/activity_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/activity.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**

- Consumes: `list_slots`, `SlotInstructorInfo`, `SlotClassroomAssignment`, actor/list access.
- Produces: `GET /api/academic/activity-slots/timetable-context`.

- [ ] **Step 1: Add the typed response**

Import `HashMap` in `models/activity.rs` and add the response interface shown in this plan.
Because `SlotInstructorInfo` currently lives in the service, either move that stable API DTO to
`models/activity.rs` or import it into the response only if doing so does not create a module
cycle. The selected implementation is to move `InstructorInfo` and `SlotInstructorInfo` to the
activity model file and update existing imports without changing serialization.

- [ ] **Step 2: Implement fixed-count service queries**

Add:

```rust
pub async fn get_timetable_context(
    pool: &PgPool,
    semester_id: Uuid,
    access: UserResourceListAccess,
) -> Result<ActivitySlotTimetableContextResponse, AppError> {
    let slots = list_slots(
        pool,
        ActivitySlotFilter {
            semester_id: Some(semester_id),
            activity_type: None,
            teacher_reg_open: None,
            student_reg_open: None,
        },
        access,
    )
    .await?;
    let slot_ids = slots.iter().map(|slot| slot.id).collect::<Vec<_>>();
    if slot_ids.is_empty() {
        return Ok(ActivitySlotTimetableContextResponse {
            slots,
            instructors_by_slot: HashMap::new(),
            classroom_assignments_by_slot: HashMap::new(),
        });
    }

    let instructors = list_instructors_for_slots(pool, &slot_ids).await?;
    let assignments = list_classroom_assignments_for_slots(pool, &slot_ids).await?;
    Ok(group_timetable_context(slots, instructors, assignments))
}
```

The two new list helpers each use one `WHERE slot_id = ANY($1)` query. `group_timetable_context`
initializes empty vectors for every slot before pushing rows, so map lookup is stable for slots
without assignments.

- [ ] **Step 3: Add handler, route, and OpenAPI registration**

The handler:

```rust
pub async fn get_timetable_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ActivityTimetableContextQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let access = activity_access_policy::resolve_activity_list_access(&context.actor)?;
    let data = activity_service::get_timetable_context(
        &context.tenant.pool,
        query.semester_id,
        access,
    )
    .await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}
```

Register the static route before `"/activity-slots/{id}"` routes and add the path/schema to
`api_contract.rs`.

- [ ] **Step 4: Verify green and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test timetable_context_ -- --test-threads=1
cargo test --test static_architecture activity_timetable_context
cargo check --all-targets
```

```bash
git add backend-school/src/modules/academic/models/activity.rs backend-school/src/modules/academic/services/activity_service.rs backend-school/src/modules/academic/services/activity_service_tests.rs backend-school/src/modules/academic/handlers/activity.rs backend-school/src/modules/academic.rs backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "feat(perf): add activity timetable context"
```

---

### Task 3: Generate and Verify Shared API Contracts

**Files:**

- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/academic-activity-workspace-contract.test.mjs`

**Interfaces:**

- Produces: generated operation `getActivitySlotTimetableContext`.

- [ ] **Step 1: Add the failing generated-contract assertion**

Assert the OpenAPI document contains the exact path/operation and the generated TypeScript
operation returns `ActivitySlotTimetableContextResponse`. Run the test and confirm failure before
generation.

- [ ] **Step 2: Generate artifacts**

```bash
cd frontend-school
npm run generate:api-contracts
```

- [ ] **Step 3: Verify and commit**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/academic-activity-workspace-contract.test.mjs
```

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/tests/static/academic-activity-workspace-contract.test.mjs
git commit -m "feat(contracts): expose activity timetable context"
```

---

### Task 4: Add Abortable API Requests and Request Coordinator

**Files:**

- Create: `frontend-school/src/lib/utils/request-coordinator.ts`
- Create: `frontend-school/tests/static/request-coordinator.test.mjs`
- Modify: `frontend-school/src/lib/api/client.ts`
- Modify: `frontend-school/src/lib/api/academic.ts`

**Interfaces:**

- Produces: optional `ApiRequestOptions`, `getActivitySlotTimetableContext`, and coordinator API.

- [ ] **Step 1: Write failing coordinator tests**

Add this helper and the three Node tests:

```js
function deferred() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, resolve, reject };
}

test('same scope and key reuse one in-flight operation', async () => {
    const coordinator = createRequestCoordinator();
    const gate = deferred();
    let calls = 0;
    const operation = async () => {
        calls += 1;
        await gate.promise;
        return 7;
    };
    const first = coordinator.run('activity', 'semester-a', operation);
    const second = coordinator.run('activity', 'semester-a', operation);
    assert.strictEqual(first, second);
    gate.resolve();
    assert.equal(await second, 7);
    assert.equal(calls, 1);
});

test('a new key aborts the prior request without deleting the newer record', async () => {
    const coordinator = createRequestCoordinator();
    let firstSignal;
    const first = coordinator.run('activity', 'semester-a', (signal) => {
        firstSignal = signal;
        return new Promise((_resolve, reject) => {
            signal.addEventListener('abort', () => reject(signal.reason), { once: true });
        });
    });
    const second = coordinator.run('activity', 'semester-b', async () => 2);
    assert.equal(firstSignal?.aborted, true);
    await assert.rejects(first, (error) => isAbortError(error));
    assert.equal(await second, 2);
});

test('abortAll aborts every scope', async () => {
    const coordinator = createRequestCoordinator();
    const signals = [];
    const pending = ['activity', 'entries'].map((scope) =>
        coordinator.run(scope, 'semester-a', (signal) => {
            signals.push(signal);
            return new Promise((_resolve, reject) => {
                signal.addEventListener('abort', () => reject(signal.reason), { once: true });
            });
        })
    );
    coordinator.abortAll();
    assert.deepEqual(signals.map((signal) => signal.aborted), [true, true]);
    await Promise.all(pending.map((promise) => assert.rejects(promise, isAbortError)));
});
```

Run and confirm module-not-found failure.

- [ ] **Step 2: Implement coordinator**

Implement an in-flight `Map<string, { key; controller; promise }>` keyed by scope. `run` returns
the exact stored promise for the same key; a different key aborts and replaces it. In `finally`,
delete only when the stored promise is the completing promise. Implement `run` as a normal
method, not an `async` method, so same-key callers receive the identical `Promise` object.

Add:

```ts
export function isAbortError(error: unknown): boolean {
    return error instanceof DOMException && error.name === 'AbortError';
}
```

- [ ] **Step 3: Extend the API client**

Change `request`, `get`, and `getBlob` to accept optional request options and forward `signal`.
Keep every existing call source-compatible:

```ts
async get<T>(endpoint: string, options: ApiRequestOptions = {}): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'GET', signal: options.signal });
}
```

Add `getActivitySlotTimetableContext(semesterId, options)` using the generated schema aliases.

- [ ] **Step 4: Verify and commit**

```bash
cd frontend-school
node --test tests/static/request-coordinator.test.mjs
npm run check
```

```bash
git add frontend-school/src/lib/utils/request-coordinator.ts frontend-school/tests/static/request-coordinator.test.mjs frontend-school/src/lib/api/client.ts frontend-school/src/lib/api/academic.ts
git commit -m "feat(frontend): coordinate abortable timetable requests"
```

---

### Task 5: Replace Per-Slot Timetable Requests

**Files:**

- Create: `frontend-school/src/lib/utils/timetable-activity-context.ts`
- Create: `frontend-school/tests/static/timetable-request-performance.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`

**Interfaces:**

- Consumes: context API and request coordinator.
- Produces: one activity-context request per semester/key with stale-result protection.

- [ ] **Step 1: Add failing static/resource tests**

Require the page to import `getActivitySlotTimetableContext`,
`createRequestCoordinator`, and `isAbortError`; reject imports/calls of
`listSlotInstructors` and `listSlotClassroomAssignments`; require `abortAll()` in `onDestroy`.
Import pure functions from the not-yet-created `timetable-activity-context.ts` and assert a fixture
with two synchronized and two independent slots selects the correct instructor slots, classroom
items, and `${slotId}|${classroomId}` instructor map. Run and confirm module-not-found failure.

- [ ] **Step 2: Replace sidebar loading**

Create one coordinator instance. Replace `loadSidebarActivitySlots()` per-slot calls with one
context call keyed by semester ID. Use `instructorsBySlot` for synchronized membership and
`classroomAssignmentsBySlot` for independent items and `activityInstructorMap`.

Implement and use these pure functions:

```ts
export interface InstructorActivityItem {
    slot: ActivitySlot;
    classroom_id: string;
    classroom_name: string;
}

export function synchronizedSlotsForInstructor(
    slots: ActivitySlot[],
    instructorsBySlot: Record<string, SlotInstructorInfo[]>,
    instructorId: string
): ActivitySlot[];

export function independentItemsForInstructor(
    slots: ActivitySlot[],
    assignmentsBySlot: Record<string, SlotClassroomAssignment[]>,
    instructorId: string
): InstructorActivityItem[];

export function activityInstructorEntries(
    assignmentsBySlot: Record<string, SlotClassroomAssignment[]>
): Array<[string, { id: string; name: string }]>;
```

Replace `checkClassroomHasInstructor()` with a lookup in the loaded assignment map. Keep
`listActivityGroups` separate because it is user-filtered and not part of the semester context.

- [ ] **Step 3: Coordinate parallel selection loads**

Create a selection key from semester, view mode, classroom, instructor, and team-ghost state. Run
entries, occupancy, courses, context, and instructor groups in parallel after required selections
exist. Each loader commits only if the current key still matches. Ignore `AbortError`; preserve
existing toasts for other failures.

Use `onDestroy(() => coordinator.abortAll())`. Do not move network work into an unbounded
`$effect`.

- [ ] **Step 4: Run Svelte correction loop**

Run the official Svelte autofixer against `+page.svelte`, apply every relevant correction, then
run it again until it reports no actionable issue.

- [ ] **Step 5: Verify and commit**

```bash
cd frontend-school
node --test tests/static/timetable-request-performance.test.mjs
npm run test:static
npm run check
```

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' frontend-school/src/lib/utils/timetable-activity-context.ts frontend-school/tests/static/timetable-request-performance.test.mjs
git commit -m "perf(timetable): batch activity context loading"
```

---

### Task 6: Measure the Candidate Index and Apply Only with Evidence

**Files:**

- Create: `scripts/analyze-timetable-index.sql`
- Create: `docs/performance/TIMETABLE_QUERY_PLANS_2026-07-23.md`
- Conditional create: `backend-school/migrations/029_timetable_active_semester_slot.sql`

**Interfaces:**

- Produces: reproducible before/after plan evidence and, only if beneficial, an immutable migration.

- [ ] **Step 1: Create representative plan script**

The SQL script sets `target_semester` to
`00000000-0000-0000-0000-000000000003`, runs in a transaction, creates a temporary
timetable-shaped table, inserts 100,000 rows across five deterministic semester UUIDs with 80%
active rows, creates the existing separate semester and day/period indexes, analyzes the table,
and captures:

```sql
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT id, academic_semester_id, day_of_week, period_id
FROM timetable_plan_fixture
WHERE academic_semester_id = :'target_semester'
  AND is_active = true
ORDER BY day_of_week, period_id;
```

Then create the candidate partial composite index, analyze again, and run the same plan. Roll back
at the end so no persistent test data remains.

- [ ] **Step 2: Run and record evidence**

```bash
for run_number in 1 2 3 4 5; do
  psql "$TEST_DATABASE_URL" -v ON_ERROR_STOP=1 \
    -f scripts/analyze-timetable-index.sql \
    > "/tmp/schoolorbit-timetable-plan-${run_number}.txt"
done
```

Record row count, plan node, median actual time, shared buffer reads/hits, and index size
before/after in `docs/performance/TIMETABLE_QUERY_PLANS_2026-07-23.md`. Remove the five temporary
text outputs after recording the aggregate; they are not repository artifacts.

- [ ] **Step 3: Apply the explicit decision rule**

Create migration `029_timetable_active_semester_slot.sql` only when the candidate removes the
avoidable scan/bitmap work and reduces total shared buffers or median execution time by at least
20% across five script runs:

```sql
CREATE INDEX idx_timetable_active_semester_slot
ON academic_timetable_entries (
    academic_semester_id,
    day_of_week,
    period_id
)
WHERE is_active = true;
```

If it misses the threshold, the result document explicitly states `Decision: no migration`; do
not create a migration file.

- [ ] **Step 4: Verify and commit**

```bash
git diff --check
cd backend-school
cargo test --test static_architecture migration -- --test-threads=1
```

```bash
git add scripts/analyze-timetable-index.sql docs/performance/TIMETABLE_QUERY_PLANS_2026-07-23.md
git add backend-school/migrations/029_timetable_active_semester_slot.sql  # only when created
git commit -m "perf(db): document timetable index analysis"
```

---

### Task 7: Full Cross-Stack Verification

**Files:**

- Modify only for proven regressions.

- [ ] **Step 1: Backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school -- --test-threads=1
cargo test --test static_architecture -- --test-threads=1
```

- [ ] **Step 2: Frontend and contract verification**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run test:static
npm run check
npm run lint
```

- [ ] **Step 3: Final Svelte verification**

Run the official Svelte autofixer once more on the final timetable page and confirm no actionable
issue remains.

- [ ] **Step 4: Scope and repository audit**

```bash
git diff --check
git status --short
rg -n 'listSlotInstructors|listSlotClassroomAssignments' 'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'
git log --oneline -8
```

Expected: the final search has no matches, generated contracts are current, migrations are
unchanged unless the evidence threshold created migration 029, and the worktree is clean.
