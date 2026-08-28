# Curriculum Structure Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ambiguous curriculum requirements with catalog-owned metrics and dynamic curriculum term slots, then deliver comparison, document, and bulk draft-editing views.

**Architecture:** A forward migration adds immutable version-owned term slots, links both requirement tables to slots, adds explicit activity total hours, and removes requirement metric copies after preflight. Academic Core exposes one typed structure workspace and one atomic replacement command; Svelte maps that contract into comparison/document/edit views without request fan-out.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/serde/utoipa, generated OpenAPI TypeScript contracts, SvelteKit 5, TypeScript, Tailwind CSS, shadcn-svelte, Node static/runtime tests, Playwright

**Spec:** `docs/superpowers/specs/2026-08-28-curriculum-structure-and-homeroom-delivery-design.md`

## Global Constraints

- Read `.rules` before changes and never edit an applied migration.
- Use only the next sequential forward migration and run its blockers before destructive DDL.
- Published catalog and curriculum versions remain immutable.
- Catalog versions own official credit, weekly load, and total-hours metrics; requirements never override them.
- Curriculum term slots are version-owned and never reference an operational `academic_term_id`.
- There is no legacy API, fallback parser, dual-write path, or compatibility adapter after cutover.
- Rust DTOs and `utoipa` own the wire contract; regenerate and consume generated TypeScript DTOs.
- Use set-based service queries and typed JSON shapes; handlers remain thin.
- Use PageShell and local shadcn-svelte primitives; do not expose UUIDs or storage codes.
- Use the Svelte analyzer/autofixer for every analyzed or edited Svelte file.
- Run commands serially because the local environment hangs under concurrent Rust/frontend work.
- Never store or log plaintext national IDs or other unnecessary student PII.

---

### Task 1: Forward curriculum structure schema and migration guards

**Files:**
- Create: `backend-school/migrations/048_curriculum_structure_workspace.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Test: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Consumes: existing `curriculum_versions`, `study_programs`, `curriculum_course_requirements`, `curriculum_activity_requirements`, `subject_versions`, and `activity_versions` from migrations 041–045.
- Produces: `curriculum_term_slots`, `activity_versions.hours_per_term`, and `term_slot_id` foreign keys on both requirement tables; old requirement `credit`, `hours`, and `recommended_term_code` columns are removed.

- [ ] **Step 1: Add failing schema assertions**

Add a database test that applies the full migration set and asserts the clean contract:

```rust
assert!(column_exists(&pool, "curriculum_term_slots", "type_occurrence").await);
assert!(column_exists(&pool, "activity_versions", "hours_per_term").await);
assert!(column_exists(&pool, "curriculum_course_requirements", "term_slot_id").await);
assert!(!column_exists(&pool, "curriculum_course_requirements", "credit").await);
assert!(!column_exists(&pool, "curriculum_activity_requirements", "hours").await);
assert!(!column_exists(
    &pool,
    "curriculum_activity_requirements",
    "recommended_term_code",
)
.await);
```

Add fixture cases proving `TERM-1`, `TERM-2`, `SUMMER`, and `REMEDIAL` map deterministically and an unknown noncanonical term code aborts before destructive DDL.

- [ ] **Step 2: Run the focused schema test and confirm failure**

Run:

```bash
scripts/test_backend_school.sh cargo test core::schema_tests::curriculum_structure_contract -- --nocapture
```

Expected: FAIL because migration 048 and the new columns/tables do not exist.

- [ ] **Step 3: Implement the forward migration**

Begin the migration with read-only blocker checks. Recognize canonical mappings only:

```sql
CASE
    WHEN recommended_term_code ~ '^TERM-[1-9][0-9]*$' THEN 'regular'
    WHEN recommended_term_code = 'SUMMER' THEN 'summer'
    WHEN recommended_term_code = 'REMEDIAL' THEN 'remedial'
    ELSE NULL
END
```

Create slots with this shape:

```sql
CREATE TABLE curriculum_term_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    curriculum_version_id UUID NOT NULL
        REFERENCES curriculum_versions(id) ON DELETE RESTRICT,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    term_type TEXT NOT NULL
        CHECK (term_type IN ('regular', 'summer', 'remedial', 'custom')),
    type_occurrence INTEGER NOT NULL CHECK (type_occurrence > 0),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (curriculum_version_id, sequence),
    UNIQUE (curriculum_version_id, term_type, type_occurrence),
    UNIQUE (id, curriculum_version_id)
);
```

Attach the existing published-curriculum child immutability trigger. Populate deterministic slots and requirement FKs before dropping old columns. Add `activity_versions.hours_per_term NUMERIC(10,2)` nullable at the catalog level, with a nonnegative check; block cleanup when a referenced activity version lacks the value rather than multiplying by an assumed week count. Replace old uniqueness constraints with `(study_program_id, grade_level_id, term_slot_id, resource_version_id)`.

- [ ] **Step 4: Run schema and migration tests**

Run:

```bash
scripts/test_backend_school.sh cargo test core::schema_tests -- --nocapture
```

Expected: PASS, including “preflight failure leaves the old columns intact” and published-slot immutability.

- [ ] **Step 5: Commit the schema boundary**

```bash
git add backend-school/migrations/048_curriculum_structure_workspace.sql backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): normalize curriculum structure schema"
```

### Task 2: Typed catalog metrics, term slots, and curriculum workspace service

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services.rs`
- Modify: `backend-school/src/modules/academic/core/services/catalog.rs`
- Modify: `backend-school/src/modules/academic/core/services/curriculum.rs`
- Create: `backend-school/src/modules/academic/core/services/curriculum_structure.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Test: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: Task 1 schema.
- Produces: `CurriculumStructureWorkspace`, `CurriculumTermSlot`, typed `CatalogCurriculumMetrics`, and `ReplaceCurriculumStructureRequest`; `curriculum_structure::get_workspace` and `curriculum_structure::replace_program_structure` service functions.

- [ ] **Step 1: Write failing service tests for the read model**

Seed two study programs, two regular slots, a basic subject, an additional subject, and an activity. Assert one set-based workspace returns typed metrics and independent program totals:

```rust
assert_eq!(workspace.term_slots.len(), 2);
assert_eq!(workspace.programs.len(), 2);
assert_eq!(workspace.requirements[0].resource_kind, RequirementResourceKind::Course);
assert_eq!(workspace.requirements[0].metrics.total_hours, Some("60.00".into()));
assert_eq!(workspace.validation.blockers, Vec::<CurriculumValidationNotice>::new());
```

Add tests that a referenced activity with no `hours_per_term` creates a blocking notice, catalog classification controls the document section, and `RequirementKind` remains independent.

- [ ] **Step 2: Run the focused workspace tests and confirm failure**

```bash
scripts/test_backend_school.sh cargo test curriculum_structure_workspace -- --nocapture
```

Expected: FAIL because the types and service do not exist.

- [ ] **Step 3: Add exact typed domain contracts**

Define the core shapes with camelCase serialization and `ToSchema`:

```rust
pub struct CurriculumTermSlot {
    pub id: Uuid,
    pub curriculum_version_id: Uuid,
    pub sequence: i32,
    pub term_type: AcademicTermType,
    pub type_occurrence: i32,
    pub name: String,
    pub row_version: i64,
}

pub enum CatalogWeeklyUnit { Period, Hour }

pub enum CurriculumDocumentSection { BasicCourse, AdditionalCourse, StudentDevelopment }

pub struct CatalogCurriculumMetrics {
    pub weekly_value: String,
    pub weekly_unit: CatalogWeeklyUnit,
    pub credit: Option<String>,
    pub total_hours: Option<String>,
}

pub struct CurriculumStructureRequirement {
    pub id: Uuid,
    pub study_program_id: Uuid,
    pub grade_level: GradeLevelLookupItem,
    pub term_slot_id: Uuid,
    pub resource_kind: RequirementResourceKind,
    pub catalog_version_id: Uuid,
    pub code: String,
    pub name: String,
    pub section: CurriculumDocumentSection,
    pub requirement_kind: RequirementKind,
    pub metrics: CatalogCurriculumMetrics,
    pub display_order: i32,
}

pub struct CurriculumValidationNotice {
    pub code: String,
    pub message: String,
    pub catalog_version_id: Option<Uuid>,
}

pub struct CurriculumStructureValidation {
    pub blockers: Vec<CurriculumValidationNotice>,
    pub warnings: Vec<CurriculumValidationNotice>,
}

pub struct CurriculumStructureWorkspace {
    pub curriculum_version: CurriculumVersion,
    pub term_slots: Vec<CurriculumTermSlot>,
    pub programs: Vec<StudyProgram>,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub requirements: Vec<CurriculumStructureRequirement>,
    pub validation: CurriculumStructureValidation,
    pub row_version: i64,
}
```

`CurriculumDocumentSection` is derived from resource kind plus normalized official catalog classification. It is not accepted in mutations.

- [ ] **Step 4: Implement the set-based workspace query and totals helpers**

Use one query for slots and programs and one union query for course/activity requirements with their immutable catalog versions. Map database numeric values to normalized decimal strings. Add pure helpers for section ordering, per-section totals, per-slot totals, and blocking notices. Do not query once per program, slot, or row.

- [ ] **Step 5: Write failing atomic replacement and publish-validation tests**

Cover add, move, reorder, duplicate, copy payload normalization, stale row version, a catalog version outside its supported grade, missing activity total hours, and mutation of a published curriculum version.

```rust
assert!(matches!(stale, Err(AppError::Conflict(message)) if message.contains("เปลี่ยนแปลง")));
assert_eq!(reloaded.requirements, original_requirements);
```

- [ ] **Step 6: Replace the old requirement input with the clean mutation**

Use an atomic complete structure request:

```rust
pub struct CurriculumStructureRequirementInput {
    pub resource_kind: RequirementResourceKind,
    pub catalog_version_id: Uuid,
    pub grade_level_id: Uuid,
    pub term_slot_id: Uuid,
    pub requirement_kind: RequirementKind,
    pub display_order: i32,
}

pub struct ReplaceCurriculumStructureRequest {
    pub requirements: Vec<CurriculumStructureRequirementInput>,
    pub row_version: i64,
}
```

Normalize and deduplicate before validated bulk SQL. Lock the owning draft curriculum version, verify slot/program/catalog ownership in set-based queries, replace both requirement tables in one transaction, increment the owning version row version, and return the complete updated workspace. Update curriculum publication validation to reject structural blockers.

- [ ] **Step 7: Add explicit activity total hours to catalog create/update services**

Extend `ActivityVersion`, create, and update DTOs with `hours_per_term: Option<String>`. Validate nonnegative decimals and require a value before the version can participate in a published curriculum. Preserve published-version immutability.

- [ ] **Step 8: Run all focused core service tests**

```bash
scripts/test_backend_school.sh cargo test modules::academic::core::services_tests -- --nocapture
```

Expected: PASS with no row-by-row SQL path.

- [ ] **Step 9: Commit the service boundary**

```bash
git add backend-school/src/modules/academic/core
git commit -m "feat(academic): add curriculum structure workspace service"
```

### Task 3: HTTP and generated API contract cutover

**Files:**
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: Task 2 service functions and DTOs.
- Produces: typed `getCurriculumStructureWorkspace` and `replaceCurriculumStructure` OpenAPI operations and generated frontend types.

- [ ] **Step 1: Add failing route/contract assertions**

Assert the generated contract contains:

```text
GET /api/academic/curriculum-versions/{curriculumVersionId}/structure
PUT /api/academic/study-programs/{studyProgramId}/structure
```

and that `CurriculumStructureRequirementInput` contains `termSlotId` but not `credit`, `hours`, or `recommendedTermCode`.

- [ ] **Step 2: Run the focused static contract test and confirm failure**

```bash
cd frontend-school && node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: FAIL because the new operations are absent.

- [ ] **Step 3: Add thin handlers and OpenAPI registration**

Handlers must perform session tenant context, existing curriculum read/manage permission enforcement, service invocation, typed `ApiResponse`, and the existing academic invalidation signal after replace. Do not place SQL or grouping in handlers.

- [ ] **Step 4: Regenerate the API contract**

```bash
cd frontend-school && npm run generate:api-contracts
```

Expected: tracked OpenAPI and generated TypeScript DTO changes; no manual generated-file edits.

- [ ] **Step 5: Replace the frontend wrapper types**

Expose concrete generated operations:

```ts
export type CurriculumStructureWorkspace = Schemas['CurriculumStructureWorkspace'];
export type ReplaceCurriculumStructureRequest = Schemas['ReplaceCurriculumStructureRequest'];
```

Remove the old `ProgramRequirementInput` wrapper and any response cast or compatibility mapping.

- [ ] **Step 6: Run API checks and focused route tests**

```bash
cd frontend-school && npm run check:api-contracts
```

```bash
cd frontend-school && npm run test:api-contracts
```

```bash
cd frontend-school && node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: PASS.

- [ ] **Step 7: Commit the contract cutover**

```bash
git add backend-school/src/modules/academic/core.rs backend-school/src/modules/academic/core/handlers.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/generated frontend-school/src/lib/api/academic-core.ts frontend-school/tests/static
git commit -m "feat(academic): expose curriculum structure contract"
```

### Task 4: Curriculum structure view models and document components

**Files:**
- Create: `frontend-school/src/lib/academic/curriculum-structure.ts`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumStructureToolbar.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumProgramComparison.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumTermDocument.svelte`
- Create: `frontend-school/tests/runtime/curriculum-structure.test.ts`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: generated `CurriculumStructureWorkspace` from Task 3.
- Produces: `buildCurriculumDocument`, `buildProgramComparison`, and typed read-only presentation components.

- [ ] **Step 1: Write failing pure view-model tests**

Cover section order, Thai labels, decimal-safe totals, missing values, dynamic third/summer slot, shared/different comparison cells, and independent program structures.

```ts
assert.deepEqual(document.termPanels.map((panel) => panel.name), [
  'ภาคเรียนที่ 1',
  'ภาคเรียนที่ 2',
  'ภาคฤดูร้อน'
]);
assert.equal(document.termPanels[0].totalCredits, '11.00');
```

- [ ] **Step 2: Run the runtime test and confirm failure**

```bash
cd frontend-school && npx vitest run tests/runtime/curriculum-structure.test.ts
```

Expected: FAIL because the mapper does not exist.

- [ ] **Step 3: Implement decimal-safe typed view models**

Never sum decimals through binary floating-point. Convert normalized two-decimal strings to integer hundredths, sum, then format. Return explicit section, term-panel, total, and comparison-cell types; do not mutate the generated DTO.

- [ ] **Step 4: Run the view-model test**

```bash
cd frontend-school && npx vitest run tests/runtime/curriculum-structure.test.ts
```

Expected: PASS.

- [ ] **Step 5: Build the toolbar, comparison, and document components**

Use shadcn-svelte Select/Tabs/Table/Badge/Tooltip controls, SchoolOrbit semantic tokens, sticky table headers, tabular numerals, and horizontal overflow. The document renders API-provided term slots and catalog-derived sections only. On mobile, term panels stack in the same order.

- [ ] **Step 6: Run the Svelte analyzer/autofixer one file at a time**

Run the project Svelte analyzer for each created `.svelte` file, apply every reported fix, and rerun until it reports no issues.

- [ ] **Step 7: Run focused frontend tests**

```bash
cd frontend-school && node --test tests/static/academic-curriculum-workspace.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: PASS.

- [ ] **Step 8: Commit the read-only curriculum UI**

```bash
git add frontend-school/src/lib/academic/curriculum-structure.ts frontend-school/src/lib/components/academic-core frontend-school/tests/runtime/curriculum-structure.test.ts frontend-school/tests/static/academic-curriculum-workspace.test.mjs
git commit -m "feat(academic): add curriculum document views"
```

### Task 5: Bulk curriculum draft editor and route integration

**Files:**
- Create: `frontend-school/src/lib/components/academic-core/CurriculumStructureEditor.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumCatalogPicker.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumChangePreview.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte`
- Remove: `frontend-school/src/lib/components/academic-core/CurriculumProgramEditor.svelte`
- Create: `frontend-school/tests/e2e/curriculum-structure-workspace.spec.ts`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: Task 3 mutation wrapper and Task 4 view models/components.
- Produces: complete read/edit route with staged atomic saving and activity total-hours input.

- [ ] **Step 1: Add failing static and browser workflow assertions**

Cover read-only published behavior, switching all-program/single-program views, dynamic slots, multi-select add, move, copy, remove, undo, preview, stale-save preservation, and no editable metric fields in requirement rows.

- [ ] **Step 2: Run the focused static test and confirm failure**

```bash
cd frontend-school && node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: FAIL because the route still uses `CurriculumProgramEditor` and old requirement fields.

- [ ] **Step 3: Implement staged editor state**

Keep an immutable server snapshot plus an editable array of clean inputs:

```ts
type StagedRequirement = {
  resourceKind: 'course' | 'activity';
  catalogVersionId: string;
  gradeLevelId: string;
  termSlotId: string;
  requirementKind: 'required' | 'elective' | 'optional';
  displayOrder: number;
};
```

All add/copy/move/remove/reorder operations return a new array and push the previous array onto an undo stack. Preview derives a stable diff keyed by resource kind, catalog version, grade, and term slot. Save sends one request with the current workspace row version and patches the returned workspace.

- [ ] **Step 4: Build the catalog picker and change preview**

The picker loads only published catalog options after manage permission and edit mode are active. It supports search, grade/resource filters, checkboxes, select-all-visible, and clear. Metrics are read-only. Preview lists additions, removals, moves, copies, and conflicts before enabling save.

- [ ] **Step 5: Integrate the route and activity metric form**

Replace the nested card editor with toolbar, comparison/document views, and draft editor. Preserve PageShell ownership and route permissions. Extend the activity version create/edit form with `ชั่วโมงรวมต่อภาคเรียน`, using the same decimal constraints as the typed API.

- [ ] **Step 6: Remove the obsolete editor and old call sites**

Follow re-exports and references, then delete `CurriculumProgramEditor.svelte` only after `rg` shows no supported call site. Remove imports and old `replaceProgramRequirements` usage.

- [ ] **Step 7: Run the Svelte analyzer/autofixer serially**

Analyze every created or edited `.svelte` file one at a time and resolve all output before continuing.

- [ ] **Step 8: Run focused frontend verification**

```bash
cd frontend-school && node --test tests/static/academic-curriculum-workspace.test.mjs
```

```bash
cd frontend-school && npx vitest run tests/runtime/curriculum-structure.test.ts
```

```bash
cd frontend-school && npx playwright test tests/e2e/curriculum-structure-workspace.spec.ts --list
```

Expected: PASS or successful Playwright discovery without committed credentials.

- [ ] **Step 9: Commit the editable workspace**

```bash
git add frontend-school/src/routes/'(app)'/staff/academic/curricula frontend-school/src/routes/'(app)'/staff/academic/catalog/activities frontend-school/src/lib/components/academic-core frontend-school/tests
git commit -m "feat(academic): add bulk curriculum structure editor"
```

### Task 6: Release 1 verification and sandbox cutover readiness

**Files:**
- Modify only if a test reveals a root-cause defect in Release 1 files.

**Interfaces:**
- Consumes: Tasks 1–5.
- Produces: one verified Release 1 commit range ready for `sandbox` deployment.

- [ ] **Step 1: Run backend verification serially**

```bash
cd backend-school && cargo fmt --all -- --check
```

```bash
scripts/test_backend_school.sh cargo test --test static_architecture
```

```bash
cd backend-school && cargo check
```

- [ ] **Step 2: Run API contract verification serially**

```bash
cd frontend-school && npm run generate:api-contracts
```

```bash
cd frontend-school && npm run check:api-contracts
```

```bash
cd frontend-school && npm run test:api-contracts
```

- [ ] **Step 3: Run frontend verification serially**

```bash
cd frontend-school && npm run lint
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
cd frontend-school && npm run test:static
```

- [ ] **Step 4: Validate migration on the permitted disposable database path**

Use `scripts/test_backend_school.sh` for local isolated migration tests. Use the documented explicit disposable Neon branch gate before applying to `sandbox`; never use a pooler endpoint for migration compatibility.

- [ ] **Step 5: Review repository state**

```bash
git diff --check
```

```bash
git status --short
```

Inspect the complete Release 1 diff and confirm no generated artifact, migration, or call site is missing.

- [ ] **Step 6: Commit any verification-only fixes**

If verification exposes a defect, first add a failing regression test, make the smallest root-cause fix, rerun the owning task's focused checks, stage only those explicit test/implementation paths, and commit with `git commit -m "fix(academic): harden curriculum structure release"`. Do not create an empty commit when no fix is required.
