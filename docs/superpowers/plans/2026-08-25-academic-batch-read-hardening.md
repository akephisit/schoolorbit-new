# Academic Batch Read Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Also use `superpowers:test-driven-development` for every behavior change, `svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices` for every Svelte edit, and `superpowers:verification-before-completion` before claiming a checkpoint or the release complete.

**Goal:** Remove every confirmed row-dependent HTTP/SQL read fan-out in the Academic Release 1 workspaces, supervision lists, timetable services, curriculum offering reads, and question-bank Word export while preserving authorization, ordering, realtime, and explicit academic-context behavior.

**Architecture:** Add narrow academic-context collection read models and POST batching only where ordered IDs can exceed URL limits. Backends select authorized parent rows first, load each child relation once with set-based queries, then assemble ordered DTOs in memory. Frontend route loaders consume generated contracts through a fixed number of cancellable requests; detail endpoints remain available for focused edits and mutations.

**Tech Stack:** Rust 2021, Axum, SQLx/PostgreSQL, utoipa/OpenAPI, TypeScript, Svelte 5/SvelteKit, Node test runner, repository contract generators.

**Spec:** `docs/superpowers/specs/2026-08-25-academic-batch-read-hardening-design.md`

## Global Constraints

- Run every command serially. Do not start background jobs, parallel test processes, or parallel agents; this workstation hangs under concurrent load.
- Read `.rules` again before implementation begins and use its change-type verification matrix.
- Do not edit applied migrations. This release must not add a migration, permission code, seed, role grant, or compatibility layer.
- Write the focused failing test first, run it and witness the intended failure, then write the minimum implementation.
- Use `apply_patch` for source edits. Generate generated files only through the repository generator.
- Rust/OpenAPI is the wire-contract owner. Query parameters must be generated camelCase fields; frontend wrappers must use generated operation/schema types.
- Preserve the current union of school, organization-unit, organization-tree, and assigned resource access. Never broaden a selected parent set while bulk-loading children.
- Never store, return, or log plaintext national IDs. Do not add request payload logging.
- Preserve stable response ordering, realtime event names/payloads, row versions, conflict semantics, and fail-closed behavior.
- Use `AbortController`: abort superseded loads and unmount cleanup, ignore only abort failures, and keep revision checks for stale non-fetch work.
- For every modified `.svelte` file, run the exact per-file `svelte-autofixer` command listed in its task before the focused frontend check.
- Make one focused commit per task. Do not push or deploy until the user explicitly approves it.

## File responsibility map

- `academic/delivery/services/groups.rs` owns authorized learning-group parent selection and the shared group bulk hydrator; route handlers only resolve actor access and serialize it.
- `academic/core/services/student_years.rs` owns year-scoped placements/advisors, while `academic/core/services/curriculum.rs` owns published study-program options and version-scoped program requirements.
- `academic/core/services/workspaces.rs` composes the context-free setup read model without embedding mutation behavior or bell-schedule periods.
- `academic/delivery/services/offerings.rs` owns preview preload maps and lightweight realtime signal descriptors; timetable services own timetable entry/relation indexes.
- `supervision/services/templates.rs` and `supervision/services/observations.rs` each own one set-based hydrator shared by list and detail reads.
- `question_bank/services.rs` owns ordered, authorized export-data assembly; file blob download remains in the existing file route.
- `backend-school/src/api_contract.rs` registers Rust handlers/DTOs as the only wire-contract source; `frontend-school/src/lib/api/generated/school-api.ts` changes only through `npm run generate:api-contracts`.
- `frontend-school/src/lib/async/latest-request.ts` owns request lifetime, and `frontend-school/src/lib/workspaces/academic-batch.ts` owns pure fixed-count composition; Svelte routes own only page state and cleanup.
- Static tests reject known fan-out shapes, runtime frontend tests verify request counts/cancellation, and database-backed Rust tests verify relation ownership and authorization behavior.

---

### Task 1: Establish the cancellation primitive

**Files:**

- Create: `frontend-school/src/lib/async/latest-request.ts`
- Create: `frontend-school/tests/runtime/latest-request.test.ts`

**Interfaces:**

- Consumes: browser-standard `AbortController`, `AbortSignal`, `DOMException`, and `Error`.
- Produces: `LatestRequest.begin(): { revision: number; signal: AbortSignal }`, `LatestRequest.isCurrent(revision: number): boolean`, `LatestRequest.abort(): void`, and `isAbortError(error: unknown): boolean` for Tasks 5, 6, 9, and 11.

- [ ] **Step 1: Add behavior tests for the request owner**

Create the Node test with these decisive cases:

```ts
import assert from "node:assert/strict";
import test from "node:test";
import {
  LatestRequest,
  isAbortError,
} from "../../src/lib/async/latest-request.ts";

test("begin aborts the prior request and advances the current revision", () => {
  const owner = new LatestRequest();
  const first = owner.begin();
  const second = owner.begin();
  assert.equal(first.signal.aborted, true);
  assert.equal(second.signal.aborted, false);
  assert.equal(owner.isCurrent(first.revision), false);
  assert.equal(owner.isCurrent(second.revision), true);
});

test("abort invalidates the active request and abort errors are narrowed", () => {
  const owner = new LatestRequest();
  const active = owner.begin();
  owner.abort();
  assert.equal(active.signal.aborted, true);
  assert.equal(owner.isCurrent(active.revision), false);
  assert.equal(isAbortError(new DOMException("aborted", "AbortError")), true);
  assert.equal(
    isAbortError(Object.assign(new Error("aborted"), { name: "AbortError" })),
    true,
  );
  assert.equal(isAbortError(new Error("network")), false);
});
```

- [ ] **Step 2: Run the test and witness failure**

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/latest-request.test.ts)
```

Expected: module-not-found for `src/lib/async/latest-request.ts`.

- [ ] **Step 3: Implement the request owner**

Keep this helper framework-independent. `begin()` must abort before replacing the controller. `isCurrent()` must require both the same revision and a non-aborted active signal. `isAbortError()` must narrow without casts by checking `error instanceof DOMException && error.name === 'AbortError'` and the portable `Error.name` equivalent.

```ts
export class LatestRequest {
  private controller: AbortController | undefined;
  private revision = 0;

  begin(): { revision: number; signal: AbortSignal } {
    this.controller?.abort();
    this.controller = new AbortController();
    this.revision += 1;
    return { revision: this.revision, signal: this.controller.signal };
  }

  isCurrent(revision: number): boolean {
    return (
      revision === this.revision && this.controller?.signal.aborted === false
    );
  }

  abort(): void {
    this.controller?.abort();
    this.controller = undefined;
  }
}

export function isAbortError(error: unknown): boolean {
  return (
    (error instanceof DOMException && error.name === "AbortError") ||
    (error instanceof Error && error.name === "AbortError")
  );
}
```

- [ ] **Step 4: Verify and commit**

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/latest-request.test.ts)
```

Expected: pass.

```bash
git diff --check
git add frontend-school/src/lib/async/latest-request.ts frontend-school/tests/runtime/latest-request.test.ts
git commit -m "feat: add cancellable request owner"
```

---

### Task 2: Add the term learning-group collection and bulk hydrator

**Files:**

- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic/delivery.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: existing `AcademicResourceListFilter`, `LearningOfferingQuery`, `LearningGroupRow`, `LearningGroup`, and offering list-access policy.
- Produces: `groups::list_for_term(pool, academic_term_id, access)`, shared `groups::hydrate_many(pool, rows)`, and OpenAPI operation `listLearningGroupsForTerm` consumed by Task 5.

- [ ] **Step 1: Add failing service and authorization tests**

Add fixtures with two accessible offerings and at least two groups. Give each group different teachers, homerooms, and preferred rooms. Test:

- `list_for_term` returns every relation under the correct group and preserves the list sort;
- school, organization-unit, organization-tree, and assigned-only actors receive the same union semantics as offering list access;
- a group attached only to an inaccessible offering is absent;
- an unknown or inaccessible term does not leak groups;
- existing `list(pool, offering_id)` still returns the same DTO through the shared bulk hydrator.

Add an OpenAPI test asserting `academicTermId` exists and `academic_term_id` does not exist for operation `listLearningGroupsForTerm`.

Add `learning_group_collection_hydrators_are_set_based` to `static_architecture.rs`. Normalize whitespace, reject a list loop that calls the single-row `hydrate`, and require `hydrate_many` in both list paths.

- [ ] **Step 2: Run the focused tests and witness failure**

```bash
./scripts/test_backend_school.sh modules::academic::delivery::services_tests::list_groups_for_term_preserves_access_union_and_relations -- --nocapture --test-threads=1
```

Expected: `list_for_term` is not defined.

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::academic_batch_read_queries_are_camel_case -- --exact --nocapture --test-threads=1
```

Expected: operation `listLearningGroupsForTerm` is absent.

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture learning_group_collection_hydrators_are_set_based -- --exact --test-threads=1
```

Expected: the current `groups::list` row loop violates the new guard.

- [ ] **Step 3: Implement one authorized parent selection and one query per child relation**

Add:

```rust
pub async fn list_for_term(
    pool: &PgPool,
    academic_term_id: Uuid,
    access: AcademicResourceListFilter,
) -> Result<Vec<LearningGroup>, AppError>;

async fn hydrate_many(
    pool: &PgPool,
    rows: Vec<LearningGroupRow>,
) -> Result<Vec<LearningGroup>, AppError>;
```

Select authorized group rows by joining learning offerings and applying the same filter-builder/policy as `offerings::list`. In `hydrate_many`, collect group IDs once, return immediately for an empty set, then issue exactly one ordered query each for teacher assignments, homeroom IDs, and preferred room IDs using `ANY($1)`. Group children by `learning_group_id` and assemble in the original row order.

Replace single-row `hydrate` with a one-element call to `hydrate_many` that fails if the selected row disappears. Change nested `list` to select its rows once and call `hydrate_many` once.

- [ ] **Step 4: Add the generated API boundary**

Add query DTO:

```rust
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningGroupTermQuery {
    pub academic_term_id: Uuid,
}
```

Add `GET /learning-groups`, operation ID `listLearningGroupsForTerm`, `ApiResponse<Vec<LearningGroup>>`, the existing offering-read capability, and the same resource-list filter resolution used by offerings. Register the handler and schemas in `api_contract.rs`.

- [ ] **Step 5: Verify the task and satisfy the group architecture guard**

```bash
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture learning_group_collection_hydrators_are_set_based -- --exact --test-threads=1
```

Expected: pass.

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/academic/delivery backend-school/src/modules/academic/delivery.rs backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "feat: batch learning groups by term"
```

---

### Task 3: Add year-scoped relationships and study-program options

**Files:**

- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/student_years.rs`
- Modify: `backend-school/src/modules/academic/core/services/curriculum.rs`
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**

- Consumes: existing academic-year, homeroom, student-year, curriculum access filters and published curriculum-version rules.
- Produces: DTOs `HomeroomAdvisorAssignment` and `StudyProgramOption`; operations `listPlacementsForAcademicYear`, `listHomeroomAdvisorsForAcademicYear`, and `listStudyProgramOptionsForAcademicYear` consumed by Task 5.

- [ ] **Step 1: Add failing multi-parent and isolation tests**

Cover the following exact boundaries:

- `list_placements_for_year` returns placements for multiple student-year records in the selected year and no placement from another year;
- `list_advisors_for_year` returns `id`, `homeroomId`, `userId`, `role` for multiple homerooms and no cross-year assignment;
- both endpoints reject an inaccessible year using the same read capability and resource policy as their parent collections;
- `list_study_program_options_for_year` returns only programs under published curriculum versions effective for that year, with `curriculumId` and `curriculumName`;
- unpublished, expired, future, and unauthorized curricula are excluded;
- school/unit/tree union access is preserved.

- [ ] **Step 2: Witness the missing services**

```bash
./scripts/test_backend_school.sh modules::academic::core::services_tests::year_relationship_collections_do_not_leak_across_years -- --nocapture --test-threads=1
```

Expected: the year-scoped service functions/types are missing.

- [ ] **Step 3: Implement the read models**

Add:

```rust
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomAdvisorAssignment {
    pub id: Uuid,
    pub homeroom_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgramOption {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub curriculum_id: Uuid,
    pub curriculum_name: String,
}
```

Add service functions taking the selected `academic_year_id` plus the exact existing parent resource filter. Query ownership through joins to `student_academic_years.academic_year_id`, `homerooms.academic_year_id`, and published/effective curriculum versions; do not filter children after fetching unrelated rows.

- [ ] **Step 4: Add routes and OpenAPI operations**

Register:

```text
GET /api/academic/placements?academicYearId=...
GET /api/academic/homeroom-advisors?academicYearId=...
GET /api/academic/study-program-options?academicYearId=...
```

Use one shared `AcademicYearQuery` with `rename_all = "camelCase"` and `deny_unknown_fields`. Operation IDs are `listPlacementsForAcademicYear`, `listHomeroomAdvisorsForAcademicYear`, and `listStudyProgramOptionsForAcademicYear`. Keep the nested detail/mutation routes.

- [ ] **Step 5: Verify and commit**

```bash
./scripts/test_backend_school.sh modules::academic::core::services_tests -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::academic_batch_read_queries_are_camel_case -- --exact --nocapture --test-threads=1
```

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/academic/core backend-school/src/modules/academic/core.rs backend-school/src/api_contract.rs
git commit -m "feat: add academic year collection reads"
```

---

### Task 4: Add curriculum-program and Academic Core setup workspaces

**Files:**

- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/curriculum.rs`
- Modify: `backend-school/src/modules/academic/core/services/years_terms.rs`
- Modify: `backend-school/src/modules/academic/core/services/bell_schedules.rs`
- Create: `backend-school/src/modules/academic/core/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/core/services.rs`
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**

- Consumes: `StudyProgram`, `ProgramRequirement`, `AcademicYear`, `AcademicTerm`, `BellSchedule`, and their existing read capabilities/order rules.
- Produces: `CurriculumProgramWorkspace`, `StudyProgramRequirement`, `AcademicSetupWorkspace`, `getCurriculumProgramWorkspace`, and `getAcademicSetupWorkspace` consumed by Task 5.

- [ ] **Step 1: Add failing workspace tests**

Test a curriculum version with two programs and course/activity requirements. Assert one response includes every program and tagged requirement, with deterministic program/requirement ordering and no requirement from another version.

Test setup data with two years, multiple terms, and bell schedules. Assert the response contains the same full rows and ordering as the existing list services and excludes bell-schedule periods. Test that a caller missing any required read capability receives forbidden rather than a partial workspace.

- [ ] **Step 2: Witness failure**

```bash
./scripts/test_backend_school.sh modules::academic::core::services_tests::workspace_reads_return_complete_ordered_collections -- --nocapture --test-threads=1
```

Expected: workspace types/functions do not exist.

- [ ] **Step 3: Implement DTOs and set-based services**

Add:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudyProgramRequirement {
    pub study_program_id: Uuid,
    #[serde(flatten)]
    pub requirement: ProgramRequirement,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumProgramWorkspace {
    pub programs: Vec<StudyProgram>,
    pub requirements: Vec<StudyProgramRequirement>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicSetupWorkspace {
    pub years: Vec<AcademicYear>,
    pub terms: Vec<AcademicTerm>,
    pub bell_schedules: Vec<BellSchedule>,
}
```

`program_workspace` validates/access-checks the version once, selects programs once, then selects all requirements by the program ID set. `setup_workspace` runs one full authorized query per collection and reuses existing ordering helpers. Do not loop through `list_terms` or `bell_schedules::list` by year.

- [ ] **Step 4: Add routes and contract registration**

Register operations:

```text
GET /api/academic/curriculum-versions/{id}/program-workspace
GET /api/academic/setup/workspace
```

Operation IDs are `getCurriculumProgramWorkspace` and `getAcademicSetupWorkspace`. Require the intersection of existing read capabilities for all included setup collections.

- [ ] **Step 5: Verify and commit**

```bash
./scripts/test_backend_school.sh modules::academic::core::services_tests -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests -- --nocapture --test-threads=1
```

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/academic/core backend-school/src/modules/academic/core.rs backend-school/src/api_contract.rs
git commit -m "feat: add academic workspace reads"
```

---

### Task 5: Generate academic contracts and add fixed-count frontend loaders

**Files:**

- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Create: `frontend-school/src/lib/workspaces/academic-batch.ts`
- Create: `frontend-school/tests/runtime/academic-batch-loader.test.ts`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`

**Interfaces:**

- Consumes: generated operations from Tasks 2–4 and `ApiRequestOptions` from the central API client.
- Produces: six typed API wrappers plus `loadTimetableCollections`, `loadStudentYearCollections`, and `loadHomeroomCollections`, all accepting one shared `AbortSignal`, consumed by Task 6.

- [ ] **Step 1: Add failing wrapper and loader tests**

The static contract tests must require generated operations and camelCase query objects for all seven new GET operations. Reject manual snake_case query keys.

Create dependency-injected pure loader functions so behavior can be tested without mounting Svelte:

```ts
export async function loadTimetableCollections(deps, termId, yearId, signal);
export async function loadStudentYearCollections(deps, yearId, signal);
export async function loadHomeroomCollections(deps, yearId, signal);
```

The fake dependencies record calls and supplied signals. For 300 offerings, `loadTimetableCollections` must call offerings once and `listLearningGroupsForTerm` once. Student-year and homeroom loaders must call each year-scoped relationship/option endpoint once regardless of parent count. All calls in one load receive the exact same signal.

- [ ] **Step 2: Witness failure**

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/academic-batch-loader.test.ts)
```

Expected: loader module is absent.

```bash
(cd frontend-school && node --test tests/static/api-query-contract.test.mjs tests/static/academic-core-cutover-contract.test.mjs)
```

Expected: new generated operations/wrappers are absent.

- [ ] **Step 3: Regenerate, then add typed wrappers**

```bash
(cd frontend-school && npm run generate:api-contracts)
```

Use generated operation aliases for parameters and response schemas. Add wrappers accepting `ApiRequestOptions`:

```ts
listLearningGroupsForTerm(academicTermId: string, options?: ApiRequestOptions)
listPlacementsForAcademicYear(academicYearId: string, options?: ApiRequestOptions)
listHomeroomAdvisorsForAcademicYear(academicYearId: string, options?: ApiRequestOptions)
listStudyProgramOptionsForAcademicYear(academicYearId: string, options?: ApiRequestOptions)
getCurriculumProgramWorkspace(versionId: string, options?: ApiRequestOptions)
getAcademicSetupWorkspace(options?: ApiRequestOptions)
```

Remove the nested-request implementation of `listStudyProgramOptionsForYear`; update callers to the new generated wrapper rather than retaining a compatibility alias.

- [ ] **Step 4: Implement pure fixed-count loaders**

Use `Promise.all` only for a fixed, route-defined set of independent calls. Never derive a promise list by mapping a server collection. Return normalized maps built in memory from the batch relationship arrays. Propagate `signal` through `{ signal }`.

- [ ] **Step 5: Verify and commit**

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/academic-batch-loader.test.ts)
```

```bash
(cd frontend-school && npm run check:api-contracts)
```

```bash
(cd frontend-school && node --test tests/static/api-query-contract.test.mjs tests/static/academic-core-cutover-contract.test.mjs)
```

```bash
git diff --check
git add frontend-school/src/lib/api frontend-school/src/lib/workspaces frontend-school/tests/runtime/academic-batch-loader.test.ts frontend-school/tests/static/api-query-contract.test.mjs frontend-school/tests/static/academic-core-cutover-contract.test.mjs
git commit -m "feat: add typed academic batch loaders"
```

---

### Task 6: Convert affected academic Svelte workspaces to cancellable fixed-count loads

**Files:**

- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/student-years/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/homerooms/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/curricula/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/core/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/admission/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/timetable-request-performance.test.mjs`
- Create: `frontend-school/tests/static/academic-workspace-request-count.test.mjs`

**Interfaces:**

- Consumes: `LatestRequest`/`isAbortError` from Task 1 and generated wrappers/pure loaders from Task 5.
- Produces: fixed-count, cancellable timetable, student-year, homeroom, curricula, Academic Core setup, and admission page behavior with unchanged UI/mutation boundaries.

- [ ] **Step 1: Add failing source-level route boundaries**

Require each page to own `LatestRequest`, pass `signal`, call `abort()` during `onMount` cleanup, and ignore `isAbortError`. Reject these patterns:

- `listLearningGroups(offering.id)` inside timetable load;
- `listHomeroomPlacements(record.id)` inside student-year load;
- `listHomeroomAdvisors(room.id)` inside homeroom load;
- `listProgramRequirements(program.id)` inside curricula load;
- loops over academic years that call `listAcademicTerms` or `listBellSchedules`;
- `listStudyProgramOptionsForYear` in any consumer.

Require the new batch function/endpoint in each matching route.

- [ ] **Step 2: Witness failure**

```bash
(cd frontend-school && node --test tests/static/timetable-request-performance.test.mjs tests/static/academic-workspace-request-count.test.mjs)
```

Expected: all current per-parent patterns are reported.

- [ ] **Step 3: Convert loaders without changing page behavior**

For each load:

1. call `const { revision, signal } = request.begin()`;
2. keep existing rendered data while loading a replacement;
3. call the fixed-count loader/wrapper with the shared signal;
4. commit state only if `request.isCurrent(revision)`;
5. silently return on `isAbortError(error)`;
6. preserve existing Thai error UI for non-abort errors;
7. return cleanup from `onMount` that calls `request.abort()`.

Build maps by `offeringId`, `studentAcademicYearId`, `homeroomId`, and `studyProgramId` in memory. Mutation refreshes must reuse the same cancellable load path. Admission must request program options once after the round's academic year is known.

- [ ] **Step 4: Run the Svelte analyzer serially for every edited component**

```bash
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/+page.svelte' --svelte-version 5)
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/student-years/+page.svelte' --svelte-version 5)
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/homerooms/+page.svelte' --svelte-version 5)
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/curricula/+page.svelte' --svelte-version 5)
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/core/+page.svelte' --svelte-version 5)
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/admission/[id]/+page.svelte' --svelte-version 5)
```

Apply valid findings with `apply_patch`, then rerun only the affected analyzer until it reports no actionable issue.

- [ ] **Step 5: Verify and commit**

```bash
(cd frontend-school && node --test tests/static/timetable-request-performance.test.mjs tests/static/academic-workspace-request-count.test.mjs)
```

```bash
(cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check)
```

```bash
git diff --check
git add 'frontend-school/src/routes/(app)/staff/academic' frontend-school/tests/static/timetable-request-performance.test.mjs frontend-school/tests/static/academic-workspace-request-count.test.mjs
git commit -m "fix: batch academic workspace requests"
```

---

### Task 7: Remove curriculum-offering and timetable service fan-out

**Files:**

- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable_templates.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: existing curriculum preview/apply requests, `LearningOffering`, realtime signal payloads, `TimetableEntry`, and transaction-lock conflict semantics.
- Produces: `LearningOfferingSignalDescriptor`, set-based offering preview/signal reads, bulk timetable result hydration, and indexed occupancy/validation internals; no frontend contract changes.

- [ ] **Step 1: Add failing equivalence tests**

Create multi-row fixtures and assert:

- curriculum preview produces the same create/retain/conflict actions for course and activity requirements while resolving existing offerings from one preloaded map;
- apply returns signal descriptors `(learning_offering_id, academic_term_id, row_version)` in result order without calling full `get` hydration per ID;
- timetable batch create, deactivate, and template application return fully hydrated entries in result order;
- occupancy reports the same instructor/homeroom conflicts for multiple entries;
- move validation returns identical conflicts across several candidate cells without slot-by-slot database reads;
- locked create/update conflict checks preserve current conflict and transaction-lock behavior.

- [ ] **Step 2: Add and witness static failures**

Extend the architecture test to reject loops containing `offerings::get`, `get_entry`, effective homeroom lookup, or instructor lookup when iterating result/entry/candidate collections.

```bash
./scripts/test_backend_school.sh modules::academic::delivery::services_tests::curriculum_preview_and_apply_use_bulk_reads -- --nocapture --test-threads=1
```

Expected: bulk descriptor/preload APIs are absent.

```bash
./scripts/test_backend_school.sh modules::academic::services::timetable_service_tests::batch_and_conflict_reads_preserve_results -- --nocapture --test-threads=1
```

Expected: current implementation hits the newly guarded per-row paths.

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture academic_delivery_and_timetable_collection_reads_are_set_based -- --exact --test-threads=1
```

Expected: the current offering/timetable result loops violate the new guard.

- [ ] **Step 3: Bulk-load offering state and signal descriptors**

Before iterating curriculum requirements, select all existing course/activity offerings for the term and candidate version IDs, keying by offering kind plus catalog-version ID. Make preview a pure lookup over this map.

Add:

```rust
pub struct LearningOfferingSignalDescriptor {
    pub learning_offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub row_version: i32,
}

pub async fn signal_descriptors(
    pool: &PgPool,
    offering_ids: &[Uuid],
) -> Result<Vec<LearningOfferingSignalDescriptor>, AppError>;
```

Fetch once with `ANY($1)`, restore requested order in memory, and fail closed if any committed result ID is missing. Emit the existing signal once per descriptor.

- [ ] **Step 4: Reuse timetable bulk hydration and relationship indexes**

Add an internal `get_entries(pool, ids)` that selects all result rows and passes them once to existing `hydrate_rows`. Batch create/deactivate/template application call it after writes.

For occupancy and validation, select the relevant term entries once, bulk-load effective homerooms and instructors for the entry/group ID sets, and build indexes by entry and `(day_of_week, bell_schedule_period_id)`. Candidate evaluation is in-memory. Keep the current row locks for mutation conflict checks, but hydrate the locked set once.

- [ ] **Step 5: Verify and commit**

```bash
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::academic::services::timetable_service_tests -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture academic_delivery_and_timetable_collection_reads_are_set_based -- --exact --test-threads=1
```

Expected: pass.

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/academic backend-school/tests/static_architecture.rs
git commit -m "perf: bulk academic delivery and timetable reads"
```

---

### Task 8: Bulk-hydrate supervision and register its complete OpenAPI contract

**Files:**

- Modify: `backend-school/src/modules/supervision/models.rs`
- Modify: `backend-school/src/modules/supervision/services/templates.rs`
- Modify: `backend-school/src/modules/supervision/services/observations.rs`
- Modify: `backend-school/src/modules/supervision/services_tests.rs`
- Modify: `backend-school/src/modules/supervision/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: existing supervision parent-row access selection, response redaction, template child rows, observation child rows, and every route already registered in `supervision::handlers::routes`.
- Produces: `hydrate_templates`, `hydrate_observations`, and generated OpenAPI operations/schemas for the complete supervision HTTP surface consumed by Task 9.

- [ ] **Step 1: Add failing multi-parent hydration tests**

For templates, create at least two templates with different sections, items, and workflow steps. For observations, create at least two authorized observations with different evaluators, actions, and ratings. Assert child ownership, ordering, rating average semantics, and unchanged redaction. Add a list-access case proving unauthorized observations are never included in the parent IDs supplied to child queries.

Add an API contract test with the exact route/operation inventory from `supervision::handlers::routes`: cycles, templates, observation list/request/detail/review/availability/timetable-options/evaluators/cancel/request update/delete/approve/return/evaluation submit/certify/approve/acknowledge, and both report routes. Assert every query property is camelCase.

Add `supervision_collection_hydrators_are_set_based` to `static_architecture.rs`. Normalize whitespace and reject template/observation list loops that call their single-row detail hydrators.

- [ ] **Step 2: Witness failure**

```bash
./scripts/test_backend_school.sh modules::supervision::services_tests::list_hydrators_preserve_multi_parent_relations -- --nocapture --test-threads=1
```

Expected: list services still call detail hydration per row or the new assertion/helper is absent.

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::supervision_routes_are_fully_documented -- --exact --nocapture --test-threads=1
```

Expected: supervision operations are absent from OpenAPI.

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture supervision_collection_hydrators_are_set_based -- --exact --test-threads=1
```

Expected: the current template and observation list loops violate the new guard.

- [ ] **Step 3: Implement template and observation bulk hydrators**

Add `hydrate_templates(pool, rows)` and `hydrate_observations(pool, rows)`. Each returns immediately for empty input, collects parent IDs, loads each relation in one ordered `ANY($1)` query, groups rows by parent ID, and assembles DTOs in parent-row order. Rating averages must be computed for all observation IDs in one grouped SQL query.

Make detail reads call the bulk helper with one row. Never call `get_template` or `observation_from_row` from a list loop.

- [ ] **Step 4: Register Rust-owned contracts**

Derive `ToSchema` for stable request/response DTOs and `IntoParams` for queries. Annotate every public handler in the route inventory with a unique operation ID, generated camelCase parameter definitions, success type, and existing error responses. Register all operations and schemas in `api_contract.rs`. Do not rename routes or add a compatibility response.

- [ ] **Step 5: Verify and commit**

```bash
./scripts/test_backend_school.sh modules::supervision::services_tests -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::supervision_routes_are_fully_documented -- --exact --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture supervision_collection_hydrators_are_set_based -- --exact --test-threads=1
```

Expected: pass; all confirmed list-hydrator patterns have been removed.

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/supervision backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "perf: bulk supervision reads and contracts"
```

---

### Task 9: Convert supervision frontend to generated cancellable contracts

**Files:**

- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/supervision.ts`
- Modify: `frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte`
- Create: `frontend-school/tests/static/supervision-api-contract.test.mjs`
- Modify: `frontend-school/tests/static/supervision-booking.test.mjs`
- Modify: `frontend-school/tests/static/supervision-rubric.test.mjs`

**Interfaces:**

- Consumes: generated supervision operations/schemas from Task 8, `ApiRequestOptions`, and `LatestRequest`/`isAbortError` from Task 1.
- Produces: a generated-type-only supervision API wrapper and cancellable `SupervisionWorkspace` reads while preserving existing booking/rubric UI behavior.

- [ ] **Step 1: Add failing contract-ownership tests**

Require `supervision.ts` to alias request/response types from generated `operations`/`components`, reject manual wire DTO interfaces/type literals for server resources, and require `ApiRequestOptions` propagation. Require the workspace to use `LatestRequest`, pass one signal through its load, abort superseded/unmounted loads, and suppress only abort errors.

- [ ] **Step 2: Witness failure**

```bash
(cd frontend-school && node --test tests/static/supervision-api-contract.test.mjs tests/static/supervision-booking.test.mjs tests/static/supervision-rubric.test.mjs)
```

Expected: generated supervision operations and cancellation ownership are absent.

- [ ] **Step 3: Regenerate and replace manual wire DTO ownership**

```bash
(cd frontend-school && npm run generate:api-contracts)
```

Use generated operation request bodies, query/path parameters, and schema aliases in every wrapper. Keep UI-only derived/view-state types local and clearly named. Add optional `ApiRequestOptions` to read wrappers and pass options to the central API client. Do not use `as any`, `unknown as`, or duplicate the server shapes.

- [ ] **Step 4: Add cancellation to the workspace**

Use one `LatestRequest` owner for each replaceable workspace read. Pass the shared signal to all fixed calls, retain revision protection, abort on cleanup, and preserve current user-visible errors for genuine failures. Do not change booking/rubric business behavior.

- [ ] **Step 5: Analyze, verify, and commit**

```bash
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer src/lib/components/supervision/SupervisionWorkspace.svelte --svelte-version 5)
```

```bash
(cd frontend-school && npm run check:api-contracts)
```

```bash
(cd frontend-school && node --test tests/static/supervision-api-contract.test.mjs tests/static/supervision-booking.test.mjs tests/static/supervision-rubric.test.mjs)
```

```bash
(cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check)
```

```bash
git diff --check
git add frontend-school/src/lib/api frontend-school/src/lib/components/supervision frontend-school/tests/static/supervision-api-contract.test.mjs frontend-school/tests/static/supervision-booking.test.mjs frontend-school/tests/static/supervision-rubric.test.mjs
git commit -m "refactor: use generated supervision contracts"
```

---

### Task 10: Add authorized question-bank export-data batching and contracts

**Files:**

- Modify: `backend-school/src/modules/question_bank/models.rs`
- Modify: `backend-school/src/modules/question_bank/services.rs`
- Create: `backend-school/src/modules/question_bank/services_tests.rs`
- Modify: `backend-school/src/modules/question_bank/handlers.rs`
- Modify: `backend-school/src/modules/question_bank.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**

- Consumes: `ActorContext`, existing question-bank access policy, `QuestionDetail`, choice rows, and authorized file metadata.
- Produces: `QuestionBankExportDataRequest`, `export_question_data(pool, actor, question_ids)`, OpenAPI operation `exportQuestionBankData`, and generated contracts for all existing question-bank routes consumed by Task 11.

- [ ] **Step 1: Add failing order, limit, and authorization tests**

Register `services_tests` under `question_bank.rs`. Create questions with choices and file metadata, request IDs in non-database order, and assert:

- response detail rows exactly follow request order;
- choice and file collections remain attached to their question;
- empty, duplicate, and over-200 ID requests fail validation;
- if any ID is missing or unauthorized, the entire request fails with the same non-enumerating error;
- no unauthorized ID can be inferred from response length/order;
- existing single-question detail behavior remains unchanged.

Add an OpenAPI test for all existing question-bank routes plus export-data, including generated camelCase `questionIds` and the `1..=200` validation boundary.

- [ ] **Step 2: Witness failure**

```bash
./scripts/test_backend_school.sh modules::question_bank::services_tests::export_data_is_ordered_bounded_and_fail_closed -- --nocapture --test-threads=1
```

Expected: test module/export service does not exist.

- [ ] **Step 3: Implement set-based export data**

Add:

```rust
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionBankExportDataRequest {
    pub question_ids: Vec<Uuid>,
}

pub async fn export_question_data(
    pool: &PgPool,
    actor: &ActorContext,
    question_ids: &[Uuid],
) -> Result<Vec<QuestionDetail>, AppError>;
```

Validate 1–200 unique IDs before SQL. Apply the existing resource policy to the complete ID set. Select summaries/details once, choices once, and authorized file metadata once; assemble by ID and restore request order. Do not fetch file blobs in this endpoint.

- [ ] **Step 4: Add route and all question-bank OpenAPI ownership**

Register `POST /questions/export-data` before `/{id}` matching and operation ID `exportQuestionBankData`. Annotate/register `/options`, question list/create/detail/update/delete, file retrieval, and export-data so generated types own the complete frontend wire surface. Preserve `/api/academic/question-bank` prefix.

- [ ] **Step 5: Verify and commit**

```bash
./scripts/test_backend_school.sh modules::question_bank -- --nocapture --test-threads=1
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests::question_bank_routes_are_fully_documented -- --exact --nocapture --test-threads=1
```

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
git diff --check
git add backend-school/src/modules/question_bank backend-school/src/modules/question_bank.rs backend-school/src/api_contract.rs
git commit -m "feat: batch question bank export data"
```

---

### Task 11: Convert question-bank export to one cancellable generated request

**Files:**

- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/questionBank.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/question-bank/+page.svelte`
- Modify: `frontend-school/tests/static/question-bank-workflow.test.mjs`
- Create: `frontend-school/tests/runtime/question-bank-export.test.ts`

**Interfaces:**

- Consumes: generated question-bank operations from Task 10, `ApiRequestOptions`, existing Word assembly/file blob download, and browser `AbortController`.
- Produces: `exportQuestionBankData(questionIds, options)` and one-request ordered export-data loading with cancellable page ownership.

- [ ] **Step 1: Add failing request-count and ordering tests**

Add a dependency-injected export loader test that passes 200 IDs, records one call to `exportQuestionBankData`, verifies the body preserves ID order, and verifies the same abort signal is supplied. Add a static test rejecting `getQuestionBankQuestion(questionIds[index])` and any loop/map that calls the detail wrapper for export.

- [ ] **Step 2: Witness failure**

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/question-bank-export.test.ts)
```

Expected: batch export wrapper/loader is absent.

```bash
(cd frontend-school && node --test tests/static/question-bank-workflow.test.mjs)
```

Expected: current per-question detail export is detected.

- [ ] **Step 3: Regenerate and implement generated wrappers**

```bash
(cd frontend-school && npm run generate:api-contracts)
```

Replace manual server DTOs in `questionBank.ts` with generated aliases for the registered question-bank operations. Add:

```ts
exportQuestionBankData(questionIds: string[], options?: ApiRequestOptions): Promise<QuestionDetail[]>;
```

The wrapper must send one POST body `{ questionIds }`. Retain file-content retrieval as a separate per-file operation because export-data contains metadata, not blobs.

- [ ] **Step 4: Convert the page workflow**

Use one request for selected question detail data. Own an export `AbortController`; abort it when a newer export starts, the export UI closes, or the component unmounts. Ignore abort errors and preserve the existing Thai message for real failures. Keep document order equal to selected ID order and keep current bounded file downloads.

- [ ] **Step 5: Analyze, verify, and commit**

```bash
(cd frontend-school && npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/question-bank/+page.svelte' --svelte-version 5)
```

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/question-bank-export.test.ts)
```

```bash
(cd frontend-school && node --test tests/static/question-bank-workflow.test.mjs)
```

```bash
(cd frontend-school && npm run check:api-contracts)
```

```bash
(cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check)
```

```bash
git diff --check
git add frontend-school/src/lib/api frontend-school/src/routes/'(app)'/staff/academic/question-bank frontend-school/tests/static/question-bank-workflow.test.mjs frontend-school/tests/runtime/question-bank-export.test.ts
git commit -m "fix: batch question bank export requests"
```

---

### Task 12: Run the Release 1.1 verification matrix and prepare deployment handoff

**Files:**

- Modify only if a test exposes a real defect in an already changed file.
- Review: `docs/superpowers/specs/2026-08-25-academic-batch-read-hardening-design.md`
- Review: `docs/TESTING.md`
- Review: `docs/OPERATIONS.md`

**Interfaces:**

- Consumes: every deliverable and focused test from Tasks 1–11 plus the `.rules` change-type matrix.
- Produces: a clean, reviewed local `main` commit series and an evidence-backed push/deploy approval request; it performs no push or deployment.

- [ ] **Step 1: Prove no confirmed frontend fan-out remains**

```bash
rg -n "listLearningGroups\(offering\.id\)|listHomeroomPlacements\(record\.id\)|listHomeroomAdvisors\(room\.id\)|listProgramRequirements\(program\.id\)|getQuestionBankQuestion\(questionIds\[" frontend-school/src
```

Expected: no matches.

```bash
rg -n "listStudyProgramOptionsForYear" frontend-school/src
```

Expected: no matches.

- [ ] **Step 2: Prove no confirmed backend row hydrator remains**

```bash
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture -- --test-threads=1
```

Expected: pass.

```bash
rg -n "for .*\{[[:space:]]*$|hydrate\(|get_template\(|observation_from_row\(|get_entry\(|offerings::get" backend-school/src/modules/academic backend-school/src/modules/supervision
```

Review every match manually; each remaining loop must be assembly/pure evaluation, a single-resource path, or an intentional independently audited mutation side effect.

- [ ] **Step 3: Run the backend matrix serially**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
```

```bash
cargo test --manifest-path backend-school/Cargo.toml api_contract::tests -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::academic -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::supervision -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::question_bank -- --nocapture --test-threads=1
```

```bash
cargo check --manifest-path backend-school/Cargo.toml
```

- [ ] **Step 4: Run the frontend matrix serially**

```bash
(cd frontend-school && npm run generate:api-contracts)
```

```bash
(cd frontend-school && npm run check:api-contracts)
```

```bash
(cd frontend-school && npm run test:api-contracts)
```

```bash
(cd frontend-school && npm run lint)
```

```bash
(cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check)
```

```bash
(cd frontend-school && npm run test:static)
```

```bash
(cd frontend-school && node --experimental-strip-types --test tests/runtime/latest-request.test.ts tests/runtime/academic-batch-loader.test.ts tests/runtime/question-bank-export.test.ts)
```

- [ ] **Step 5: Final review and handoff**

```bash
git diff --check
git status --short --branch
git log --oneline --decorate -15
```

Review the complete diff against every acceptance criterion in the approved spec. Confirm there is no migration, permission-contract, seed, plaintext national-ID, compatibility, unrelated formatting, or unrelated generated change.

If verification required a source correction, add a focused regression test, rerun its focused command plus the affected matrix section, then commit:

```bash
git add backend-school/src/modules/academic backend-school/src/modules/supervision backend-school/src/modules/question_bank backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs frontend-school/src/lib frontend-school/src/routes/'(app)'/staff/academic frontend-school/tests
git commit -m "fix: close academic batch read verification gaps"
```

Do not push. Report the commits, tests, residual risks, and expected production smoke checks, then request explicit approval to push `main`. After a later approved deployment, smoke-check readiness, authenticated timetable request count, context switching cancellation, student-year/homeroom request count, supervision lists, and a multi-question Word export.
