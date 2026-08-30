# Academic Curriculum Alignment Release 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show how each homeroom's term delivery differs from its immutable curriculum and provide an explicit, lossless handoff from an approved curriculum version to a future-effective draft.

**Architecture:** Extend the existing bounded homeroom-delivery read model instead of introducing a per-row endpoint. One request resolves an explicit or date-default timetable version, compares curriculum requirements with targeted offerings for every homeroom, and returns stable alignment states. Published curriculum versions remain immutable. A dedicated clone service copies term slots, study programs, and course/activity requirements into a new future-effective draft in one transaction; staff then edit and publish the draft through the existing curriculum workspace.

**Tech Stack:** Rust/Axum/SQLx, Utoipa OpenAPI, generated TypeScript contracts, SvelteKit 5, Tailwind CSS, local shadcn-svelte primitives, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-academic-operational-change-and-timetable-versioning-design.md`

## Global Constraints

- Run every command serially. Do not run Rust and frontend commands at the same time.
- Do not add or edit a database migration; Release 4 uses the existing versioned curriculum and delivery schema.
- Published curriculum versions and their children remain immutable.
- Alignment is a read model and never mutates curriculum or delivery.
- Use the selected timetable version's effective date and targets; do not infer an unknown term end date.
- An expected offering ending before the selected version's effective date is `ended_early`; one starting later does not satisfy the selected version.
- Alignment states may coexist, for example `ended_early` and `operational_periods_differ`.
- Extra offerings are reported per targeted homeroom/program rather than hidden in a global unmatched bucket.
- Clone only from a published source version, bind to its `rowVersion`, preserve all source catalog references and display order, and require explicit future academic-year effectiveness.
- Rust DTOs and Utoipa own the wire contract. Regenerate OpenAPI and generated TypeScript; never hand-edit generated artifacts.
- Preserve existing curriculum and delivery permissions and resource-policy filtering; add no broad permission.
- UI direction: a calm curriculum-audit document using existing SchoolOrbit blue/neutral tokens, compact status badges, tabular workload values, and direct recovery actions. Use local shadcn-svelte components and keep dense tables horizontally scrollable.

---

### Task 1: Set-based curriculum-versus-delivery alignment

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Extend `HomeroomDeliveryQuery` with optional `timetableVersionId`.
- Add stable `CurriculumDeliveryAlignmentState` values: `matches_curriculum`, `curriculum_requirement_not_offered`, `extra_offering`, `ended_early`, and `operational_periods_differ`.
- Add version/effectiveness context, alignment states on expected items, and per-room extra-offering rows to `HomeroomDeliveryWorkspace`.

- [ ] **Step 1: Write failing service tests**

Add focused database-backed tests proving one workspace response:

1. uses an explicitly selected timetable version and its weekly-period targets;
2. reports a matching required course;
3. reports a missing curriculum requirement;
4. reports an offering targeted to the homeroom but absent from the curriculum as extra;
5. reports an expected offering that ended before the selected version as ended early;
6. reports a target differing from the catalog standard without mutating either value; and
7. rejects a timetable version belonging to another term.

- [ ] **Step 2: Run each focused test and verify RED**

Use `CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh <exact-test> -- --exact --nocapture --test-threads=1` from the repository root, one test at a time.

- [ ] **Step 3: Implement the bounded alignment read model**

Resolve the explicit version when supplied; otherwise preserve the documented planning/active/historical resolution. Carry `effective_from` with the selected version. Extend the existing set queries with offering availability boundaries and compute alignment in memory from the bounded result sets. Prefer an offering available on the selected effective date; retain an ended expected offering only to report `ended_early`. Build extra rows by homeroom target coverage and keep the existing resource-scope filter authoritative.

- [ ] **Step 4: Run the focused service tests and verify GREEN**

Expected: all literal states, IDs, workload values, cross-term rejection, and source immutability assertions pass.

- [ ] **Step 5: Commit the alignment domain task**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/workspaces.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(academic): expose curriculum delivery alignment"
```

### Task 2: Transactional curriculum draft cloning

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/curriculum.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Add `CloneCurriculumVersionRequest` with `versionName`, `startAcademicYearId`, optional `endAcademicYearId`, optional `description`, and `sourceRowVersion`.
- Add `POST /api/academic/curriculum-versions/{id}/clone-draft` returning the created `CurriculumVersion`.

- [ ] **Step 1: Write failing clone service tests**

Create a published source with multiple term slots, programs, course requirements, and activity requirements. Assert cloning:

- requires the exact published source row version;
- rejects draft/archived sources;
- creates a draft under the same curriculum with the requested effectiveness;
- copies exact slot/program/requirement cardinalities and field values with new IDs;
- leaves every source row unchanged; and
- produces a workspace that can be edited through existing draft-only services.

- [ ] **Step 2: Run focused tests and verify RED**

Run the new service tests serially through `scripts/test_backend_school.sh` with one build job and one test thread.

- [ ] **Step 3: Implement clone transaction and authorized handler**

Validate requested academic years with the existing version validator. Lock and re-read the source, verify published status and `sourceRowVersion`, insert the draft, and copy term slots, programs, and requirements with deterministic source-to-target ID mappings inside one transaction. Reuse the existing curriculum manage resource policy in the handler. Register the route and OpenAPI operation.

- [ ] **Step 4: Run focused service, handler-contract, and policy tests**

Expected: cloning is atomic, stale-safe, scoped, and absent from denied callers.

- [ ] **Step 5: Commit the clone boundary**

```bash
git add backend-school/src/modules/academic/core/models.rs \
  backend-school/src/modules/academic/core/services/curriculum.rs \
  backend-school/src/modules/academic/core/services_tests.rs \
  backend-school/src/modules/academic/core/handlers.rs \
  backend-school/src/modules/academic/core.rs backend-school/src/api_contract.rs
git commit -m "feat(academic): clone published curriculum drafts"
```

### Task 3: Generated contracts and typed frontend boundaries

**Files:**
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Modify: API contract static tests as required by generated output

- [ ] **Step 1: Add failing contract assertions**

Require the optional camelCase timetable-version query, all five alignment enum literals, the clone request schema, and the clone operation/path.

- [ ] **Step 2: Regenerate and consume contracts**

Run `npm run generate:api-contracts` from `frontend-school`, then add wrappers whose query/body types are derived from generated operations. Do not add casts or handwritten wire DTOs.

- [ ] **Step 3: Run contract gates**

Run `npm run check:api-contracts` and `npm run test:api-contracts` serially.

- [ ] **Step 4: Commit contracts and wrappers**

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/academic-core.ts frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/tests/static
git commit -m "feat(academic): publish curriculum alignment contracts"
```

### Task 4: Delivery alignment and permanent-change handoff UI

**Files:**
- Modify: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/CurriculumVersionPanel.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.svelte`
- Modify/create: focused frontend static tests

- [ ] **Step 1: Write failing UI behavior tests**

Require Thai copy and accessible controls for all five states, per-room extra offerings, exact curriculum-context links, read-only context inspection, published-source clone action, explicit effective-year fields, stale/error recovery, and no management-options request for read-only users.

- [ ] **Step 2: Implement delivery alignment presentation**

Add an alignment column with concise badges and plain-language workload differences. Display extra offerings inside the affected homeroom rather than only in the global unlinked section. Add `ตรวจในหลักสูตร` links carrying `versionId`, `academicYearId`, `academicTermId`, `studyProgramId`, and selected `timetableVersionId`.

- [ ] **Step 3: Implement curriculum context panel and clone handoff**

When explicit delivery context exists in the URL, load the same set-based workspace once, filter it to the current curriculum/program, and show counts plus affected rows. The clone action is visible only for an authorized user viewing a published version. Its dialog explains that the published source remains unchanged, requires future effectiveness, calls the clone endpoint, selects the returned draft, and leaves editing/publication to the existing curriculum workflow.

- [ ] **Step 4: Run Svelte tooling and focused frontend tests**

Run the Svelte autofixer for every touched `.svelte` file, then the focused static test and `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`.

- [ ] **Step 5: Commit the Release 4 UI**

```bash
git add frontend-school/src/lib/components/academic-core \
  frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.svelte' \
  frontend-school/tests/static
git commit -m "feat(academic): add curriculum alignment handoff"
```

### Task 5: Browser workflow, full verification, and deployment

**Files:**
- Create: `frontend-school/tests/e2e/curriculum-delivery-alignment.spec.ts`
- Delete after completion: `docs/superpowers/plans/2026-08-30-academic-curriculum-alignment-release-4.md`

- [ ] **Step 1: Add focused Playwright workflows**

Cover alignment rendering and exact version context, read-only inspection, published-source cloning, stale clone recovery, selection of the created draft, source immutability in mocked requests, and absence of row-by-row API fan-out.

- [ ] **Step 2: Run focused and full verification serially**

Run focused Rust tests, Playwright discovery/execution, then the `.rules` matrix one command at a time:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cd frontend-school && npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
git diff --check
git status --short
```

- [ ] **Step 3: Review the final diff and retire the active plan**

Confirm no migration, permission, national-ID, untyped-contract, request-fan-out, or unrelated changes. Delete this completed active plan so Git history remains the record, as required by `.rules`.

- [ ] **Step 4: Commit, push, deploy, and verify sandbox**

Push `main`, watch permission/API/backend/frontend workflows serially, then run the focused authenticated or mocked sandbox Playwright workflow against `https://sandbox.schoolorbit.app`. Do not claim completion until deployment and the final workflow pass.
