# Homeroom Delivery Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Learning Delivery homeroom-first and extend curriculum preparation to atomically create safe draft offerings and reviewed normal, combined, or split learning groups.

**Architecture:** A set-based backend workspace joins each homeroom's exact study-program requirements to existing offerings, groups, teachers, rosters, and timetable aggregates for the selected operational term. The existing source-hashed/idempotent curriculum offering preview/apply flow is extended with normalized group proposals and explicit generated-group provenance; Svelte renders homeroom and offering projections over the same records.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/serde/utoipa, generated OpenAPI TypeScript contracts, SvelteKit 5, TypeScript, Tailwind CSS, shadcn-svelte, Node static/runtime tests, Playwright

**Spec:** `docs/superpowers/specs/2026-08-28-curriculum-structure-and-homeroom-delivery-design.md`

## Global Constraints

- Release 1 `2026-08-28-curriculum-structure-workspace.md` must be complete and verified first.
- Read `.rules` before changes and never edit an applied migration.
- Keep catalog version, curriculum requirement, homeroom, offering, learning group, roster, and timetable as separate authoritative concepts.
- Delivery uses the Topbar-selected `academicYearId` and `academicTermId`; curriculum definition does not.
- Reuse the existing offering preview/apply source hash and idempotency path instead of creating a competing workflow.
- Never auto-assign students, teachers, preferred physical rooms, timetable slots, or roster publication.
- Required requirements may propose ordinary groups; elective/optional requirements default to deferred grouping.
- Never overwrite a manual or customized group.
- Use set-based service queries; no request-per-homeroom, offering, group, teacher, roster, or timetable pattern.
- Overview responses contain no student identities, national IDs, contact details, or roster membership.
- Rust DTOs and `utoipa` own the wire contract; regenerate and consume generated TypeScript DTOs.
- Use PageShell and local shadcn-svelte primitives, and analyze every edited Svelte file with project tooling.
- Run every command serially because concurrent Rust/frontend work hangs the local environment.

---

### Task 1: Generated-group provenance schema

**Files:**
- Create: `backend-school/migrations/049_homeroom_delivery_workspace.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Test: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Consumes: Release 1 migration 048 and existing `learning_groups`/`learning_group_homerooms` schema.
- Produces: explicit `learning_groups.generation_source` and `learning_groups.generation_key` used to reuse generated groups and protect manual groups.

- [ ] **Step 1: Write failing schema tests**

Assert the new columns and partial uniqueness contract:

```rust
assert!(column_exists(&pool, "learning_groups", "generation_source").await);
assert!(column_exists(&pool, "learning_groups", "generation_key").await);
assert!(index_exists(
    &pool,
    "learning_groups_curriculum_generation_key"
)
.await);
```

Add a database test proving two curriculum-generated groups with the same offering/generation key conflict while two manual groups with null keys remain valid.

- [ ] **Step 2: Run the focused test and confirm failure**

```bash
scripts/test_backend_school.sh cargo test homeroom_delivery_provenance_contract -- --nocapture
```

Expected: FAIL because migration 049 does not exist.

- [ ] **Step 3: Implement the forward migration**

Add the clean provenance contract:

```sql
ALTER TABLE learning_groups
    ADD COLUMN generation_source TEXT NOT NULL DEFAULT 'manual'
        CHECK (generation_source IN ('manual', 'curriculum_prepare')),
    ADD COLUMN generation_key TEXT,
    ADD CONSTRAINT learning_groups_generation_shape_check CHECK (
        (generation_source = 'manual' AND generation_key IS NULL)
        OR (generation_source = 'curriculum_prepare'
            AND generation_key IS NOT NULL
            AND btrim(generation_key) <> '')
    );

CREATE UNIQUE INDEX learning_groups_curriculum_generation_key
    ON learning_groups (academic_term_id, learning_offering_id, generation_key)
    WHERE generation_source = 'curriculum_prepare';
```

Existing rows become `manual` and are never treated as generated. Do not infer provenance from names, migration flags, or JSON.

- [ ] **Step 4: Run focused schema tests**

```bash
scripts/test_backend_school.sh cargo test core::schema_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit the provenance schema**

```bash
git add backend-school/migrations/049_homeroom_delivery_workspace.sql backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): track curriculum generated learning groups"
```

### Task 2: Set-based homeroom delivery read model

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Test: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: Release 1 term slots/requirements and Task 1 group provenance.
- Produces: `HomeroomDeliveryWorkspace` and `workspaces::get_homeroom_delivery_workspace(pool, academic_year_id, academic_term_id)`.

- [ ] **Step 1: Write failing workspace service tests**

Seed and assert these independent cases:

- an expected requirement with no offering;
- a draft offering with no group;
- one ordinary group linked to one homeroom;
- one group linked to two homerooms;
- two groups linked to one homeroom;
- missing/assigned primary teacher;
- draft/published roster aggregate;
- zero/some/complete timetable entries;
- elective requirement with no group;
- an offering or group not linked to any homeroom; and
- a homeroom whose curriculum term slot cannot resolve.

```rust
assert_eq!(room.items[0].group_mode, HomeroomGroupMode::Combined);
assert_eq!(room.items[0].groups[0].homeroom_ids.len(), 2);
assert_eq!(workspace.unlinked.len(), 1);
```

- [ ] **Step 2: Run the focused tests and confirm failure**

```bash
scripts/test_backend_school.sh cargo test homeroom_delivery_workspace -- --nocapture
```

Expected: FAIL because the read model does not exist.

- [ ] **Step 3: Define exact typed status contracts**

```rust
pub enum HomeroomOfferingState { Missing, Draft, Published, Closed }
pub enum HomeroomGroupMode { Missing, Normal, Combined, Split, Deferred }
pub enum HomeroomTeacherState { MissingPrimary, Assigned }
pub enum HomeroomTimetableState { Unscheduled, PartlyScheduled, Scheduled }

pub struct DeliveryPrerequisite {
    pub code: String,
    pub message: String,
    pub recovery_path: Option<String>,
}

pub struct UnlinkedDeliveryItem {
    pub offering_id: Uuid,
    pub group_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub reason: String,
}

pub struct HomeroomDeliveryGroupSummary {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub status: LearningOfferingStatus,
    pub roster_status: RosterStatus,
    pub homeroom_ids: Vec<Uuid>,
    pub homeroom_names: Vec<String>,
    pub primary_teacher_count: i64,
    pub timetable_entry_count: i64,
}

pub struct HomeroomDeliveryItem {
    pub requirement_id: Uuid,
    pub resource_kind: LearningOfferingKind,
    pub catalog_version_id: Uuid,
    pub code: String,
    pub name: String,
    pub requirement_kind: RequirementKind,
    pub offering_id: Option<Uuid>,
    pub offering_state: HomeroomOfferingState,
    pub group_mode: HomeroomGroupMode,
    pub teacher_state: HomeroomTeacherState,
    pub timetable_state: HomeroomTimetableState,
    pub groups: Vec<HomeroomDeliveryGroupSummary>,
}

pub struct HomeroomDeliveryRoom {
    pub homeroom: HomeroomLookupItem,
    pub grade_level: GradeLevelLookupItem,
    pub study_program: StudyProgramOption,
    pub expected_count: usize,
    pub ready_count: usize,
    pub items: Vec<HomeroomDeliveryItem>,
    pub blockers: Vec<DeliveryPrerequisite>,
}

pub struct HomeroomDeliveryWorkspace {
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub homerooms: Vec<HomeroomDeliveryRoom>,
    pub unlinked: Vec<UnlinkedDeliveryItem>,
}
```

- [ ] **Step 4: Implement set-based loading and pure assembly**

Resolve operational term occurrence with a window over terms of the same type. Load homerooms/programs, expected requirements, offerings/targets, groups/homeroom links, primary-teacher counts, roster statuses, and active timetable counts in bounded queries. Assemble by keyed maps in Rust; never perform a query inside a homeroom or item loop.

`ready_count` means an offering exists and at least one applicable group is linked; teacher, roster, and timetable remain separately visible and do not make the curriculum item disappear.

- [ ] **Step 5: Run focused delivery service tests**

```bash
scripts/test_backend_school.sh cargo test modules::academic::delivery::services_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit the read model**

```bash
git add backend-school/src/modules/academic/delivery/models.rs backend-school/src/modules/academic/delivery/services/workspaces.rs backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(academic): add homeroom delivery workspace service"
```

### Task 3: Extend curriculum preparation preview and apply

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Test: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: Task 1 provenance, Task 2 expected-room assembly, and existing `source_hash`/idempotency storage.
- Produces: extended `CurriculumOfferingPreview` and `apply_from_curriculum` capable of reviewed generated groups.

- [ ] **Step 1: Write failing preview tests**

Assert one offering proposal per `(academicTermId, resourceKind, catalogVersionId)` with accumulated targets. Required requirements propose one group per homeroom; elective and optional proposals contain no default groups and report `deferred`.

```rust
assert_eq!(preview.offerings.len(), 1);
assert_eq!(preview.offerings[0].default_groups.len(), 3);
assert!(elective.default_groups.is_empty());
assert_eq!(elective.grouping_state, PreparationGroupingState::Deferred);
```

- [ ] **Step 2: Write failing apply tests**

Cover normal groups, one combined group, two split groups, skip, deferred groups, existing compatible generated groups, existing manual group conflicts, stale source hash, idempotent retry, changed request under the same key, and transaction rollback.

- [ ] **Step 3: Run the focused preparation tests and confirm failure**

```bash
scripts/test_backend_school.sh cargo test curriculum_preparation_groups -- --nocapture
```

Expected: FAIL because preview/apply still handles offerings only.

- [ ] **Step 4: Define normalized preview/apply types**

```rust
pub enum PreparationAction { Apply, Skip, DeferGroups }
pub enum PreparationGroupingState { Proposed, Deferred, Conflict }

pub struct CurriculumGroupProposal {
    pub group_key: String,
    pub name: String,
    pub homeroom_ids: Vec<Uuid>,
}

pub struct PreparationConflict {
    pub code: String,
    pub message: String,
    pub offering_id: Option<Uuid>,
    pub group_id: Option<Uuid>,
}

pub struct CurriculumPreparationProposal {
    pub proposal_id: String,
    pub resource_kind: LearningOfferingKind,
    pub catalog_version_id: Uuid,
    pub requirement_ids: Vec<Uuid>,
    pub target_homeroom_ids: Vec<Uuid>,
    pub existing_offering_id: Option<Uuid>,
    pub grouping_state: PreparationGroupingState,
    pub default_groups: Vec<CurriculumGroupProposal>,
    pub conflicts: Vec<PreparationConflict>,
}

pub struct CurriculumPreparationChoice {
    pub proposal_id: String,
    pub action: PreparationAction,
    pub groups: Vec<CurriculumGroupProposal>,
}

pub struct ApplyCurriculumOfferingsRequest {
    pub academic_term_id: Uuid,
    pub source_hash: String,
    pub idempotency_key: Uuid,
    pub choices: Vec<CurriculumPreparationChoice>,
}
```

Proposal and group keys are stable hashes over normalized source identities and sorted homeroom IDs. User-entered names do not define identity.

- [ ] **Step 5: Implement normalized preview**

Reuse existing offering preview queries and term-slot resolution. Deduplicate offerings, sort/deduplicate IDs before hashing, and create ordinary default groups only for required requirements. Report manual/customized existing groups as explicit conflicts; never convert them to generated provenance.

- [ ] **Step 6: Implement atomic apply**

Recompute preview, compare `source_hash`, validate every choice against its proposal, and lock existing offerings/groups. Reuse or insert offerings/targets and `curriculum_prepare` groups by generation key. Replace homeroom coverage only for a generated group whose key matches the reviewed choice. Never alter teachers, preferred rooms, roster members/status, timetable entries, or manual groups.

Return typed counts and per-homeroom created/retained/skipped/conflict results. Store the complete normalized request hash in the existing idempotency record.

- [ ] **Step 7: Run focused preparation and delivery tests**

```bash
scripts/test_backend_school.sh cargo test curriculum_preparation_groups -- --nocapture
```

```bash
scripts/test_backend_school.sh cargo test modules::academic::delivery::services_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit preparation behavior**

```bash
git add backend-school/src/modules/academic/delivery
git commit -m "feat(academic): prepare offerings and learning groups"
```

### Task 4: Homeroom delivery and preparation API contracts

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic/delivery.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Modify: `frontend-school/tests/static/academic-workspace-request-count.test.mjs`

**Interfaces:**
- Consumes: Tasks 2–3 service functions and DTOs.
- Produces: `getHomeroomDeliveryWorkspace`, extended preview/apply operations, and concrete generated frontend wrappers.

- [ ] **Step 1: Add failing contract assertions**

Assert this read route exists and requires camelCase `academicYearId` and `academicTermId`:

```text
GET /api/academic/delivery/homerooms?academicYearId=...&academicTermId=...
```

Assert preview/apply schemas contain `defaultGroups`/`choices` and do not accept snake_case query aliases or legacy offering-only apply shapes.

- [ ] **Step 2: Run focused contract tests and confirm failure**

```bash
cd frontend-school && node --test tests/static/learning-delivery-workspace.test.mjs
```

Expected: FAIL.

- [ ] **Step 3: Add thin handlers and route registration**

Use the existing delivery read/manage permissions and resource policy. Return typed `ApiResponse<HomeroomDeliveryWorkspace>`. Keep the current offering overview endpoint for the secondary view. Emit the existing delivery invalidation signal only after successful apply.

- [ ] **Step 4: Regenerate and consume contracts**

```bash
cd frontend-school && npm run generate:api-contracts
```

Add concrete wrappers and remove old preview/apply casts or compatibility mappings.

- [ ] **Step 5: Run contract and request-count checks**

```bash
cd frontend-school && npm run check:api-contracts
```

```bash
cd frontend-school && npm run test:api-contracts
```

```bash
cd frontend-school && node --test tests/static/learning-delivery-workspace.test.mjs tests/static/academic-workspace-request-count.test.mjs
```

Expected: PASS and no per-row endpoint loop.

- [ ] **Step 6: Commit the delivery contracts**

```bash
git add backend-school/src/modules/academic/delivery.rs backend-school/src/modules/academic/delivery/handlers.rs backend-school/src/api_contract.rs contracts/openapi/school-api.json frontend-school/src/lib/api/generated frontend-school/src/lib/api/learning-delivery.ts frontend-school/tests/static
git commit -m "feat(academic): expose homeroom delivery contracts"
```

### Task 5: Homeroom-first delivery UI and reviewed preparation

**Files:**
- Create: `frontend-school/src/lib/academic/homeroom-delivery.ts`
- Create: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryToolbar.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryList.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryTable.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/UnlinkedDeliveryQueue.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts`
- Create: `frontend-school/tests/runtime/homeroom-delivery.test.ts`
- Create: `frontend-school/tests/e2e/homeroom-delivery-workspace.spec.ts`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`

**Interfaces:**
- Consumes: Task 4 generated workspaces and mutations.
- Produces: default by-homeroom route, secondary offering view, and preview editor for normal/combined/split/deferred choices.

- [ ] **Step 1: Write failing pure mapper tests**

Cover grade/program grouping, completeness counts, status priority, combined badges with shared group IDs, split labels, deferred elective rows, unlinked queue, and stable Thai copy.

```ts
assert.equal(room.summary, 'ครบ 11/13 รายการ');
assert.equal(combined.badge, 'เรียนรวม ม.1/1, ม.1/2');
assert.equal(split.groups.length, 2);
```

- [ ] **Step 2: Run runtime tests and confirm failure**

```bash
cd frontend-school && npx vitest run tests/runtime/homeroom-delivery.test.ts
```

Expected: FAIL because the mapper does not exist.

- [ ] **Step 3: Implement typed presentation helpers**

Map generated DTOs into grade/program sections without mutating them. Status priority is prerequisite blocker, missing offering, missing group, missing primary teacher, roster/timetable follow-up, then ready. Do not collapse separately actionable states into one boolean.

- [ ] **Step 4: Build homeroom list, table, and unlinked queue**

Use shadcn-svelte Tabs, Accordion/Collapsible, Table, Badge, Tooltip, Button, Alert, and skeleton primitives. Homeroom is the default tab. OfferingOverviewTable remains the secondary `ตามรายวิชา/กิจกรรม` tab. Row actions navigate to the existing offering/group detail route.

- [ ] **Step 5: Extend preparation preview UI**

Render room-by-room proposals. Staff can keep ordinary groups, select homerooms and combine them, split one homeroom into named groups, skip, or defer elective grouping. Changes remain local until apply; show source-stale and manual-group conflicts without discarding choices.

- [ ] **Step 6: Preserve page-state and Topbar boundaries**

Load one homeroom workspace request for the selected term and load the offering overview only when its tab is first opened. Abort stale requests on context changes. Distinguish missing context, empty term, missing curriculum/program dependency, permission denial, and request error through shared app-state components.

- [ ] **Step 7: Run the Svelte analyzer/autofixer serially**

Analyze every created/edited Svelte file one at a time and resolve all reported output.

- [ ] **Step 8: Run focused frontend tests**

```bash
cd frontend-school && npx vitest run tests/runtime/homeroom-delivery.test.ts
```

```bash
cd frontend-school && node --test tests/static/learning-delivery-workspace.test.mjs tests/static/academic-workspace-request-count.test.mjs
```

```bash
cd frontend-school && npx playwright test tests/e2e/homeroom-delivery-workspace.spec.ts --list
```

Expected: PASS or successful Playwright discovery without committed credentials.

- [ ] **Step 9: Commit the homeroom-first UI**

```bash
git add frontend-school/src/lib/academic/homeroom-delivery.ts frontend-school/src/lib/components/learning-delivery frontend-school/src/routes/'(app)'/staff/academic/delivery frontend-school/tests
git commit -m "feat(academic): add homeroom first delivery workspace"
```

### Task 6: Complete verification, sandbox smoke, and push

**Files:**
- Modify only when verification reveals a root-cause defect in the two approved releases.

**Interfaces:**
- Consumes: all Release 1 and Release 2 tasks.
- Produces: verified `main`, pushed for automatic deployment, with sandbox smoke evidence.

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

- [ ] **Step 4: Run migration compatibility on the documented disposable path**

Use isolated local tests first, then the explicit disposable Neon child-branch gate with a direct endpoint. Confirm migration 048 blockers and migration 049 constraints before touching `sandbox`.

- [ ] **Step 5: Review the final repository state**

```bash
git diff --check
```

```bash
git status --short --branch
```

Inspect all commits since the approved design and confirm only scoped files and generated artifacts changed.

- [ ] **Step 6: Push verified main**

```bash
git push origin main
```

- [ ] **Step 7: Run deployed sandbox smoke**

After automated deployment completes, run the documented `scripts/smoke_test.sh` with `SMOKE_*` credentials from the environment against `sandbox.schoolorbit.app`. Verify curriculum read/edit, homeroom view, offering view, preview/apply idempotency, and that no roster is published or timetable assigned by apply.

- [ ] **Step 8: Apply forward fixes only if smoke reveals a defect**

Diagnose with systematic debugging, add a failing regression test, implement the smallest root-cause fix, rerun the applicable serial matrix, commit, and push. Never edit migrations 048/049 after they have been applied; use the next sequential migration if a schema correction is required.
