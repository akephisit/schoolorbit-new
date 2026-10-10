# Personnel Career History Implementation Plan

> **Execution:** Follow the current repository `.rules` and approved scope. Implement this plan task-by-task, run affected owned checks, and integrate through a verified PR. Steps use checkbox (`- [ ]`) syntax for tracking. Work inline in this session; do not delegate under the project’s default workflow.

**Goal:** ให้ฝ่ายบุคคลเก็บประเภทบุคลากร ประวัติตำแหน่ง/วิทยฐานะ วันที่มีผล และคำสั่งอ้างอิง พร้อมรักษาข้อมูลปัจจุบันและข้อมูลเดิมให้ถูกต้อง

**Architecture:** `school-staff` owns typed career records and their current projections through one transactional service. Current profile/directory/overview fields remain read projections; current-entry pointers identify the dated source. Root handlers enforce existing permissions/resource policy; frontend consumes generated contracts and renders history as an independent region.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, chrono calendar dates, existing workspace chrono-tz for Asia/Bangkok, utoipa/OpenAPI, SvelteKit 5/TypeScript, local UI primitives, Node tests and Playwright. No new dependency version or crate.

**Spec:** [Approved personnel career history design](../specs/2026-10-03-personnel-career-history-design.md). User approved the written design after its workflow explanation. This plan still requires its own review before product implementation.

## Global Constraints

- Work on `codex/personnel-career-history`, currently based on `origin/main` tree from `300615ae`; recheck divergence before execution/integration. Preserve the root checkout and existing recovery assets.
- Never edit applied migrations 001–084. Add migration 085 unless the remote timeline advances; use only the central tenant migration runner. No personnel migration service/endpoint/deploy hook.
- Personnel codes: `civil_servant`, `government_employee`, `contract_employee`, `permanent_employee`, `other`; null is unspecified. Do not infer these from employment_type, position labels, role or hired_date.
- Career kinds: `personnel_type`, `job_position`, `academic_rank`; preserve existing rank codes, including null versus `none` versus `not_applicable`, and stable position UUIDs.
- Dates are nullable calendar dates. Reject future career effective/order dates, allow an effective date before its order date, never fabricate dates from timestamps/hired_date, and display พ.ศ. without timezone day shifts.
- Order number ≤100 Unicode scalar values; note/correction reason ≤1,000. Trim outer whitespace, reject control characters, normalize whitespace-only text to null; correction reasons must be nonempty.
- Reads use existing staff-profile resource scopes; mutations require STAFF_CREATE_ALL/STAFF_UPDATE_ALL as appropriate. No new permission, public-profile field, PII storage/logging, external notification or realtime payload.
- History page default 20/max 50. Safe GET preload; separate loading/error/retry/mutation regions; no auth reload or workspace-wide invalidation.
- This plan delivers the history phase. Next-submission calculations and career dashboard date buckets require their own verified-criteria spec/plan after this delivery.

## Review Focus

1. An editor changes a rank but carries the previous rank’s dates: clear new-entry dates and preserve the old history (Tasks 3, 5, 6).
2. Another user promotes a person while a correction dialog is open: return 409 instead of silently correcting a now-historical entry as if current (Tasks 3, 6).
3. Retrying POST after a lost response: same entry ID/payload acknowledges once, changed payload conflicts, cross-person IDs disclose nothing (Tasks 3, 4).
4. Old v3 draft survives an upgrade: preserve other fields and original expiry, add no dates/type, remove old key only after successful migration (Task 5).
5. A malformed/foreign pagination cursor or one failed profile region: reject cursor locally and keep successful sibling regions visible (Tasks 3, 4, 6).

## File Ownership

- New `backend-school/crates/school-staff/src/career.rs`: wire DTOs/enums and pure validation. New `src/services/staff_career_service.rs`: current projections, chronology, pagination and transaction/audit owner. New `src/services/staff_career_tests.rs`: synthetic schema/domain fixtures; all under the existing crate. Its Cargo.toml opts into the existing `chrono-tz = { workspace = true }` dependency for calendar-day boundaries.
- New `backend-school/migrations/085_staff_career_history.sql`: table, current pointers, integrity constraints, backfill and reconciliation, executed by the existing migration runner.
- Existing crate `lib.rs`, `models.rs`, `personnel.rs`, `services.rs`, `services/staff_info_service.rs`, `services/staff_service.rs`: exports, typed career request integration and profile reads. Existing `services/personnel_tests.rs`, `services/status_tests.rs`: update current-runtime call sites/fixtures, preserve historical migration tests.
- New `backend-school/src/modules/staff/handlers/career.rs` and `career_integration_tests.rs`; existing `handlers.rs`, `modules/staff.rs`, `app.rs`, `api_contract.rs`, `profile_integration_tests.rs`: scoped HTTP registration and integration verification.
- Generated `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts` only through generator. New `frontend-school/src/lib/api/staff-career.ts` and `src/lib/forms/staff-career.ts`: typed HTTP boundary and explicit UI draft mapping.
- New `frontend-school/src/lib/components/staff/StaffCareerDates.svelte`, `StaffCareerHistory.svelte`, `StaffCareerEntryDialog.svelte`; existing `StaffPersonnelFields.svelte`, `forms/staff-personnel.ts`, `forms/staff-create-draft.ts`: shared inputs, career metadata and safe draft upgrade.
- Existing `frontend-school/src/routes/(app)/staff/manage/new/+page.svelte`, `manage/[id]/edit/+page.svelte`, `manage/[id]/+page.ts`, `manage/[id]/+page.svelte`, `staff/profile/+page.ts`, `staff/profile/+page.svelte`: create/edit/review/current summary/history region. Existing overview/directory API stays on current projection.
- New `frontend-school/tests/static/staff-career.test.mjs`, `tests/e2e/staff-career-workflow.spec.ts`; existing static/personnel/draft tests and e2e/personnel fixtures: contract and behavior coverage. `docs/TESTING.md` owns added test recipes; `docs/OPERATIONS.md` owns the new schema/binary recovery boundary.

---

### Task 1: Typed career contract and calendar validation

**Interfaces (produce in `career.rs`; Rust fields snake_case, new wire DTOs camelCase):**
- `StaffPersonnelType`, `StaffCareerKind`, `StaffCareerSource` enums with the exact codes above and sources `existing_record` / `staff_entry`.
- `StaffCareerFact` tagged enum `{ kind, value }`: each kind carries `Option<StaffPersonnelType>`, `Option<Uuid>`, or `Option<StaffAcademicRank>` respectively. Null is a clear/unspecified fact; unrelated typed payloads and unknown fields are rejected.
- `StaffCareerEntryInput { fact, effective_date: Option<NaiveDate>, order_date: Option<NaiveDate>, order_number: Option<String>, note: Option<String> }`.
- `StaffCareerReference { id: Uuid, revision: i64 }`; `StaffCareerCurrent { personnel_type: Option<StaffCareerEntry>, job_position: Option<StaffCareerEntry>, academic_rank: Option<StaffCareerEntry> }`.
- `StaffCareerEntry { id, staff_id: Uuid, fact, effective_date, order_date, order_number, note, source, revision: i64, is_current: bool, created_at, updated_at: DateTime<Utc> }`.
- `CreateStaffCareerRequest { entries: Vec<StaffCareerEntryInput> }`; `StaffCareerCurrentChange { expected_current: Option<StaffCareerReference>, entry: StaffCareerEntryInput, correction_reason: Option<String> }`; `UpdateStaffCareerRequest { changes: Vec<StaffCareerCurrentChange> }`. At most three distinct kinds; expectedCurrent is required on wire even when null.
- `CreateStaffCareerHistoryRequest { id: Uuid, entry: StaffCareerEntryInput }`; `CorrectStaffCareerHistoryRequest { expected_revision: i64, expected_is_current: bool, entry: StaffCareerEntryInput, reason: String }`. Correction cannot change kind or staff owner.
- `StaffCareerHistoryQuery { cursor: Option<Uuid>, page_size: Option<i64> }`; `StaffCareerHistoryPage { items: Vec<StaffCareerEntry>, current: StaffCareerCurrent, next_cursor: Option<Uuid> }`; `StaffCareerMutationAck { id: Uuid, revision: i64 }`.
- `normalize_career_entry(input: &StaffCareerEntryInput, today: NaiveDate) -> Result<StaffCareerEntryInput, AppError>`; `normalize_correction_reason(input: &str) -> Result<String, AppError>`; `validate_create_career(input: &CreateStaffCareerRequest, today: NaiveDate) -> Result<(), AppError>` and `validate_update_career(input: &UpdateStaffCareerRequest, today: NaiveDate) -> Result<(), AppError>` enforce distinct kinds/max three and normalized inputs.

- [ ] Add focused failing unit tests in `career.rs`: `career_date_and_text_validation`, `career_fact_preserves_null_and_rank_codes`, `career_rejects_duplicate_kinds_and_missing_expectation`. Assert 100/1,000 Thai/emoji scalars accepted, 101/1,001 rejected, control characters rejected, Feb 29 accepted on leap years, malformed calendar dates rejected, null distinct from none/not_applicable, future dates rejected, retroactive orders accepted. Missing expectedCurrent differs from explicit null.

  Representative assertions for the request contract test:
  ```rust
  assert!(serde_json::from_str::<UpdateStaffCareerRequest>(
      r#"{"changes":[{"entry":{"fact":{"kind":"academic_rank","value":"none"}}}]}"#
  ).is_err());
  assert!(serde_json::from_str::<UpdateStaffCareerRequest>(
      r#"{"changes":[{"expectedCurrent":null,"entry":{"fact":{"kind":"academic_rank","value":"none"}}}]}"#
  ).is_ok());
  ```
- [ ] Run `cargo test --manifest-path backend-school/Cargo.toml -p school-staff career::tests` and record the meaningful failing assertions before implementation.
- [ ] Implement DTOs/validation and module exports, deriving ToSchema/IntoParams as needed. Use explicit structs/tagged variants, not freeform JSON. Obtain today in Asia/Bangkok at the service boundary; inject today into pure validators.
- [ ] Rerun the focused command; require all selected tests pass, then commit only Task 1 files.

### Task 2: Schema, preservation and projection integrity

**Interfaces:** `staff_career_history` stores typed facts, date/order/note/source, actor UUIDs, timestamps and revision starting at 1. `staff_info` gains `personnel_type`, `current_personnel_type_history_id`, `current_job_position_history_id`, `current_academic_rank_history_id`. Current projection values remain in staff_info, always backed by matching pointers when non-null.

- [ ] Create failing `staff_career_schema_preserves_legacy_without_dates` and `staff_career_schema_rejects_inconsistent_projection` tests in `services/staff_career_tests.rs`, registered from `services.rs`. Reuse test-only `personnel_tests::migrate_through(&PgPool, i64)`. Seed schema 84 with active/custom/inactive positions, null, none/not_applicable, Thai education text, license data, metadata and stable timestamps. Assert exact old-field equality after 85, source existing_record, dates/orders/actors null, no inferred type, runner retry unchanged and empty provisioning succeeds.
- [ ] Run `./scripts/test_backend_school.sh --package school-staff staff_career_schema -- --test-threads=1`; require failure because the new migration/schema is absent.
- [ ] Implement migration 085 in a transaction: lock source; create table with kind/exclusive-payload/code/revision checks and FK to staff users/catalog/nullable actor; add current pointers; copy non-null position/rank facts; set pointers without altering old updated_at; reconcile exact values and unrelated field snapshots before success. Preserve null rows without fabricating history.
- [ ] Add deferrable FK/current consistency triggers on both staff_info and history so commit rejects missing/wrong-owner/wrong-kind/mismatched pointers, including raw writes by old binaries. Use row-local lookups, permit coordinated updates in one transaction, and support existing user-deletion cascade. Do not add another migration/audit runtime service.

  Assert preservation/retry against the same fixture snapshot:
  ```rust
  assert_eq!(after_original_fields, before_original_fields);
  assert!(imported_entries.iter().all(|entry| entry.effective_date.is_none()));
  assert!(imported_entries.iter().all(|entry| entry.order_date.is_none()));
  assert_eq!(after_retry_entries, imported_entries);
  assert!(inconsistent_transaction_commit.is_err());
  ```
- [ ] Add the bounded history index `(user_id, effective_date DESC NULLS LAST, created_at DESC, id DESC)` for the exact pagination predicate/order. Test invalid codes, missing pointers, mismatched values, cross-person pointers, current entry deletion, rollback and staff cascade. Inject a backfill failure and prove it rolls back schema/data; runner retry works after repair of the synthetic fixture.
- [ ] Rerun both focused schema tests and historical personnel migration fixtures. Require full pass and no changed applied SQL checksums; commit Task 2 files.

### Task 3: Transactional history owner and current CRUD

**Interfaces (produce in `staff_career_service.rs`):**
- `create_current_career(tx: &mut Transaction<'_, Postgres>, staff_id: Uuid, actor_id: Uuid, input: &CreateStaffCareerRequest) -> Result<(), AppError>`.
- `patch_current_career(tx: &mut Transaction<'_, Postgres>, staff_id: Uuid, actor_id: Uuid, input: &UpdateStaffCareerRequest) -> Result<(), AppError>`. Caller holds staff-user lock and initializes staff_info; no nested transaction.
- `list_staff_career_history(pool: &PgPool, staff_id: Uuid, query: StaffCareerHistoryQuery) -> Result<StaffCareerHistoryPage, AppError>`.
- `append_staff_career_history(pool: &PgPool, staff_id: Uuid, actor_id: Uuid, input: CreateStaffCareerHistoryRequest) -> Result<StaffCareerMutationAck, AppError>`.
- `correct_staff_career_history(pool: &PgPool, staff_id: Uuid, entry_id: Uuid, actor_id: Uuid, input: CorrectStaffCareerHistoryRequest) -> Result<StaffCareerMutationAck, AppError>`.
- Existing `create_staff(pool, payload, actor_id)` / `update_staff(pool, staff_id, payload, actor_id)` and staff_info create/patch receive actor UUID for transactional audit; update every verified call site.
- CreateStaffInfoRequest/UpdateStaffInfoRequest replace writable flat position/rank fields with optional typed `career`; retain education/major/university/license patch semantics. StaffInfoResponse retains current read fields and adds `current_career: StaffCareerCurrent`.

- [ ] Add domain DB tests `staff_career_current_and_history_commit_together`, `staff_career_correction_conflict_and_audit_failure`, `staff_career_retry_and_cursor_scope`, `staff_career_preserves_unrelated_profile_patch`. Assert rank none→proficient appends and changes projection, old dates remain, same value/metadata is no-op, clear creates null fact, metadata-only correction requires reason and increases revision, backdated append leaves current unchanged, future/current chronology conflicts fail, and inactive existing position metadata can be corrected but cannot be assigned to another person.
- [ ] Include concurrent updates with the same current reference: exactly one succeeds and the other gets 409. A correction carrying expectedIsCurrent=true after another promotion gets 409; history correction cannot change kind. Hiding/failing audit_logs rolls back records and projections. API retries of same ID/normalized input add only one audit; changed or foreign IDs conflict without returning foreign facts.

  Representative assertions after retry/rollback cases:
  ```rust
  assert_eq!(first_ack.id, retry_ack.id);
  assert_eq!(first_ack.revision, retry_ack.revision);
  assert_eq!(audit_count_after_retry, audit_count_after_first);
  assert_eq!(current_after_failed_audit, current_before_failed_audit);
  assert_eq!(concurrent_results.iter().filter(|result| result.is_ok()).count(), 1);
  ```
- [ ] Run `./scripts/test_backend_school.sh --package school-staff staff_career -- --test-threads=1` and confirm meaningful failures against missing service/owner behavior.
- [ ] Implement single owner using parent-row locks, expected reference/revision comparisons and existing `AuditLogBuilder::save_in_transaction`. Changed current value creates a new row; equal value with changed metadata corrects current row with reason; unchanged fields do nothing. Block known historical dates after known current dates on append/correction/current changes. Record audits only for actual changes, with typed values and actor UUID, no contact/identity expansion.
- [ ] Read current entry details through bounded joins in staff_info_service; route all writable career fields through this owner. Historical append/correction starts and owns its own transaction and staff lock; target must exist as staff. Preserve license/education semantics and no-op absent staff_info behavior.
- [ ] Implement stable pagination by effective_date DESC NULLS LAST, created_at DESC, id DESC using cursor anchor constrained to staff_id; missing/foreign cursor returns 400 without leaking another row. Fetch pageSize+1 and three current pointers in bounded set-based queries, never N+1. Inspect representative EXPLAIN on synthetic data before retaining the index.
- [ ] Update current-runtime personnel/status fixtures and call signatures; historical 81–84 audit tests remain pinned to their versions. Update profile integration fixtures to apply through 85 after their existing academic reconciliation fixture; do not globally change the shared academic “phase B” helper that intentionally ends at 46.
- [ ] Rerun `./scripts/test_backend_school.sh --package school-staff -- --test-threads=1` and focused root profile tests; require pass and commit Task 3 files.

### Task 4: Scoped HTTP and generated frontend boundary

**Interfaces:** New handler operations `listStaffCareerHistory`, `appendStaffCareerHistory`, `correctStaffCareerHistory` at the three routes in the spec. Produce frontend `listStaffCareerHistory(staffId, query?, options?)`, `appendStaffCareerHistory(staffId, input, options?)`, `correctStaffCareerHistory(staffId, entryId, input, options?)`, returning the generated page/ack DTOs. ApiRequestOptions.requestFetch and signal flow through.

- [ ] Add failing endpoint/policy tests in new root `career_integration_tests.rs`, using isolated current schema and synthetic actors: own/unit/tree/school and union allowed reads, out-of-scope/no-permission reads denied, missing/staff-versus-student targets, writer-without-profile-read acknowledgment only, read-only mutations denied, same/cross-person duplicate IDs and cursor errors bounded. Assert public-profile contains no career dates/orders/history.

  Assert HTTP envelopes and resource denial, rather than inspecting handler source:
  ```rust
  assert_eq!(own_read_status, StatusCode::OK);
  assert_eq!(outside_scope_status, StatusCode::FORBIDDEN);
  assert_eq!(reader_write_status, StatusCode::FORBIDDEN);
  assert!(writer_ack.get("current").is_none());
  assert!(writer_ack.get("items").is_none());
  ```
- [ ] Run `./scripts/test_backend_school.sh modules::staff::career_integration_tests -- --test-threads=1`; require failures for missing routes/enforcement, then implement thin handlers with AuthenticatedSession→actor tenant context→existing policy/permission→domain service→ApiResponse.
- [ ] Register handlers/modules/router and DTO/path schemas in api_contract.rs. Preserve update_staff file/cache side effects; pass authenticated actor UUID into domain create/update. Add 409 response to update/correction and document 400/401/403/404 consistently.
- [ ] Run `npm run generate:api-contracts` from frontend-school. Implement staff-career.ts aliases/wrappers using generated schemas/operations and requireApiData; remove retired flat writable fields from every consumer and fixture rather than accepting runtime aliases.
- [ ] Run endpoint tests, `cargo test --manifest-path backend-school/Cargo.toml api_contract::tests`, `npm run check:api-contracts`, `npm run test:api-contracts`; require pass and commit Task 4 files including generated artifacts.

### Task 5: Create/edit inputs and draft migration

**Interfaces (frontend forms/staff-career.ts):** `StaffCareerFactDraft<V> { value: V | null; effectiveDate: string; orderDate: string; orderNumber: string; note: string; reference: StaffCareerReference | null }`. `StaffCareerDraft { personnelType: StaffCareerFactDraft<StaffPersonnelType>; jobPosition: StaffCareerFactDraft<string>; academicRank: StaffCareerFactDraft<StaffAcademicRank> }`. `StaffCareerCorrectionReasons = Partial<Record<StaffCareerKind, string>>`.

Produce `staffCareerDraft(current: StaffCareerCurrent | null): StaffCareerDraft`, `buildCreateStaffCareer(draft: StaffCareerDraft): CreateStaffCareerRequest | undefined`, `buildUpdateStaffCareer(before: StaffCareerDraft, after: StaffCareerDraft, reasons: StaffCareerCorrectionReasons): UpdateStaffCareerRequest | undefined`, `resetCareerDetailsForChangedValue(before: StaffCareerDraft, after: StaffCareerDraft): StaffCareerDraft`, and `formatCareerDate(value: string | null): string`. Existing StaffPersonnelDraft owns education/text plus this career view model; `buildStaffPersonnelPatch(before, after, reasons = {})` composes career changes and untouched non-career patches.

- [ ] Add runnable failing Node tests in staff-career.test.mjs: serialized null/omitted facts, expectedCurrent=null versus reference, per-kind unchanged no-op, changed rank clears inherited dates, metadata-only update carries reason, calendar date displays 29 February in พ.ศ. in UTC and America/Los_Angeles subprocesses. Tests consume the real TypeScript form owner through the existing Node type-stripping pattern, not a copied implementation.
- [ ] Extend staff-create-draft.test.mjs for owner-qualified v4: v3 field values transformed to career draft, other fields retained, no dates/type inferred, original expiresAt unchanged, expired/corrupt/cross-owner input not restored; failed storage write keeps valid v3 input available and surfaces safe failure.

  Assertions for the date/draft tests:
  ```javascript
  assert.equal(buildUpdateStaffCareer(before, before, {}), undefined);
  assert.equal(changed.academicRank.effectiveDate, '');
  assert.equal(changed.jobPosition.effectiveDate, before.jobPosition.effectiveDate);
  assert.equal(migratedEnvelope.expiresAt, originalEnvelope.expiresAt);
  assert.equal(migrated.career.personnelType.value, null);
  ```
- [ ] Run `node --test tests/static/staff-career.test.mjs tests/static/staff-create-draft.test.mjs`; confirm failures, implement mappers/label records/date formatting and the v4 migration. Parse date components as a calendar date, use explicit UTC for formatting, and retain existing draft lifetime/cleanup boundaries.
- [ ] Implement StaffCareerDates.svelte shared labeled effective/order date, order number, note controls; extend StaffPersonnelFields with personnel-type Select and per-kind collapsible metadata. Changing a value clears only that kind’s details. Use existing DatePicker/Input/Button primitives, field-local errors, and correction reason when editing existing metadata.
- [ ] Wire new/edit payload, original references, validation and review summary. Preserve role/organization/file behavior; an education-only save sends no career changes. Keep entered dates/details after server errors or 409, with an explicit reload/reconcile action before resubmitting stale expected references.
- [ ] Run focused Node tests and personnel synthetic Playwright cases for create/review/edit/clear/draft reload with the updated generated payload. Use project Svelte checker on changed files. Commit Task 5 files after pass.

### Task 6: Current summaries and history region/dialog

**Interfaces:** `StaffCareerHistory.svelte` accepts `{staffId: string, initial: Promise<RouteLoadResult<StaffCareerHistoryPage | null>>, canEdit: boolean, onCurrentChanged: () => Promise<void>}`. `StaffCareerEntryDialog.svelte` accepts `{staffId, entry: StaffCareerEntry | null, open: boolean, onSaved: (ack: StaffCareerMutationAck) => void}`; entry=null means append historical record. Produce a profile loader history result named `careerHistory`, independent of staff/achievements/profile results, using existing captureRouteLoad and the same settled-user promise.

- [ ] Write failing synthetic browser tests in staff-career-workflow.spec.ts: current summary dates and พ.ศ.; unknown-date legacy badges; delayed/error history with successful profile visible; retry/load-more; historical append leaves current card; current correction updates only history/current profile region; 409 retains dialog and reason; duplicate submit disabled/retry keeps draft entry UUID; foreign-cursor error bounded; read-only/own access loads no action data.

  Pin the independent-region and failed-save behavior with browser assertions:
  ```typescript
  await expect(page.getByRole('region', { name: 'ประวัติตำแหน่งและวิทยฐานะ' })).toContainText('โหลดประวัติไม่สำเร็จ');
  await expect(page.getByRole('heading', { name: 'รายละเอียดบุคลากร', exact: true })).toBeVisible();
  await expect(page.getByLabel('เหตุผลการแก้ไข', { exact: true })).toHaveValue('แก้วันที่ตามคำสั่ง');
  expect(capturedCareerMutationIds[0]).toBe(capturedCareerMutationIds[1]);
  ```
- [ ] Use typed generated fixtures extending existing staff-directory-route-data.ts and self-profile-route-data.ts. Fix snapshots in existing personnel-workflow.spec.ts to use testInfo.outputPath rather than recreate the retired feature scratch directory. All browser mutations stay mocked/synthetic.
- [ ] Run `npx playwright test tests/e2e/staff-career-workflow.spec.ts --project=chromium --workers=1`; confirm failure before implementing components/loaders.
- [ ] Add GET history in both detail and own-profile loaders, concurrently after settled identity and exact read capability; own profile remains on its existing auth profile endpoint for basic data. No history request when permission is absent; no extra staff-profile read is required for own history because its response includes the three current entries.
- [ ] Implement current cards and timeline with type/current/unknown-date badges. Initial read is loader-owned; retry/page/dialog reads use cancellable/superseding interaction requests. On route change clear old-staff state before paint. On mutation reset the affected history page and reread only history/current profile as applicable; cancel old results and keep other regions. No invalidateAll/auth refresh.
- [ ] Implement append/correction dialog using typed fact selection, nullable dates, order/note/reason, expectedRevision/expectedIsCurrent, stable append UUID across retries. No delete control or next-submission date claim. Use icon Buttons with readable labels and shared app-state pending/error/success states.
- [ ] Rerun career/personnel browser tests, plus relevant profile/directory region-loading tests. Inspect sanitized screenshots at 390×844 and 1440×1000, light/dark, initial loading, background refresh, empty/error/validation/409 and keyboard focus. View screenshots rather than infer visual quality from passing assertions. Commit Task 6 files after pass.

### Task 7: Full verification, provider rehearsal and release acceptance

**Interfaces:** Existing migration runner, generated contracts, CI/full release and recovery procedures; no new personnel operation endpoint. Final evidence belongs to the exact tested tree.

- [ ] Update docs/TESTING.md with career schema/domain/HTTP/browser test commands and docs/OPERATIONS.md with the 85 current-projection invariant and old-binary prohibition. Keep approved workflow artifacts as inputs; do not create a completion report. Review final diff, migration checksums and absence of retired writable fields/temporary migration owners.
- [ ] Run backend required matrix from backend-school: `cargo fmt --all -- --check`, `cargo test --test static_architecture`, `cargo check --workspace --all-targets`; run `./scripts/test_backend_school.sh --package school-staff -- --test-threads=1`, root `modules::staff` and `policies::staff_access_policy` DB selections through the root script. Require nonzero selected-test counts and all passes.
- [ ] Run frontend required matrix from frontend-school: `npm run lint`, `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`, `npm run test:static`, `npm run check:api-contracts`, `npm run test:api-contracts`. Run full focused personnel/career browser workflows on the candidate frontend; no live mutation for browser acceptance.
- [ ] Before promotion preserve a current provider recovery point ≥7 days using the protected snapshot/quota fallback in Operations. Keep all earlier snapshots/branches/encrypted archives intact; keep new secrets/private metadata outside repo with restrictive permissions and verify any archive decryption/pg_restore. Rehearse the central migration CLI on disposable copies of each affected active tenant, reading only bounded versions/counts/preservation/integrity results. Never manually execute individual SQL on production.
- [ ] Run `git diff --check`, inspect clean committed feature tree, fetch origin/main and resolve any advance on the feature branch. Rerun affected checks only if tested tree/environment changes. Squash integrate into updated main, confirm matching tree IDs, then normal push according to .rules.
- [ ] Verify all required CI and the coordinated full release: backend/frontend artifacts match accepted main, all active tenants migrated to latest actual SQLx version, maintenance/readiness/smoke gates pass. Run canonical `bash scripts/smoke_test.sh` with its own private config loader; do not reinterpret shell credentials through another parser.
- [ ] Run read-only live personnel acceptance for each active tenant: overview→filtered directory→detail/history/current facts; own history when its exact read permission exists. Verify deployed old records still show unknown dates and no formula/eligibility claim. Preserve recovery assets/feature branch until acceptance passes; report any unrun checks or remaining work truthfully.

## Plan Self-Review and Handoff

All design sections map to Tasks 1–7: typed dates/personnel values (1), legacy preservation/integrity (2), history/corrections/current owner/audit (3), scopes/generated API (4), forms/draft (5), profile/independent region/UI states (6), operations/provider/deployment verification (7). Five review-focus conditions have explicit owning tests. Date calculations remain a separate deliverable as the approved design states.

Execution method remains inline/native in this session, consistent with the user’s earlier preference and .rules. After the human reviews this written plan, implement each checked step in order under `.rules`; do not start product code before that review.
