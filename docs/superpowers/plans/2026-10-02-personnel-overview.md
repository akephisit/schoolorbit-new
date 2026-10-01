# Personnel Overview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Execute inline in the current session; the repository requires an explicit user request before delegating to subagents.

**Goal:** เพิ่ม Dashboard งานบุคคลและข้อมูลตำแหน่ง วิทยฐานะ วุฒิ สาขา และสถาบันที่จัดกลุ่มได้ถูกต้อง พร้อมกรองรายชื่อจากกราฟและใช้กลุ่มสาระจากสังกัดเดิม

**Architecture:** `school-staff` owns canonical HR types, reference catalogs, scoped directory predicates and the aggregate. Root application handlers resolve session/policy and delegate to these services. One new migration replaces free-text education ownership after transactional reconciliation; route loaders and generated contracts own frontend reads.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, Svelte 5/SvelteKit, existing shadcn primitives, CSS/SVG, rootless Podman and Playwright. No new production dependency or crate.

**Spec:** [Approved personnel design](../specs/2026-10-02-personnel-overview-design.md), approved by the user's “เริ่มทำเลย” following the written-spec review request.

## Global Constraints

- Read `.rules` before changes. Keep the existing root branch `feat/personnel-overview`; no edits on `main`, no applied migration edits, no parallel agents.
- Exact rank codes: `none`, `not_applicable`, `proficient`, `senior_proficient`, `expert`, `senior_expert`; null is unspecified.
- Exact education codes: `primary`, `lower_secondary`, `upper_secondary`, `vocational_certificate`, `higher_vocational`, `diploma`, `bachelor`, `master`, `doctorate`, `other`; null is unspecified.
- Current group membership: `started_at <= CURRENT_DATE AND (ended_at IS NULL OR ended_at > CURRENT_DATE)` in an active subject-group unit linked to an active subject group. Deduplicate by user/group IDs.
- Never infer job position or rank from roles, duties or teaching assignments. Preserve `employment_type`, licenses, metadata, identities and relationships.
- Unknown legacy education blocks migration. No guessing, duplicate archives, runtime legacy fallbacks or post-cutover old binary rollback.
- Use generated DTOs/permissions, thin handlers, `ApiResponse`, resource scope, lazy action data and route-owned reads. Never log source rows, PII, national IDs, secrets or database URLs.
- Main integration/push is authorized by `.rules` only after the full feature passes required checks. Keep the feature branch through deployment acceptance.

## Review Focus

1. Catalog deactivation races with saving a person: an existing inactive assignment remains readable, while a newly inactive choice is rejected transactionally (Task 3).
2. People lacking a `staff_info` row: include them in total and unspecified chart buckets; creating HR data must preserve their user identity (Tasks 2 and 5).
3. Malformed or conflicting filter tokens in a deep link: reject invalid values safely and never silently widen the user's resource scope (Tasks 4 and 9).
4. A request resolves after logout, navigation, filter change or a closed editor: it must not paint another context, overwrite a draft or redirect (Tasks 7–9).
5. A tenant changes legacy values after preflight: migration rechecks locked source data and refuses cleanup without current reconciliation; no tenant migration starts if the all-tenant preflight fails (Tasks 1 and 10).

## File Ownership Map

- `backend-school/migrations/081_staff_personnel_data.sql`: canonical catalog, HR columns/constraints, transformation and bounded cutover audit. Recheck the latest number before creation.
- `backend-school/crates/school-staff/src/personnel.rs`: enums, HR patch/response, reference and overview DTOs. `models.rs` re-exports them and integrates staff DTO fields.
- `backend-school/crates/school-staff/src/services/staff_info_service.rs`: validated transactional create/patch/read of HR data.
- `backend-school/crates/school-staff/src/services/staff_directory_query.rs`: shared scope, status and HR/group predicates.
- `backend-school/crates/school-staff/src/services/reference_service.rs`: bounded reference search/create/update and normalization.
- `backend-school/crates/school-staff/src/services/personnel_overview_service.rs`: scoped aggregate; no home-dashboard dependency.
- `backend-school/crates/school-staff/src/services/personnel_cutover_service.rs`: bounded legacy preflight and canonical audit reading; legacy access is migration-only and version guarded.
- `backend-school/crates/school-staff/src/services/personnel_tests.rs`: sanitized DB fixtures and integration cases for these owners; register from `services.rs`.
- `backend-school/src/modules/staff/handlers/personnel.rs`: overview/catalog HTTP adapters, registered through `handlers.rs`, `app.rs`, `api_contract.rs`.
- `backend-school/src/modules/system/services/personnel_migration_service.rs`: all-tenant read-only preflight through direct temporary connections, bypassing the auto-migrating pool factory only for this explicit migration operation.
- `frontend-school/src/lib/api/personnel.ts`: generated-type API wrappers; existing `staff.ts` consumes generated list query types.
- `frontend-school/src/lib/forms/staff-personnel.ts`: exhaustive enum labels and nullable patch comparison; no persisted free-text path.
- `frontend-school/src/lib/components/staff/{StaffPersonnelFields,StaffReferencePicker,PersonnelBarChart,PersonnelStatusChart}.svelte`: focused fields, searchable lazy selection and accessible charts.
- Existing manage/new/edit/detail/list loaders and pages: integration, URL context and mutation ownership.
- New manage/overview and manage/reference-data route pairs: independent routes, route-owned aggregates/catalog pages, metadata and regional states.
- Existing route inventory, staff directory fixtures/tests, new personnel browser/static tests, canonical Testing/Operations and coordinated release workflow: verification and cutover gates.

## Task 1: Canonical migration, preflight and reconciliation

**Files:** Create migration `081_staff_personnel_data.sql`, `personnel_cutover_service.rs`, `personnel_tests.rs`; modify `lib.rs`/`services.rs` test registration. Follow the existing isolated database test pattern in `school-test-db`.

**Interfaces:** `read_personnel_preflight(pool: &PgPool) -> Result<PersonnelPreflightReport, AppError>`; `read_personnel_cutover_audit(pool: &PgPool) -> Result<PersonnelCutoverAudit, AppError>`. Reports contain migration version, `passed`, bounded check codes/counts and no source values. `PERSONNEL_MIGRATION_VERSION` is 81 unless the timeline advanced.

Define these report types in `personnel_cutover_service.rs`: `PersonnelCheck { code: String, passed: bool, count: i64 }`, `PersonnelPreflightReport { migration_version: i64, passed: bool, checks: Vec<PersonnelCheck> }`, and `PersonnelCutoverAudit` with the same bounded shape. Seed position codes are `teacher`, `assistant_teacher`, `contract_teacher`, `government_employee_teacher`, `school_director`, `deputy_school_director`, `support_staff` in the approved Thai label order. Generated catalog codes use `ref_` followed by the item's simple UUID; users edit labels, not codes.

Education mapping accepts each canonical code plus these exact aliases after whitespace normalization and lowercasing. All other nonblank values block migration:

| Canonical                | Accepted legacy aliases                              |
| ------------------------ | ---------------------------------------------------- |
| `primary`                | ประถมศึกษา                                           |
| `lower_secondary`        | มัธยมศึกษาตอนต้น, ม.3                                |
| `upper_secondary`        | มัธยมศึกษาตอนปลาย, ม.6                               |
| `vocational_certificate` | ปวช., ประกาศนียบัตรวิชาชีพ                           |
| `higher_vocational`      | ปวส., ประกาศนียบัตรวิชาชีพชั้นสูง                    |
| `diploma`                | อนุปริญญา                                            |
| `bachelor`               | ปริญญาตรี, ป.ตรี, bachelor's degree, bachelor degree |
| `master`                 | ปริญญาโท, ป.โท, master's degree, master degree       |
| `doctorate`              | ปริญญาเอก, ป.เอก, doctoral degree, phd, ph.d.        |
| `other`                  | อื่น ๆ, อื่นๆ                                        |

Blank values become null. No broad “contains” matching or inferred `other` mapping is allowed. Preflight and migration must agree on every alias in database-backed table-driven tests.

- [x] Write `migration_081_preserves_and_normalizes_personnel` against a sanitized fixture in a fresh `create_named_test_pool` schema, using a test-only SQLx Migrator filtered through version 80 before seeding: `ปริญญาตรี` and `ป.ตรี` become `bachelor`; `ปริญญาโท` becomes `master`; blank becomes null; identical normalized major/university names share an ID; person IDs, row IDs, employment, licenses and metadata remain identical. Use the complete migrations through 80, not a manually edited production schema or a simplified substitute table.
- [x] Run `./scripts/test_backend_school.sh --package school-staff migration_081 -- --test-threads=1`; confirm it fails because the canonical migration is absent, rather than accepting a zero-test result.
- [x] Implement the new transaction: lock `staff_info`, snapshot preservation fields in temporary transaction-local state, validate education mapping, create `staff_reference_items`, seed the seven approved positions, map references, verify each preservation/mapping check, record `staff_personnel_cutover_audit`, then remove persisted text `major`/`university`. Catalog normalized names use one database-owned normalization expression for migration and later writes. Add enum constraints and FK kind integrity, using generated constant-kind columns with composite `(id, kind)` references so manual wrong-kind writes also fail.
- [x] Add/run `migration_081_rejects_unknown_without_cleanup`, `migration_081_rejects_ambiguous_degree`, `migration_081_rechecks_drift`, `migration_081_failed_transaction_is_retryable`, `migration_081_preserves_empty_state`, and `migration_081_cutover_audit_is_required`. Assert unknown/ambiguous source is unchanged after failure, old columns still exist, no audit/canonical owner is partially committed, and a rerun after an explicitly corrected fixture succeeds. Run all Task 1 cases via the Podman command above.
- [x] Commit the migration and its owner/tests: `feat: add canonical personnel schema and guarded migration`.

Test assertions:

```rust
assert_eq!(education_codes, vec![Some("bachelor"), Some("bachelor"), Some("master"), None]);
assert_eq!(preserved_before, preserved_after);
assert!(!unknown_preflight.passed);
assert!(legacy_columns_remain_after_failure);
```

## Task 2: HR types and transactional persistence

**Files:** Create `personnel.rs`, `staff_info_service.rs`; modify `models.rs`, `lib.rs`, `services.rs`, `staff_service.rs`, and affected typed fixture construction in the staff crate.

**Interfaces:** `StaffAcademicRank`, `StaffEducationLevel`, `StaffReferenceKind`, `StaffReferenceSummary { id, code, name, is_active }`; create input has optional `job_position_id`, `academic_rank`, `education_level`, `major_id`, `university_id` and existing license fields. `UpdateStaffInfoRequest` uses omitted/null/value semantics. `StaffInfoResponse` returns `job_position`, `academic_rank`, `education_level`, `major`, `university`, with references represented as `Option<StaffReferenceSummary>`. Preserve existing outer staff field casing; new standalone DTOs use camelCase.

- [x] Write tests `personnel_patch_distinguishes_missing_null_value`, `personnel_rejects_unknown_enum`, `personnel_patch_preserves_license_and_employment`, and `personnel_patch_creates_missing_info_row`. Assertions pin `{}` to unchanged, `{"academic_rank":null}` to clear, and `{"academic_rank":"none"}` to an explicit value, with stable user/row IDs.
- [x] Run focused staff tests through `./scripts/test_backend_school.sh --package school-staff personnel_patch -- --test-threads=1` and enum tests via the same runner; confirm missing functionality fails.
- [x] Implement `read_staff_info(pool: &PgPool, user_id: Uuid) -> Result<Option<StaffInfoResponse>, AppError>`, `create_staff_info(tx: &mut Transaction<'_, Postgres>, user_id: Uuid, input: &CreateStaffInfoRequest) -> Result<(), AppError>`, and `patch_staff_info(tx: &mut Transaction<'_, Postgres>, user_id: Uuid, patch: &UpdateStaffInfoRequest) -> Result<(), AppError>` in the focused owner. Keep HR changes inside existing user create/update transactions, serialize per-person updates, join reference labels and replace free-text SQL/COALESCE paths.
- [x] Run the Task 2 tests, add wrong-kind/missing-reference/unchanged-inactive cases, then run the full affected staff crate suite. Verify seed code retains its valid `employment_type` behavior.
- [x] Commit: `feat: persist standardized personnel fields with nullable patches`.

Test assertions:

```rust
assert_eq!(omitted.academic_rank, None);
assert_eq!(cleared.academic_rank, Some(None));
assert_eq!(explicit_none.academic_rank, Some(Some(StaffAcademicRank::None)));
assert_eq!(license_before, license_after_hr_patch);
```

## Task 3: Reference catalogs and authorized HTTP workflow

**Files:** Create `reference_service.rs`, handler `personnel.rs`; modify service/handler registrations, `app.rs`, `api_contract.rs`; extend `personnel_tests.rs` and policy tests.

**Interfaces:** `list_reference_items(pool, ReferenceListQuery) -> Result<ReferencePage, AppError>`; `create_reference_item(pool, CreateReferenceRequest) -> Result<StaffReferenceItem, AppError>`; `update_reference_item(pool, id: Uuid, UpdateReferenceRequest) -> Result<StaffReferenceItem, AppError>`. Page contains `items`, `total`, `page`, `pageSize`; maximum page size 50. Kind, UUID and code are immutable.

Define `ReferenceListQuery { kind: StaffReferenceKind, search: Option<String>, status: Option<ReferenceStatusFilter>, page: Option<i64>, page_size: Option<i64> }`, where status is `active` (default), `inactive` or `all`; `CreateReferenceRequest { kind, name: String, display_order: Option<i32> }`; `UpdateReferenceRequest { name: Option<String>, is_active: Option<bool>, display_order: Option<i32> }`. All reject unknown fields. `StaffReferenceItem` extends the Task 2 summary with `kind`, `display_order`, `created_at`, `updated_at`; catalog DTOs serialize camelCase.

- [x] Write `reference_catalog_is_bounded_searchable_and_unique`, `reference_mutation_requires_staff_update`, and `reference_deactivation_race_rejects_new_assignment`. Assert normalized whitespace/case duplicates conflict, blank/over-200-character names fail, page size clamps to 50, and no hard deletion occurs.
- [x] Run `./scripts/test_backend_school.sh --package school-staff reference_ -- --test-threads=1` plus focused root handler/policy tests; confirm expected red results.
- [x] Implement service and `GET/POST /api/staff/reference-items`, `PATCH /api/staff/reference-items/{id}`. Read permits profile-read scopes or staff create/update; writes require `STAFF_UPDATE_ALL`. In HR validation, lock referenced catalog rows inside the person transaction and validate kind/status against the person's existing IDs, so deactivation cannot slip through validation. Return typed resources to support local UI updates.
- [x] Run Task 3 tests, including multibyte Thai names, normalized whitespace, page traversal, selected inactive reference read and read-only mutation denial.
- [x] Commit: `feat: add managed personnel reference catalogs`.

Test assertions:

```rust
assert_eq!(bounded_page.page_size, 50);
assert!(normalized_duplicate.is_err());
assert!(newly_deactivated_assignment.is_err());
assert_eq!(existing_inactive_assignment.id, original_reference_id);
```

## Task 4: Scoped directory filters and group ownership

**Files:** Create `staff_directory_query.rs`; modify `staff_service.rs`, `models.rs`, `staff_access_policy.rs`, root staff handler if needed, `OrganizationMembersSection.svelte`, and directory/policy tests.

**Interfaces:** `push_staff_access_filter(query: &mut QueryBuilder<Postgres>, access: StaffListAccess)` and shared validated predicates exported only within `school-staff`; `StaffListFilter` adds job/rank/education/group selection. `StaffListItem` adds reference position and rank. `StaffProfileResponse` adds `subject_groups: Vec<StaffSubjectGroupSummary { id, name }>` derived from current affiliations.

- [x] Write `personnel_directory_filters_match_missing_values`, `personnel_directory_scope_survives_unrelated_permissions`, and `personnel_groups_deduplicate_current_membership`. Pin `unspecified`/`unassigned`, wrong UUID/enum, multiple units linked to one group, future starts, expired memberships and inactive units/groups.
- [x] Run filtered crate tests and root `policies::staff_access_policy::tests`; confirm the old resolver's unrelated-permission widening is exposed.
- [x] Extract the existing scope predicates and add shared HR/group validation/filtering. Directory and overview resolve only profile-read resource scopes. Switch the organization member picker from full directory data to `lookupStaff` under its existing action gates; retain its keyboard selection and lazy search. Ensure rows/counts and profile group summaries use the same current-affiliation rules.
- [x] Run Task 4 cases plus existing scoped directory and organization picker browser tests. Assert own-only plus achievement-create permission remains own-only; no HR assignment changes roles or organizational duties.
- [x] Commit: `feat: add scoped personnel filters and derived subject groups`.

Test assertions:

```rust
assert_eq!(own_with_achievement_permission, own_without_achievement_permission);
assert_eq!(duplicate_unit_group_membership_count, 1);
assert_eq!(invalid_filter_error.status_code(), StatusCode::BAD_REQUEST);
```

## Task 5: Personnel aggregate endpoint

**Files:** Create `personnel_overview_service.rs`; modify `personnel.rs`, handler `personnel.rs`, `app.rs`, `api_contract.rs`, and `personnel_tests.rs`.

**Interfaces:** `get_personnel_overview(pool: &PgPool, query: PersonnelOverviewQuery, access: StaffListAccess) -> Result<PersonnelOverview, AppError>`. Query has `status` default `active`. Response has `asOf`, `total`, `active`, `otherStatuses`, `filteredTotal`, `statuses`, `subjectGroups`, `jobPositions`, `academicRanks`, `educationLevels`. Buckets contain label, count and the typed filter value used for drilldown; missing buckets use the approved tokens.

Define `PersonnelStatusFilter` as `all`, `active`, `inactive`, `suspended`, `resigned`, `retired`; `PersonnelDimension` as `status`, `subject_group`, `job_position`, `academic_rank`, `education_level`; and `PersonnelBucket { key: String, label: String, count: i64 }`. Keys are valid values for their dimension: current status codes, a reference/group UUID, canonical rank/education code, `unspecified`, or `unassigned`. Return the appropriate array for each dimension; never return arbitrary unvalidated filter text.

- [x] Write `personnel_overview_matches_directory_counts`: for every returned nonzero bucket, query `list_staff` with the bucket/status and assert the same total. Assert rank/position/education sums equal filtered total; a multi-group person counts once in root total and once per distinct group. Add missing-info, zero, status-all and all five account-status fixtures.
- [x] Run `./scripts/test_backend_school.sh --package school-staff personnel_overview -- --test-threads=1`; confirm missing endpoint/service fails.
- [x] Implement one consistent scoped aggregate using a matched-staff CTE and pre-aggregated distinct group relationships. Return counts only, no profiles or person names. Cards/status counts use all statuses within scope; category charts use selected status. Use shared predicates and deterministic label ordering; register `GET /api/staff/personnel-overview` ahead of the dynamic staff ID route.
- [x] Run Task 5 tests across own/unit/tree/school and denied, including mixed unrelated permissions. Verify aggregate snapshot/count consistency and safe error output. Do not make a performance claim without representative timing/query-plan evidence.
- [x] Commit: `feat: expose scoped personnel overview aggregates`.

Use three people in the core count fixture: A is active, teacher/proficient/bachelor in Math and Science (two Math units); B is active, assistant/none/master in Math with a future Science membership; C is inactive, has no `staff_info`, and has no subject-group membership. An ordinary root/child-unit membership for C exercises tree scope without adding a group. Scope-specific cases use isolated fixtures.

```rust
assert_eq!((overview.total, overview.active, overview.other_statuses, overview.filtered_total), (3, 2, 1, 2));
assert_eq!(overview.subject_groups.iter().find(|b| b.key == math_id.to_string()).unwrap().count, 2);
assert_eq!(overview.subject_groups.iter().find(|b| b.key == science_id.to_string()).unwrap().count, 1);
assert_eq!(list_total_for_bucket, bucket.count);
```

## Task 6: Generated contracts and frontend API boundary

**Files:** Regenerate existing API contract artifacts; create `frontend-school/src/lib/api/personnel.ts` and `forms/staff-personnel.ts`; modify `api/staff.ts`, `navigation/staff-management.ts`, existing fixtures and focused static tests.

**Interfaces:** generated schemas supply all request/response/enum types. Wrappers: `getPersonnelOverview(query, options)`, `listStaffReferenceItems(query, options)`, `createStaffReferenceItem(input)`, `updateStaffReferenceItem(id, input)`. Each returns the typed domain resource through `requireApiData`. `buildStaffPersonnelPatch(before, after) -> UpdateStaffInfoRequest | undefined`; `personnelDrilldownHref(dimension: PersonnelDimension, bucket: PersonnelBucket, status: PersonnelStatusFilter) -> string` uses the list URL allowlist. The helper serializes the dimension to the exact Task 4 query name; status slices use their own bucket key.

- [x] Write static/behavior assertions for generated endpoint/schema presence, exhaustive rank/education labels, new list query serialization, return context preservation, and omission versus explicit null in `buildStaffPersonnelPatch`. Example: clearing `major_id` emits `{ major_id: null }`; unchanged fields emit no patch.
- [x] Run focused tests before regeneration and confirm missing contracts/helpers fail.
- [x] Register every DTO/path in Rust OpenAPI, generate API contracts once the preceding tasks are coherent, implement typed wrappers and generated operation query types. Remove persisted-text create/update fields and update only the fixture consumers found by direct search. Add HR/group filter parameters to the return URL allowlist; keep its open-redirect checks.
- [x] Run `npm run generate:api-contracts`, `npm run check:api-contracts`, `npm run test:api-contracts`, focused static tests and Svelte check. All regenerated artifacts must be reproducible offline without database credentials.
- [x] Commit: `feat: generate personnel contracts and typed client helpers`.

Test assertions (generated input shapes remain optional; omitted fields are unchanged):

```typescript
assert.deepEqual(
  buildStaffPersonnelPatch({ major_id: majorId }, { major_id: null }),
  { major_id: null },
);
assert.equal(
  buildStaffPersonnelPatch(
    { academic_rank: "none" },
    { academic_rank: "none" },
  ),
  undefined,
);
assert.equal(
  new URL(drilldownHref, "https://schoolorbit.invalid").searchParams.get(
    "subject_group_id",
  ),
  mathId,
);
```

## Task 7: Staff HR editor, create flow and profile

**Files:** Create `StaffReferencePicker.svelte`, `StaffPersonnelFields.svelte`; modify manage/new, manage/[id]/edit and manage/[id] pages/loaders, `staff-create-draft.ts` when safe IDs need draft support, and directory E2E fixtures/tests.

**Interfaces:** picker takes reference kind, selected summary, nullable ID value, allowed state and accessible label; searches lazily on opening with a maximum-50-item page and `LatestRequest` ownership. Fields use the generated HR input and enum label helpers. Existing form owns the draft and mutation epoch.

- [x] Add production-preview browser tests for selecting job/rank/degree/reference, create review, changing then clearing values, retaining inactive selections, and displaying unknown fields. Assert personal-only saves omit `staff_info` and HR-only saves do not clear licenses or send role/org mutations.
- [x] Inspect the baseline controls and run production-preview browser coverage. Canonical schema/static tests provide feature RED evidence; creation binding regressions provide observed browser RED evidence, followed by green verification.
- [x] Implement the shared controls in the existing wizard information/review steps and the editor's named section. Use standard Select for fixed values and searchable lazy reference picker. Add a permission-gated link to reference management that preserves the draft/return destination, and a refresh action for the selected options. Detail displays reference names, rank/degree labels and derived group summaries.
- [x] Rebuild and run tests for draft warning, deep links, validation, omitted/null/value payloads, late picker search, failed-save draft retention, logout/navigation during submit and selected inactive rows. Use project Svelte tooling to resolve all errors/warnings.
- [x] Commit: `feat: add standardized personnel controls to staff workflows`.

Browser assertions:

```typescript
expect(savedPayload.staff_info.major_id).toBeNull();
expect(personalOnlyPayload).not.toHaveProperty("staff_info");
await expect(
  page.getByRole("button", { name: "ตำแหน่งงาน", exact: true }),
).toBeVisible();
expect(staleNavigationCount).toBe(0);
```

## Task 8: Reference-management page

**Files:** Create `staff/manage/reference-data/+page.ts` and `+page.svelte`; add inventory/route access records and personnel fixture/browser tests.

**Interfaces:** child `_meta.access` requires `STAFF_UPDATE_ALL`, academic context `none`. URL owns `kind`, `search`, `page`, active filter. Loader owns the selected bounded catalog region through event `fetch` and a named dependency `school:staff-reference-items`.

- [x] Write browser tests for exact permission denial, independent catalog loading/error/retry, creating/renaming/deactivating, duplicate conflict and pagination. Assert read-only staff never request the management catalog or mutation endpoint.
- [x] Inspect the baseline route absence and verify the new route through production-preview coverage. Record that initial browser smoke failures concerned mock authentication rather than claiming route-absence RED evidence.
- [x] Implement one selected-kind catalog region with focused initial skeleton, retained refresh, cancellation and local mutation patching. Use existing dialog/table/form primitives and typed results; no hard-delete button. Guard concurrent dialog saves and preserve names on failure. Links back to forms are allowlisted local destinations, not arbitrary `returnTo` values.
- [x] Run the Task 8 tests, including switching kind during a pending read/save, closing a dialog before completion, empty catalog and mobile controls. Register the route-data inventory and run route/access/menu checks.
- [x] Commit: `feat: add personnel reference management page`.

Browser assertions:

```typescript
expect(readOnlyCatalogRequests).toBe(0);
expect(readOnlyMutationRequests).toBe(0);
await expect(
  page.getByText("รายการชื่อนี้มีอยู่แล้ว", { exact: true }),
).toBeVisible();
await expect(page.getByTestId("staff-reference-catalog")).not.toContainText(
  oldKindResultName,
);
```

## Task 9: Dashboard, charts and directory drilldown

**Files:** Create overview route pair, `PersonnelBarChart.svelte`, `PersonnelStatusChart.svelte`; modify directory route pair, staff breadcrumb/navigation, route inventory/menu metadata and personnel fixtures/browser tests.

**Interfaces:** overview `_meta.menu` belongs to personnel workspace/group with profile-read discovery and academic context `none`; loader owns `school:personnel-overview` and uses `getPersonnelOverview`. Chart props consume generated buckets and drilldown helper. Directory retains all existing role/org/status/search filters and adds the four HR/group dimensions.

- [x] Write browser assertions for all cards/charts, exact labels and values, active/all status filtering, unknown buckets, multi-group note, accessible keyboard links, and graph-to-directory-to-detail-to-return. Add first-paint skeleton, retained refresh and malformed deep-link cases.
- [x] Run the tests and confirm the route/charts/filters are absent.
- [x] Implement the three summary cards, SVG status donut with text legend and four horizontal bar charts using theme tokens. Render zero states without invalid percentages, keep labels readable, and use one-column mobile layout. Preserve context through list/profile/editor navigation; load filter options only when opened and authorized.
- [x] Build production preview and run the Task 9 tests at desktop and 390px mobile widths. Test empty/own-only/error/retry/refresh, rapid status changes, slow earlier navigation completion and logout. Assert the selected bucket's list count equals its shown graph count from backend-backed fixtures.
- [x] Commit: `feat: add personnel dashboard and linked directory filters`.

Browser assertions:

```typescript
await expect(
  page.getByRole("heading", { name: "ภาพรวมงานบุคคล", exact: true }),
).toBeVisible();
expect(directoryTotalAfterDrilldown).toBe(selectedBucketCount);
await expect(page).toHaveURL(new RegExp(`subject_group_id=${mathId}`));
expect(horizontalDocumentOverflowAt390px).toBe(false);
```

## Task 10: All-tenant cutover safety and operational gates

**Files:** Create system `personnel_migration_service.rs`; modify system service/handler registration, `app.rs`, migration handler/status, release workflow, deployment guard, and canonical `docs/OPERATIONS.md`/`docs/TESTING.md`.

**Interfaces:** `preflight_personnel_tenants(schools: &[ActiveSchool]) -> Result<PersonnelTenantPreflight, AppError>` opens short-lived direct read-only pools without invoking auto-migration and delegates Task 1 checks. `GET /internal/personnel-preflight` uses existing internal authentication. Migration status adds `personnelCutover { migrationVersion, status, passed, checks }`, with no source values.

Define camelCase DTOs `PersonnelTenantPreflight { total_schools: usize, passed: bool, schools: Vec<PersonnelSchoolPreflight> }` and `PersonnelSchoolPreflight { subdomain: String, migration_version: i64, passed: bool, checks: Vec<PersonnelCheck> }`. A missing connection or failed check reports a bounded finding and `passed: false`. Both preflight and cutover status require nonempty checks; an empty checks array cannot pass vacuously.

- [x] Write root service tests `personnel_preflight_prevents_any_tenant_migration_on_failure`, `personnel_preflight_reads_actual_version`, and `personnel_status_requires_current_audit`. Add a deployment guard that requires preflight success before `/internal/migrate-all` and personnel audit success before proxy reopening.
- [x] Run focused root tests through the Podman runner and the deployment static test; confirm missing gate/audit fails.
- [x] Implement the preparatory all-tenant gate before any `get_pool_with_permission_change` call; reuse the already-read active-school list. Register the internal read-only operation and bounded status checks. Gate the coordinated full release on preflight then all-tenant audit. Map errors to bounded `PERSONNEL_*` findings; never log connection URLs or raw degree/catalog values. Version-guard legacy preflight access and use canonical audit after migration, without a product runtime fallback.
- [x] Run focused gate/status tests and the full applicable workflow/topology matrix from `.rules`: shellcheck, shfmt, installer Bats, deployment static guard, Compose dry-run, Podman actionlint. Document the migration-only read boundary, maintenance sequence, protected snapshot, old-binary prohibition and roll-forward recovery in Operations; put reproducible test recipes in Testing.
- [x] Commit: `feat: gate personnel cutover with all-tenant reconciliation`.

Test assertions:

```rust
assert!(!failed_tenant_preflight.passed);
assert_eq!(migration_calls_after_failed_preflight, 0);
assert_eq!(reported_version, actual_sqlx_version);
assert!(!status_without_cutover_audit.passed);
```

## Task 11: Final verification, integration and deployment acceptance

**Files:** Final whole-branch diff and generated artifacts; approved workflow artifacts are removed only after an implementation PR records the completed outcome, preserving their reviewed Git history as `.rules` requires.

- [x] Review the entire feature against the spec inline, including every denied path and removal of free-text runtime owners. Run focused database/browser tests for any finding that requires a fix; no subagent or additional permission prompt is needed under the supplied execution method.
- [x] From `backend-school`, run `cargo fmt --all -- --check`, `cargo test --test static_architecture`, `cargo check --workspace --all-targets`, and `RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school`. From the root, run full affected `school-staff` tests via `./scripts/test_backend_school.sh --package school-staff -- --test-threads=1` and focused root policy/handler/cutover tests via the same runner. Expected: every applicable check passes and every focused filter runs at least one test.
- [x] Run frontend API generation/check/test and permission generation/check/test if the registry changes; `npm run lint`, `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`, `npm run test:static`, `npm run test:menu-sync`, `npm run test:route-loading`, production build and the affected staff/personnel Playwright suite through local production preview. For real-account acceptance, disable traces/screenshots/video and use only existing environment credentials.
- [ ] Verify the safe production preflight/fixture rehearsal and protected recovery prerequisites before promotion. If an actual unmatched value is found, report only bounded evidence, retain the branch, and obtain a specific mapping decision; do not guess or push a release that would strand maintenance. Inspect `git diff --check`, final diff/status, fetch main, resolve any divergence and rerun affected checks. Commit, squash into updated main, confirm identical tree IDs for result reuse, and push normally as `.rules` authorizes.
- [ ] Monitor contract CI and the full coordinated release. Require all-tenant version/audits, readiness, repository smoke through the deployed proxy, frontend assets/route availability and authorized read-only staff/personnel acceptance before reporting completion. Keep the branch while recovery remains necessary; remove workflow artifacts only when an implementation PR has recorded the outcome, otherwise retain them under the allowed Superpowers paths.
