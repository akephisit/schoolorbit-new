# Personnel Data Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Work inline in this session, as required by `.rules`; do not dispatch subagents without explicit user authorization.

**Goal:** ยกเลิกหน้ารายการกลาง ให้กรอกสาขาและสถาบันเอง ใช้ตำแหน่งที่ระบบดูแล และรักษาข้อมูลเดิมพร้อมความถูกต้องของการกรองและ Dashboard

**Architecture:** `school-staff` owns a read-only job-position catalog and person-owned education text. Two forward migrations expand/reconcile then contract under coordinated maintenance; typed HTTP contracts and all UI consumers switch together. Existing staff resource policies and scoped aggregate predicates remain authoritative.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, utoipa/OpenAPI generated TypeScript, SvelteKit 5, existing shadcn-svelte primitives, rootless Podman, Node 24, Playwright.

**Spec:** [Approved design](../specs/2026-10-03-personnel-data-simplification-design.md). Written-spec approval: user replied “โอเค” on 2026-10-03. Implementation awaits review of this plan; execution method is inline/native under the repository default.

## Global Constraints

- “ห้ามแก้ migration 081 หรือ 082 เพิ่ม migration ต่อเนื่องจาก timeline ณ ลงมือจริง โดยผ่าน tenant migration runner เดียวของระบบ”
- “เก็บข้อมูลพื้นฐานได้ทุกประเภทบุคลากร แต่การคำนวณระยะแรกมุ่งข้าราชการครูสายการสอนตามเกณฑ์ ก.ค.ศ.” This plan delivers only the first project; career history and eligibility calculations require their subsequent specs/plans.
- “รายการตำแหน่งมาตรฐานไม่ใช่เกณฑ์การเลื่อนวิทยฐานะ” No eligibility inference from names, permission roles, or generic employment type.
- “การส่ง patch ที่ไม่ระบุช่องคงค่าเดิม ส่วน explicit null ล้างช่องได้” Preserve all unrelated personnel/license fields.
- “ไม่เพิ่ม permission code ใหม่ในงานนี้” Use generated constants and existing profile resource policies.
- “รายงานเฉพาะ bounded finding codes และ aggregate counts ไม่ส่งชื่อจริงหรือ raw rows” Never print secrets, credentials, national IDs, draft content, or source rows.
- “หลัง schema/API cutover ห้ามนำ binary รุ่นที่ใช้ catalog เก่ากลับมา” No runtime legacy aliases, dual reads/writes, or generic catalog fallback after cutover.
- Reuse shared UI/theme owners; verify mobile/desktop, both themes and loading/error/empty/validation/pending states with synthetic fixtures.

## Review Focus

1. An inactive or nonstandard current position must survive an unrelated edit, but cannot be newly assigned to another person — Task 2 database tests and Task 3 browser tests.
2. A valid pre-release browser draft may contain reference UUIDs plus display names; preserve resolvable names, reject ambiguity visibly and never import another owner’s draft — Task 3 draft tests.
3. Reconciliation from version 81 is insufficient for this replacement; failed/missing/partial/stale new evidence must block cleanup and reopening — Tasks 1 and 4.
4. Legitimate personnel edits after completed cutover must not invalidate the historical audit by comparing current data to its frozen migration checksum — Task 1 status test.
5. Combined own/unit/tree scopes, missing-position buckets and old UUID drilldowns must still agree between counts and directory — Task 2 scope tests and Task 3 browser tests.

## File and ownership map

| Owner | Files | Responsibility |
|---|---|---|
| Forward migration | `backend-school/migrations/083_staff_personnel_simplification_expand.sql`, `084_staff_personnel_simplification_cleanup.sql` | Locked transformation, fresh reconciliation, canonical constraints and cleanup |
| Personnel domain | `backend-school/crates/school-staff/src/personnel.rs`, `models.rs`, `services.rs`, `services/staff_info_service.rs`, new `services/job_position_service.rs`, `services/staff_service.rs`, `services/personnel_overview_service.rs` | DTOs, validation, transactions, catalog reads and scoped joins |
| Migration evidence | Existing `services/personnel_cutover_service.rs`, new `services/personnel_simplification_tests.rs` in the same crate | Version-aware preflight/status and fixture tests |
| Application adapters | `backend-school/src/modules/staff/handlers/personnel.rs`, `src/app.rs`, `src/api_contract.rs`, `src/policies/staff_access_policy.rs`, `src/modules/system/services/personnel_migration_service.rs`, `src/modules/system/handlers/migration.rs` | Thin handlers, access checks, registration and all-tenant release status |
| Client/forms | `frontend-school/src/lib/api/personnel.ts`, `api/staff.ts`, `forms/staff-personnel.ts`, `forms/staff-create-draft.ts`, new `components/staff/StaffJobPositionPicker.svelte`, existing `StaffPersonnelFields.svelte` | Generated types, nullable patches, isolated drafts, lazy position selector and direct text inputs |
| Pages | `frontend-school/src/routes/(app)/staff/manage/+page.svelte`, `new/+page.svelte`, `[id]/edit/+page.svelte`, `[id]/+page.svelte` | Form consumers, summaries, directory filter and removal of catalog navigation |
| Retired owners | `services/reference_service.rs`, `StaffReferencePicker.svelte`, `staff/manage/reference-data/+page.svelte`, `+page.ts`, generic reference DTOs and registrations | Remove after replacements are wired; no legacy endpoints or routes |
| Release/tests | `.github/workflows/deploy-school-release.yml`, `.github/workflows/backend-school-neon-compatibility.yml`, existing focused tests plus new executable release-gate test | Require current evidence, provider rehearsal and end-to-end acceptance |

Before implementation, fetch `origin/main` and inspect divergence and the entire migration filename timeline. Continue on `codex/personnel-data-simplification`; if main advanced, merge it into this branch and revalidate the spec/plan. Numbers 083/084 assume current latest 082; renumber all new constants/tests/release checks together if an intervening migration arrives. Do not deploy intermediate checkpoint commits.

### Task 1: Preserve data and provide current migration evidence

**Files:** Create the two migrations and `services/personnel_simplification_tests.rs`; modify `services/personnel_cutover_service.rs`, `services.rs` and migration-specific fixtures in `services/personnel_tests.rs`; modify `backend-school/src/modules/system/services/personnel_migration_service.rs` and `handlers/migration.rs` tests.

**Interfaces:**
- Retain `read_personnel_preflight(&PgPool) -> Result<PersonnelPreflightReport, AppError>` and `read_personnel_cutover_audit(&PgPool) -> Result<PersonnelCutoverAudit, AppError>`; report actual SQLx history for preflight and applicable cutover version for status.
- Add `PERSONNEL_REFERENCE_MIGRATION_VERSION: i64 = 81`, `PERSONNEL_SIMPLIFICATION_EXPAND_VERSION: i64 = 83`; set current `PERSONNEL_MIGRATION_VERSION: i64 = 84`.
- Produce `staff_job_positions(id uuid PK, code varchar(64) UNIQUE, name varchar(200), is_active bool, is_selectable bool, display_order int, created_at timestamptz, updated_at timestamptz)`; preserve existing values/UUIDs. `is_selectable` is true only for an active member of the seven standard codes in the spec. Names/codes must be nonempty and control-free; do not retain dependencies on the generic catalog normalization function.
- Produce nullable `staff_info.major/university varchar(200)` and `staff_info_job_position_fkey`, preserve rank/degree constraints and existing unrelated staff fields.
- Produce `staff_personnel_simplification_audit` with one row for version 84: nonnegative staff/position/unused-education-option inventory counts, `passed`, `cutover_completed`, typed checks JSON, internal source/target fingerprints and `completed_at`. Exact required success codes: `PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED`, `PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED`, `PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED`, `PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED`. Never expose fingerprints or row values through status responses.
- Completed status appends exactly three live checks to those four stored checks: `PERSONNEL_SIMPLIFICATION_CANONICAL_SCHEMA_VALID`, `PERSONNEL_SIMPLIFICATION_RETIRED_OWNERS_REMOVED`, `PERSONNEL_MIGRATION_HISTORY_VALID`, each passed only when its violation count is zero. Schema checks include types, nullability, PK/FK and rank/degree constraints. The final report requires these seven distinct codes exactly once; stored checks are historical preservation evidence, live checks are current structure/history evidence.

- [ ] **Step 1: Write migration/evidence tests.** Reuse the existing `migrate_through(pool, version)` test pattern; new fixtures run to 82, insert synthetic staff and references, then run to 83 and 84 through the centralized migration source. Add tests named `personnel_simplification_preserves_reference_names_and_ids`, `personnel_simplification_empty_source_succeeds`, `personnel_simplification_blocks_missing_or_wrong_kind_reference`, `personnel_simplification_blocks_stale_or_partial_audit`, `personnel_simplification_failed_cleanup_retries_atomically`, and `personnel_simplification_completed_audit_survives_valid_edits`.
  - Assert exact equality of old/new person-owned names, nulls, UUIDs, position names/status/order/times and unrelated staff fields; use Thai and disabled references, shared names and a custom position.
  - For corrupt source fixtures, explicitly remove the source FK only inside the isolated test schema. Require migration failure and retained source columns/table, not silent repair.
  - After 83, alter a source row, a target row, remove an audit, duplicate/delete a required check, change its version, or set a negative/failed count: 84 must refuse cleanup. Correct the fixture and rerun the migrator; compare data and absence of partial cleanup.
  - After successful 84, make a legitimate canonical text edit: status remains passed while current schema/FK violations still fail. Frozen migration fingerprints are not an ongoing equality constraint.
  - Seed a fixture user with major `คณิตศาสตร์`, university `สถาบันทดสอบ` and a chosen original position UUID. In the preservation test, pin the post-84 assertion as follows; also compare the full before/after invariant snapshot described above:

```rust
let actual: (Option<uuid::Uuid>, Option<String>, Option<String>) = sqlx::query_as(
    "SELECT job_position_id, major, university FROM staff_info WHERE user_id = $1"
).bind(fixture_user).fetch_one(&pool).await.unwrap();
assert_eq!(actual, (Some(original_position), Some("คณิตศาสตร์".into()), Some("สถาบันทดสอบ".into())));
assert!(!read_personnel_cutover_audit(&pool).await.unwrap().checks.is_empty());
```
- [ ] **Step 2: Run red tests.** From root run `./scripts/test_backend_school.sh --package school-staff personnel_simplification -- --test-threads=1`; require a failure caused by the missing new migrations/owner, not missing PostgreSQL or a broken harness.
- [ ] **Step 3: Implement 083 expand and fresh reconciliation.** Lock source staff and reference tables; validate reference kind/presence and exact text limits before transforming. Copy every job-position row, including custom/unused entries, with stable UUIDs; backfill texts using the referenced names regardless of active status. Keep nulls and unrelated fields. Capture deterministic fingerprints of all source/target fields relevant to this transformation, ordered by stable IDs, and inventory unreferenced education options. Record the four complete passing checks with `cutover_completed=false`. Use built-in deterministic checksums only as drift evidence, with row comparisons proving preservation; no pgcrypto or encryption changes.
- [ ] **Step 4: Implement 084 cleanup.** Lock the same sources plus targets, require the version-84 audit with exactly the four distinct required passing checks, and recompute source/target comparisons and fingerprints before any destructive statement. Replace the position FK; remove all three generated kind columns, major/university reference FKs/UUID columns, generic catalog table and its normalization function; retain the completed audits. Set `cutover_completed=true` only after schema/invariant verification succeeds in this migration transaction. No `CASCADE` to hide unresolved dependencies.
- [ ] **Step 5: Update version-aware preflight/status.** Empty/version <81 retains the existing migration-input preflight; 81–82 verifies old audit/schema plus fresh simplification source checks; 83 verifies expanded evidence and live source/target consistency; >=84 verifies the completed new audit, exact check set, current canonical columns/constraints and absence of retired owners. SQL errors, unavailable pool, failed migration history and incomplete evidence fail closed. Do not run the old version-81 “text columns removed” check on the new canonical schema. Keep read-only direct-endpoint connections and `after_personnel_preflight` unchanged in responsibility.
- [ ] **Step 6: Run green migration and all-tenant boundary tests.** Run the Step 2 command plus `./scripts/test_backend_school.sh personnel_simplification -- --test-threads=1`. Name the new application status tests with this same prefix and cover 82/83/84 transitions, failed all-tenant preflight and refusal of old evidence for new completion. Historical 081 fixture tests remain explicitly pinned to their migration stage; do not delete their preservation assertions. The full existing runtime staff suite is run after Task 2 switches its consumers; do not claim the old runtime works on the contracted schema at this checkpoint.
- [ ] **Step 7: Check and checkpoint.** `git diff --check`; commit only this task’s files with `feat(staff): preserve personnel data through catalog simplification`. This checkpoint is not releasable with the old runtime consumers.

### Task 2: Switch the personnel owner and generated API

**Files:** Personnel domain/application adapter owners above; modify `services/personnel_tests.rs`, new catalog service inline tests, `backend-school/tests/static_architecture.rs`, `frontend-school/tests/static/personnel-contract.test.mjs`, and generated `contracts/openapi/school-api.json`, `frontend-school/src/lib/api/generated/school-api.ts` and generator-owned artifacts. Remove `services/reference_service.rs` and its re-export.

**Interfaces:**
- In `personnel.rs`, replace generic reference summaries with `StaffJobPositionSummary { id: Uuid, code: String, name: String, is_active: bool, is_selectable: bool }`, camelCase on the wire.
- Define `JobPositionListQuery { search: Option<String>, selectable_only: Option<bool>, page: Option<i64>, page_size: Option<i64> }` with camelCase query parameters and unknown-field rejection; `JobPositionPage { items: Vec<StaffJobPositionSummary>, total: i64, page: i64, page_size: i64 }`, camelCase response fields.
- Define `list_job_positions(pool: &PgPool, query: JobPositionListQuery) -> Result<JobPositionPage, AppError>` in `job_position_service.rs`; default page 1, pageSize 25, clamp page 1–1,000,000 and pageSize 1–50; default selectableOnly false, true filters both selectable and active. Return items/total in one snapshot; order display_order/name/id; no staff holder names/counts.
- Define `normalize_education_text(value: &str) -> Result<Option<String>, AppError>` in `staff_info_service.rs`: reject raw control characters, trim surrounding whitespace, map blank to null, then require <=200 Unicode scalar values. Preserve internal spaces and spelling.
- Create DTO retains `job_position_id`/rank/degree/license fields, replaces education reference IDs with `major: Option<String>`, `university: Option<String>`. Update DTO uses existing `Option<Option<T>>` omitted/null handling for the text fields. Responses expose `major/university: Option<String>` and the new position summary. Remove retired IDs/kind/CRUD schemas; reject old request fields rather than ignore them.
- Add GET operation `listStaffJobPositions` at `/api/staff/job-positions`; rename the coarse policy to `require_job_position_read(&ActorContext) -> Result<(), AppError>` with the same six generated capabilities as previous reference-read policy. No POST/PATCH/DELETE route for this catalog.

- [ ] **Step 1: Write behavior tests.** Test `normalize_education_text` with Thai, preserved internal double spaces, Unicode supplementary characters, 200/201 scalar values, empty/whitespace, embedded tab/newline/control characters. Test create/patch/read with omitted/null/text inputs and old unknown-ID fields. Database tests prove current custom/inactive assignment can remain, new assignment fails, valid selection can replace it, failed mutations preserve other fields, and choice changes are checked under the existing staff transaction lock.
  - Replace generic CRUD tests with searchable/bounded/read-only position tests, including empty out-of-range pages with correct total and filter mode selecting only valid new assignments.
  - Extend directory/overview tests to cover custom/inactive position UUIDs, `unspecified`, and own + unit/tree combinations with matching bucket/list counts.
  - Adapt runtime fixtures currently writing `major_id/university_id` to the new model while retaining the version-pinned migration tests from Task 1.
  - Pin the text/patch assertions in the same test owner:

```rust
assert_eq!(normalize_education_text("  คณิตศาสตร์  ประยุกต์  ").unwrap(), Some("คณิตศาสตร์  ประยุกต์".into()));
assert_eq!(normalize_education_text("   ").unwrap(), None);
assert!(normalize_education_text(&"ก".repeat(200)).is_ok());
assert!(normalize_education_text(&"ก".repeat(201)).is_err());
assert!(normalize_education_text("คณิตศาสตร์\n").is_err());
let omitted: UpdateStaffInfoRequest = serde_json::from_str("{}").unwrap();
let cleared: UpdateStaffInfoRequest = serde_json::from_str(r#"{"major":null}"#).unwrap();
assert_eq!(omitted.major, None);
assert_eq!(cleared.major, Some(None));
```
- [ ] **Step 2: Run red tests.** `./scripts/test_backend_school.sh --package school-staff -- --test-threads=1` and `./scripts/test_backend_school.sh policies::staff_access_policy::tests -- --test-threads=1`; observe the new contract/behavior failures.
- [ ] **Step 3: Implement domain DTOs and services.** Normalize text before writes; preserve nullable patch semantics and unrelated fields. Read profile text directly and positions from the new table. Change position joins and summary types in `staff_service.rs`, `models.rs`, and `personnel_overview_service.rs`; retain `staff_directory_query.rs` predicate ownership and URL tokens. Remove generic CRUD service/types/re-export after all backend callers switch.
- [ ] **Step 4: Register thin GET handler and remove old routes.** Update `personnel.rs` handler, `app.rs`, policy and OpenAPI paths/schemas. Add tests that an actor with each existing read/create/update capability can load choices, denied actors cannot, and catalog reads cannot authorize an out-of-scope profile. Guard against obsolete CRUD routes remaining registered.
- [ ] **Step 5: Regenerate and verify the wire contract.** From `frontend-school`, run `npm run generate:api-contracts`, `npm run check:api-contracts`, `npm run test:api-contracts`; update the focused contract test to assert person-owned text, unchanged job UUID filtering, nullable patch semantics and absence of reference CRUD. Do not hand-edit generated files. Client API exports must consume these generated types.
- [ ] **Step 6: Run green backend checks and checkpoint.** Repeat Step 2 and run `./scripts/test_backend_school.sh personnel_ -- --test-threads=1`, then from `backend-school` run `cargo fmt --all -- --check`, `cargo test --test static_architecture`, `cargo check --workspace --all-targets`. `git diff --check`; commit `feat(staff): expose canonical personnel text and read-only positions`.

### Task 3: Replace all form consumers and retire the management page

**Files:** Client/form/page owners above; remove `StaffReferencePicker.svelte` and both `reference-data` route files. Modify `frontend-school/tests/static/personnel-contract.test.mjs`, `staff-create-draft.test.mjs`, `tests/e2e/personnel-workflow.spec.ts`, `staff-directory-region-loading.spec.ts`, `personnel-live-acceptance.spec.ts` where fixtures/assertions depend on the retired page. Add focused draft-import coverage in the existing draft test owner.

**Interfaces:**
- API module exports generated `StaffJobPositionSummary`, `JobPositionListQuery`, `JobPositionPage`, and `listStaffJobPositions(query: JobPositionListQuery, options: ApiRequestOptions = {}): Promise<JobPositionPage>`.
- `StaffPersonnelDraft` picks `job_position_id`, `academic_rank`, `education_level`, `major`, `university` from generated update DTO. Retain `buildStaffPersonnelPatch(before, after)` and `staffPersonnelDraft(info)` signatures with new text fields and changed-field-only behavior.
- `StaffJobPositionPicker` props: `label: string`, bindable `value: string | null`, bindable `selected: StaffJobPositionSummary | null`, `selectableOnly: boolean = true`, `disabled: boolean = false`, `emptyLabel: string = 'ยังไม่ระบุ'`, optional `missingToken: string`. Directory filter passes selectableOnly false and preserves its missing token. Form selections remain authoritative UUIDs, not names.
- `StaffPersonnelFields` receives a draft and bindable selected position only; remove its generic `references` prop. Direct text Input bindings allow empty strings in the editable draft; normalize at patch/create preparation and recheck in the backend.
- Add `normalizeStaffEducationText(value: string | null | undefined): string | null` in `forms/staff-personnel.ts` with the Task 2 control/trim/blank/scalar-limit rules. Throw a validation Error for an invalid value; callers associate it with the field before sending and retain the entered draft. Count Unicode scalar values using `Array.from(value).length`; do not impose a conflicting UTF-16 maxlength or zod string-length limit on valid 200-scalar input.
- Draft writer uses v3 owner-qualified key/envelope, keeping the 30-minute lifetime and origin/user isolation. Import v2 once as migration input using exact matching reference UUID+summary name, then write v3 with the original expiry (not a renewed lifetime) and delete the original key on successful import. No API lookup or UUID-to-name guess. If a nonnull education ID lacks a matching summary, refuse import visibly and retain the original private key only until its existing expiry; never silently restore a partial draft with missing education data. Import is migration-only; no v2 writer, API fields or form model remains.
- The bounded v2 importer is owned only by the draft helper, for the initial frontend rollout. After both target frontends accept the canonical version and at least 30 minutes have elapsed since the last target cutover, remove the importer/v2 schema and obsolete-key quarantine, retain guards against old writes and rerun affected frontend verification. Task 5 includes this cleanup; it is not an indefinite compatibility path or a new draft archive.

- [ ] **Step 1: Write failing helper/draft tests.** Pin patches `{major:'คณิตศาสตร์'} -> {major:null}`, unchanged text -> no patch, and blank clearing without touching licenses/rank. Draft tests cover new fields, exact v2 label import, mismatched IDs, original expiry, malformed storage, different origin/user, stale draft and confirmation-free import of valid data. Return a typed draft-read result `{ draft: StaffCreateDraft | null, migrationFailed: boolean }` from `readStaffCreateDraft`; page shows “ร่างเดิมมีข้อมูลสาขาหรือสถาบันที่ตรวจสอบไม่ได้ กรุณาตรวจข้อมูลก่อนกรอกใหม่” when true, without displaying IDs or raw storage.

```javascript
assert.deepEqual(buildStaffPersonnelPatch({ major: 'คณิตศาสตร์' }, { major: null }), { major: null });
assert.equal(buildStaffPersonnelPatch({ university: 'สถาบันทดสอบ' }, { university: 'สถาบันทดสอบ' }), undefined);
assert.equal(normalizeStaffEducationText('   '), null);
assert.equal(normalizeStaffEducationText('ก'.repeat(200)), 'ก'.repeat(200));
assert.throws(() => normalizeStaffEducationText('ก'.repeat(201)));
const store = storage(); // existing owner-local in-memory test helper
saveStaffCreateDraft(store, owner, { personnel: { major: 'คณิตศาสตร์', university: 'สถาบันทดสอบ' } }, 100);
assert.equal(readStaffCreateDraft(store, owner, 101).draft.personnel.major, 'คณิตศาสตร์');
assert.equal(readStaffCreateDraft(store, { ...owner, origin: 'https://another.schoolorbit.invalid' }, 101).draft, null);
```
- [ ] **Step 2: Run red helper checks.** From frontend run `node --test tests/static/personnel-contract.test.mjs tests/static/staff-create-draft.test.mjs`; require failures for the new fields/read result, not tooling errors.
- [ ] **Step 3: Implement generated API wrapper, patch/draft helpers and picker.** Rename/remove generic wrapper exports, replace option component with position-only lazy reads and LatestRequest cancellation, typed selected state, bounded pagination/search, loading/error/retry and an associated accessible label. Unselectable current positions display from the profile; dropdown cannot newly assign them. Direct text fields do not fetch choices. Update v3 draft parsing/writing/clearing and its one-time import path.
- [ ] **Step 4: Switch create/edit/detail/directory.** Update initial drafts, draft restoration, review-step text, selected-position bindings, submit payload and profile text displays. Keep edits/drafts after failed save. Remove catalog links and route files/_meta; verify route scanner no longer discovers the retired route. Directory retains its new icon button for overview and its position filter UUID semantics. Do not change unrelated page loaders, permissions, organization membership editing or dashboard layout.
- [ ] **Step 5: Replace retired CRUD browser tests with new workflows.** Synthetic route fixtures serve `/api/staff/job-positions`; assert no request to reference-items and no catalog action. Cover create/edit text payloads and review display, current custom position, filter/chart UUID links, missing bucket, empty/failed/retried option loads, superseded search, permission-denied/read-only views, failed-save draft preservation, and removed route 404. Preserve existing useful directory/region/authorization and mobile chart coverage.
- [ ] **Step 6: Run green frontend checks.** Repeat Step 2; run `npm run lint`, `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`, `npm run test:static`, `npm run test:menu-sync`. Build with documented local fixture env and run preview on 127.0.0.1:4173; execute `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/personnel-workflow.spec.ts tests/e2e/staff-directory-region-loading.spec.ts --project=chromium`. Visually inspect synthetic 390px/1440px light/dark states, never capture real personnel screenshots.
- [ ] **Step 7: Review and checkpoint.** Review all caller changes, retired route discovery and permission gating; `git diff --check`; commit `feat(staff): simplify personnel forms and retire reference management`.

### Task 4: Require current evidence in the coordinated release

**Files:** `.github/workflows/deploy-school-release.yml`, `.github/workflows/backend-school-neon-compatibility.yml`, `frontend-school/tests/static/deployment-installer.test.mjs`, new `scripts/tests/personnel-cutover-gate.test.mjs`, new `scripts/verify_personnel_cutover.sh`, `docs/TESTING.md`, `docs/OPERATIONS.md`; retain the existing read-only `scripts/lib/schoolorbit-installer/remote/personnel_preflight.sh` unless integration proves a necessary change.

**Interfaces:** `scripts/verify_personnel_cutover.sh` is a sourced shell owner exposing `schoolorbit_personnel_cutover_filter()` which emits a jq filter fragment for the version-84, completed, nonempty/nonnegative checks requirement. The release’s existing jq container composes this with existing academic/gradebook/latest-version gates; do not require host jq or introduce a second deploy flow. Use this exact fragment in executable Node tests against sanitized jq fixtures. Update workflow file-transfer/source lists for the helper, without moving or replacing unrelated gates.

Test-only helpers in `scripts/tests/personnel-cutover-gate.test.mjs`: `acceptsPersonnelCutover(cutover: object): Promise<boolean>` wraps the supplied fixture as `.personnelCutover` and runs the actual fragment through native Podman with the existing `ghcr.io/jqlang/jq:1.7.1` image; `acceptsMigrationStatus(report: object): Promise<boolean>` runs the full composed release filter. Feed JSON on stdin and treat jq exit 0 as acceptance. `validCutoverFixture` has version 84, status cutoverCompleted, passed true, and the seven distinct Task 1 checks, with nonnegative counts and passing flags.

- [ ] **Step 1: Write release-gate failures.** Add executable tests for version 81/83 audit rejection, 84 success, missing audit/checks, duplicated or missing required code, negative/failed count, incomplete tenant coverage and unrelated pending/failed migration statuses. Test the filter fragment through native Podman jq as used by release; require the full gate to reject each bad fixture. Update static ownership tests to reflect the current audit version and helper.

```javascript
assert.equal(await acceptsPersonnelCutover(validCutoverFixture), true);
assert.equal(await acceptsPersonnelCutover({ ...validCutoverFixture, migrationVersion: 81 }), false);
assert.equal(await acceptsPersonnelCutover({ ...validCutoverFixture, migrationVersion: 83 }), false);
assert.equal(await acceptsPersonnelCutover({ ...validCutoverFixture, checks: [] }), false);
assert.equal(await acceptsPersonnelCutover({ ...validCutoverFixture, checks: validCutoverFixture.checks.slice(1) }), false);
assert.equal(await acceptsPersonnelCutover({ ...validCutoverFixture, status: 'cutoverPending' }), false);
```
- [ ] **Step 2: Run red gates.** `node --test scripts/tests/personnel-cutover-gate.test.mjs` from root; require wrong evidence to fail acceptance with the old version assumption before changing the gate.
- [ ] **Step 3: Implement current completion gate and provider rehearsal.** Require personnelCutover version 84, cutoverCompleted, passed true, exactly the seven distinct check codes defined by Task 1, all nonnegative/passing; preserve total/latest-version and academic/gradebook gates. Add `cargo test -p school-staff personnel_simplification -- --test-threads=1` and `cargo test personnel_ --bin backend-school -- --test-threads=1` to the manual Neon compatibility workflow’s existing fresh-child direct-endpoint test step. Maintain its extension provisioning, secret masking and cleanup/expiry ownership.
- [ ] **Step 4: Update canonical procedures.** Testing documents the new focused tests, rehearsal and acceptance commands. Operations records schema 82→83→84, protected pre-cutover snapshot, version-dependent preflight, all-tenant fresh evidence before reopening, retirement of CRUD, frozen historical audit versus live invariants, and prohibited old binaries/roll-forward recovery. Explain the draft import’s bounded owner/expiry and absence of runtime API fallback. No extra Markdown report.
- [ ] **Step 5: Verify workflow/topology matrix.** Run Step 2 green and `node --test frontend-school/tests/static/deployment-installer.test.mjs`. Run all installer/deployment checks in `.rules` section 11: shellcheck/shfmt, installer bats, native Podman Compose dry-run with the sanitized runtime fixture, and containerized actionlint. Include the new helper in shellcheck/shfmt. `git diff --check`; commit `fix(release): require current personnel simplification evidence`.

### Task 5: Verify the final tree, rehearse, integrate and accept

**Files:** Final diff across Tasks 1–4; no new owner or completion report.

**Interfaces:** One coherent releasable tree whose current migration version is 84, current personnelCutover reports version 84, and all product consumers use canonical positions/text. No individual checkpoint is a deploy target.

- [ ] **Step 1: Review spec coverage and final diff inline.** Verify each requirement and Review Focus has implemented coverage; inspect removed registrations and callers with rg. Version-pinned migration fixtures and draft import are explicit migration inputs, not runtime reference fallbacks. No broad API cache or permission widening. Keep career history/calculation outside this first change.
- [ ] **Step 2: Complete required local verification.** Run only affected focused checks not already valid for this exact final tree, then the backend, generated API, frontend, browser and workflow matrices listed above. Use verification-before-completion and report unavailable/failed checks as such. `git diff --check` and a clean status are required; do not advance on failures.
- [ ] **Step 3: Rehearse both new migrations on a disposable/protected provider copy.** Supply secrets privately, use the existing migration CLI/runner and direct non-pooled endpoint, compare bounded inventories and new audit against the locked source. Prove empty provisioning and source-version 82 transition; rehearse failed/stale evidence on disposable state. Dispatch the existing manual Neon compatibility workflow with the final candidate ref. Keep the protected pre-cutover snapshot; never modify live data to make a gate pass.
- [ ] **Step 4: Integrate under `.rules`.** Commit any verified final repairs; fetch origin/main, merge advances into this feature branch and rerun affected checks. Squash into up-to-date local main and compare feature/integrated tree IDs before reusing evidence; push origin/main normally. Retain the feature branch and recovery inputs through deployment acceptance.
- [ ] **Step 5: Verify the coordinated release.** Select backend/contracts/frontend together through the existing release workflow for the reviewed tree. Maintenance remains enabled on any preflight, migration, schema, audit or frontend failure. Require all tenants at the repository latest version with new completion evidence before reopening; verify the workflow result, service readiness, existing smoke script with private env and deployed read-only acceptance. Confirm both tenant UI routes work, old management route is retired, option loading and existing profile display use canonical data. Production acceptance has tracing/screenshots/video disabled; mutation acceptance remains on isolated synthetic tenants.
- [ ] **Step 6: Remove the bounded draft importer.** Track the latest actual frontend cutover time privately; wait until its 30-minute pre-release-draft lifetime has elapsed while continuing acceptance work and communicating progress. Delete the v2 importer/schema; v3 alone remains, and old keys are removed without being read as product drafts. Replace migration-input tests with canonical-draft and expired/obsolete-key rejection tests. This is a separate small cleanup change from current origin/main on a dedicated `codex/` branch; rerun affected frontend/contract/browser checks, squash/push under `.rules` and confirm the frontend cleanup deploy/readonly acceptance. Do not block a tool call for more than 60 seconds or claim this cleanup before it happens.
- [ ] **Step 7: Report the actual outcome.** Distinguish local tests, provider rehearsal, both deployment stages and live acceptance. State first-project delivery separately from future history/calculation work; retain artifacts/checkpoint branch until acceptance and the repository’s artifact-retirement condition are satisfied. Do not imply the date calculator was delivered.
