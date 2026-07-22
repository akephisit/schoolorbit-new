# Role and Organization Unit Soft-Deactivation Implementation Plan

> **For implementation:** Follow this plan in order with `superpowers:test-driven-development`. Do not edit applied migrations or generated API artifacts by hand. Run `svelte-autofixer` after every touched Svelte component.

**Goal:** Implement reversible role and organization-unit deactivation so inactive authorization sources stop granting access, protected records remain safe, administrators can reactivate records, and backend/runtime/OpenAPI/frontend contracts stay aligned.

**Architecture:** Add explicit system-record metadata through a new migration, keep status-transition rules in the existing staff services, and write audit rows in the same transaction. Axum handlers own permission checks plus cache/realtime side effects. Authorization queries and organization-scope policies filter inactive sources. Backend `utoipa` metadata remains the wire-contract source of truth; generated OpenAPI and TypeScript are regenerated and consumed by the existing frontend API wrappers.

**Tech Stack:** Rust, Axum, sqlx, PostgreSQL, utoipa, SvelteKit 5, TypeScript, Tailwind/shadcn-svelte, Node static tests, generated OpenAPI contracts.

**Design spec:** `docs/superpowers/specs/2026-07-22-role-organization-soft-deactivation-design.md`

---

## Execution setup

Before Task 1, use `superpowers:using-git-worktrees` and create an isolated feature worktree from the current `main`, which already contains the approved design and this plan. Suggested branch:

```text
feat/role-organization-soft-deactivation
```

Verify the worktree baseline before changes:

```bash
git status --short --branch
cd backend-school && cargo test error::tests --bin backend-school
cd ../frontend-school && npm run check:api-contracts
```

Expected: clean feature branch and passing focused baseline. Database-backed tests require `TEST_DATABASE_URL`; do not imply that they ran when the variable is absent.

## Task 1: Add protected-record schema and domain conflict errors

**Files:**

- Create: `backend-school/migrations/027_role_organization_system_flags.sql`
- Modify: `backend-school/src/error.rs`
- Modify: `backend-school/src/modules/staff/models.rs`
- Modify: `backend-school/tests/static_architecture.rs`

### Step 1: Write failing tests

Add an `AppError` response test in `backend-school/src/error.rs`:

```rust
#[tokio::test]
async fn conflict_error_returns_standard_409_envelope() {
    let response = AppError::Conflict("สถานะทรัพยากรขัดแย้ง".to_string()).into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
```

Add a migration guard in `backend-school/tests/static_architecture.rs` that reads only the new migration, normalizes whitespace, and asserts:

```rust
let normalized = migration.split_whitespace().collect::<Vec<_>>().join(" ");
assert!(normalized.contains("ALTER TABLE roles ADD COLUMN is_system"));
assert!(normalized.contains("ALTER TABLE organization_units ADD COLUMN is_system"));
assert!(normalized.contains("WHERE code = 'ADMIN'"));
assert!(normalized.contains("WHERE code = 'SCHOOL'"));
```

The test must also confirm that `CreateRoleRequest`, `UpdateRoleRequest`, `CreateOrganizationUnitRequest`, and `UpdateOrganizationUnitRequest` do not expose `is_system`.

Run:

```bash
cd backend-school
cargo test conflict_error_returns_standard_409_envelope --bin backend-school
cargo test role_and_organization_system_flags_are_migration_owned --test static_architecture
```

Expected: FAIL because the variant, fields, and migration do not exist.

### Step 2: Add the migration and model fields

Create migration 027 without touching `001_baseline.sql`:

```sql
ALTER TABLE roles
    ADD COLUMN is_system boolean NOT NULL DEFAULT false;

ALTER TABLE organization_units
    ADD COLUMN is_system boolean NOT NULL DEFAULT false;

UPDATE roles SET is_system = true WHERE code = 'ADMIN';
UPDATE organization_units SET is_system = true WHERE code = 'SCHOOL';

COMMENT ON COLUMN roles.is_system IS
    'Protected system role; status cannot be deactivated through normal APIs';
COMMENT ON COLUMN organization_units.is_system IS
    'Protected system unit; status cannot be deactivated through normal APIs';
```

Add `pub is_system: bool` to the response/database models `Role` and `OrganizationUnit`. Do not add it to create/update payloads.

### Step 3: Implement an explicit 409 domain error

Add:

```rust
Conflict(String),
```

to `AppError`, map it to `StatusCode::CONFLICT`, and retain `ApiErrorResponse` as the body.

### Step 4: Run focused tests

```bash
cd backend-school
cargo fmt --all -- --check
cargo test conflict_error_returns_standard_409_envelope --bin backend-school
cargo test role_and_organization_system_flags_are_migration_owned --test static_architecture
cargo check --bin backend-school
```

Expected: PASS.

### Step 5: Commit

```bash
git add backend-school/migrations/027_role_organization_system_flags.sql \
  backend-school/src/error.rs \
  backend-school/src/modules/staff/models.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(authz): mark protected access records"
```

## Task 2: Make inactive authorization sources ineffective

**Files:**

- Modify: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/modules/auth/services.rs`
- Modify: `backend-school/src/modules/staff/services/user_role_service.rs`
- Modify: `backend-school/src/policies/resource_access_policy.rs`
- Modify: `backend-school/src/modules/staff/services/staff_service.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Create: `backend-school/src/modules/staff/services/status_tests.rs`
- Modify: `backend-school/src/modules/staff/services.rs`

### Step 1: Add fast failing architecture tests

Add a test named `inactive_authorization_sources_are_filtered` that directly inspects the current authorization source files and requires:

- effective role permissions join `roles` and check `r.is_active = true`;
- organization grants join `organization_units` and check `ou.is_active = true`;
- scoped delegations require a live organization unit while unscoped delegations remain valid;
- current-user primary role and the user-permissions admin read exclude inactive roles;
- resource-access organization roots/members join active organization units;
- staff organization-unit/tree list filters start from active units.

This static test is a focused regression guard, not a replacement for the database behavior test below.

Run:

```bash
cd backend-school
cargo test inactive_authorization_sources_are_filtered --test static_architecture
```

Expected: FAIL on the current SQL.

### Step 2: Add a database behavior test

Register a test-only module in `backend-school/src/modules/staff/services.rs`:

```rust
#[cfg(test)]
mod status_tests;
```

In `status_tests.rs`, create one isolated end-to-end authorization scenario using `create_test_pool()`, `run_test_migrations()`, unique test users, and distinct existing permission codes. Prove this sequence:

1. an active role assignment contributes its permission;
2. setting the role inactive and invalidating the test cache removes it;
3. an active organization membership/grant contributes a different permission;
4. setting the unit inactive removes it;
5. a valid delegation scoped to the inactive unit does not contribute its permission;
6. an otherwise-valid unscoped delegation still contributes;
7. reactivation restores preserved role/unit relationships.

Call the same public cached-permission loader used by production:

```rust
let cache = PermissionCache::new();
let permissions = get_cached_user_permissions("status-test", user_id, &pool, &cache).await?;
```

Run with a dedicated test database:

```bash
cd backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test modules::staff::services::status_tests --bin backend-school -- --nocapture
```

Expected: FAIL until the authorization SQL is corrected.

### Step 3: Correct the effective-permission query

Update `fetch_user_permissions` so the three union branches use active records:

```sql
FROM user_roles ur
JOIN roles r ON r.id = ur.role_id AND r.is_active = true
JOIN role_permissions rp ON rp.role_id = r.id
```

```sql
FROM organization_members om
JOIN organization_units ou
  ON ou.id = om.organization_unit_id AND ou.is_active = true
JOIN organization_permission_grants opg
  ON opg.organization_unit_id = ou.id
```

```sql
LEFT JOIN organization_units delegated_ou
  ON delegated_ou.id = opd.organization_unit_id
...
AND (opd.organization_unit_id IS NULL OR delegated_ou.is_active = true)
```

Retain all existing assignment dates, membership dates, expiry, and revocation predicates.

### Step 4: Correct secondary authorization/scope reads

- Filter `get_primary_role_name` and `user_role_service::get_user_permissions` through active roles.
- In `resource_access_policy.rs`, make actor unit IDs and recursive actor roots start from active units. Descendants already require active units; keep that check.
- Make same-unit membership checks join the active unit.
- In `staff_service.rs`, add the active-unit join to organization-unit and organization-tree list filters so inactive membership cannot expand staff visibility.
- Historical profile/detail reads may continue to display inactive assignments; do not erase them.

### Step 5: Run tests

```bash
cd backend-school
cargo test inactive_authorization_sources_are_filtered --test static_architecture
cargo test middleware::permission::tests --bin backend-school
cargo test policies::resource_access_policy::tests --bin backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test modules::staff::services::status_tests --bin backend-school -- --nocapture
cargo check --bin backend-school
```

Expected: PASS. If `TEST_DATABASE_URL` is unavailable, explicitly report that the database scenario was not run.

### Step 6: Commit

```bash
git add backend-school/src/middleware/permission.rs \
  backend-school/src/modules/auth/services.rs \
  backend-school/src/modules/staff/services/user_role_service.rs \
  backend-school/src/policies/resource_access_policy.rs \
  backend-school/src/modules/staff/services/staff_service.rs \
  backend-school/src/modules/staff/services/status_tests.rs \
  backend-school/src/modules/staff/services.rs \
  backend-school/tests/static_architecture.rs
git commit -m "fix(authz): ignore inactive access sources"
```

## Task 3: Implement transactional role status transitions and audit

**Files:**

- Modify: `backend-school/src/utils/audit.rs`
- Modify: `backend-school/src/modules/staff/services/role_service.rs`
- Modify: `backend-school/src/modules/staff/services/status_tests.rs`

### Step 1: Write failing role lifecycle tests

Extend `status_tests.rs` to cover:

- active custom role returns `Changed { is_active: false }`;
- deleting it again returns `Unchanged`;
- `ADMIN` returns `AppError::Conflict` and remains active;
- reactivation returns `Changed { is_active: true }`;
- each real transition writes exactly one audit row with actor ID, action, entity ID, and only status values;
- an idempotent transition writes no additional audit row.

Add pure unit tests for the transition outcome enum so the cache/event decision can later use `outcome.changed()` without database access.

Run:

```bash
cd backend-school
cargo test role_status_outcome --bin backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test role_soft_deactivation --bin backend-school -- --nocapture
```

Expected: FAIL because the service functions and outcome do not exist.

### Step 2: Add transaction-aware audit persistence

Extend `AuditLogBuilder` with a `save_in_transaction` path that executes the same insert using `&mut Transaction<'_, Postgres>`. Keep the existing pool-based method for current callers.

Do not log permission arrays, memberships, request headers, or personal data. Use:

```json
{"is_active": true}
```

and:

```json
{"is_active": false}
```

for old/new values.

### Step 3: Implement the role transition primitive

In `role_service.rs`, add a structured outcome such as:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTransitionOutcome {
    Changed { is_active: bool },
    Unchanged,
}
```

Implement a private transaction-scoped transition that:

1. selects `name`, `is_active`, and `is_system` with `FOR UPDATE`;
2. returns not found for a missing role;
3. returns unchanged for an identical state;
4. rejects transition to inactive when `is_system` is true;
5. updates `is_active` and `updated_at`;
6. inserts the audit row in the same transaction.

Expose:

```rust
pub async fn set_role_active(
    pool: &PgPool,
    role_id: Uuid,
    is_active: bool,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError>
```

### Step 4: Route general updates through the same primitive

Change `update_role` to accept `actor_user_id` and return enough outcome information for the handler to distinguish a real status transition. Keep role-field updates, permission replacement, status validation, and status audit in one transaction.

Do not call `set_role_active` in a separate transaction from the other update fields.

### Step 5: Add `include_inactive` service behavior

Change:

```rust
pub async fn list_roles(pool: &PgPool, include_inactive: bool)
```

Defaulting remains a handler concern. The SQL applies `r.is_active = true` only when `include_inactive` is false.

### Step 6: Run focused tests and commit

```bash
cd backend-school
cargo fmt --all -- --check
cargo test role_status_outcome --bin backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test role_soft_deactivation --bin backend-school -- --nocapture
cargo check --bin backend-school
```

```bash
git add backend-school/src/utils/audit.rs \
  backend-school/src/modules/staff/services/role_service.rs \
  backend-school/src/modules/staff/services/status_tests.rs
git commit -m "feat(authz): add audited role deactivation"
```

## Task 4: Implement organization-unit transitions and hierarchy invariants

**Files:**

- Modify: `backend-school/src/modules/staff/services/organization_unit_service.rs`
- Modify: `backend-school/src/modules/staff/services/status_tests.rs`

### Step 1: Write failing organization lifecycle tests

Cover:

- `SCHOOL` cannot be deactivated;
- a non-system leaf can be deactivated and deleted idempotently;
- a unit with an active direct child cannot be deactivated;
- a unit whose children are all inactive can be deactivated;
- a child cannot be reactivated under an inactive parent;
- an active unit cannot be created/moved under an inactive parent;
- every real status transition writes one transactional audit row;
- `list_organization_units(pool, false)` excludes inactive units;
- `list_organization_units(pool, true)` includes them.

Run the new test names with `TEST_DATABASE_URL`. Expected: FAIL.

### Step 2: Implement transaction-scoped validation

Add `set_organization_unit_active` and a private transaction helper mirroring the role implementation. Use `FOR UPDATE` on the target row and return `AppError::Conflict` for system and hierarchy violations.

Before deactivation, query active direct children:

```sql
SELECT EXISTS (
    SELECT 1 FROM organization_units
    WHERE parent_unit_id = $1 AND is_active = true
)
```

Before reactivation, require the parent to be active when `parent_unit_id` is non-null.

### Step 3: Reuse the primitive from updates

Change `update_organization_unit` to accept `actor_user_id`, keep field/status/audit work in one transaction, and return `StatusTransitionOutcome`.

Validate an explicitly supplied parent for create/update. If the unit remains active, the parent must be active. Preserve existing payload semantics; do not redesign nullable parent clearing in this batch.

### Step 4: Add `include_inactive` service behavior

Change:

```rust
pub async fn list_organization_units(pool: &PgPool, include_inactive: bool)
```

Apply the active filter only when false.

### Step 5: Run tests and commit

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test organization_unit_soft_deactivation --bin backend-school -- --nocapture
cargo check --bin backend-school
```

```bash
git add backend-school/src/modules/staff/services/organization_unit_service.rs \
  backend-school/src/modules/staff/services/status_tests.rs
git commit -m "feat(authz): add audited unit deactivation"
```

## Task 5: Reject new assignments to inactive access records

**Files:**

- Modify: `backend-school/src/modules/staff/services/organization_member_service.rs`
- Modify: `backend-school/src/modules/staff/services/organization_delegation_service.rs`
- Modify: `backend-school/src/modules/staff/services/staff_service.rs`
- Modify: `backend-school/src/modules/staff/services/status_tests.rs`

### Step 1: Write failing association tests

Add database cases proving:

- `add_member` rejects an inactive unit;
- moving a member to an inactive unit rejects;
- creating a scoped delegation for an inactive unit rejects;
- staff create/update rejects inactive role IDs and inactive organization assignments;
- configuring permission grants on an inactive unit remains allowed but ineffective until reactivation.

Expected: FAIL on current direct inserts.

### Step 2: Add shared validation at service boundaries

- Add an `ensure_organization_unit_active` service helper used by member add/move and delegation creation.
- Validate all role IDs and organization-unit IDs before bulk replacement in `staff_service`; perform validation before deleting existing assignments during staff update.
- Retain the current user-type validation for roles.
- Do not silently skip invalid IDs in bulk inserts.

### Step 3: Run tests and commit

```bash
cd backend-school
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test inactive_assignment_targets_are_rejected --bin backend-school -- --nocapture
cargo test modules::staff::services --bin backend-school
cargo check --bin backend-school
```

```bash
git add backend-school/src/modules/staff/services/organization_member_service.rs \
  backend-school/src/modules/staff/services/organization_delegation_service.rs \
  backend-school/src/modules/staff/services/staff_service.rs \
  backend-school/src/modules/staff/services/status_tests.rs
git commit -m "fix(authz): reject inactive assignment targets"
```

## Task 6: Add Axum routes, permissions, cache events, and OpenAPI metadata

**Files:**

- Modify: `backend-school/src/modules/staff/handlers/roles.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

### Step 1: Change contract tests first

In `api_contract.rs`:

- replace assertions that the two delete operations are null;
- add `deleteRole` and `deactivateOrganizationUnit` to the authorization operation inventory;
- change the authorization count from 30 to 32 and total unique operation count from 66 to 68;
- assert HTTP 200 `ApiResponse_EmptyData`, 401, 403, 404, 409, and unique operation IDs;
- assert `include_inactive` is an optional boolean query parameter on both lists;
- assert `Role.is_system` and `OrganizationUnit.is_system` are required booleans;
- add 409 responses to update operations.

In `static_architecture.rs`, require runtime handlers for both deletes and ensure the authorization router-to-OpenAPI guard sees them.

Run:

```bash
cd backend-school
cargo test api_contract::tests --bin backend-school
cargo test authorization_handlers_are_registered_in_the_openapi_document --test static_architecture
```

Expected: FAIL.

### Step 2: Add list query extraction and permission rules

Add a typed query DTO such as:

```rust
#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct ListManagedResourcesQuery {
    pub include_inactive: Option<bool>,
}
```

List handlers pass `unwrap_or(false)` to services.

For `PUT`, require `roles.update.all` as today. If `payload.is_active == Some(false)`, additionally require `roles.delete.all` before invoking the service. Pass `actor.user_id` into audited update services.

### Step 3: Add delete handlers

Add documented handlers:

```rust
pub async fn deactivate_role(...)
pub async fn deactivate_organization_unit(...)
```

Each handler:

1. resolves actor and tenant once;
2. requires `roles.delete.all`;
3. calls the relevant status service with `false` and `actor.user_id`;
4. invalidates the tenant cache and notifies all users only for `Changed`;
5. returns an explicit Thai soft-deactivation message in `ApiResponse<EmptyData>`.

Update handlers invalidate/notify on real status changes. Role updates continue tenant invalidation when permissions are replaced, preserving existing behavior.

### Step 4: Register runtime and OpenAPI routes

Consolidate each resource path into one Axum method router where practical:

```rust
get(get_role).put(update_role).delete(deactivate_role)
```

Register both handlers in `SchoolApiDoc` paths. Do not edit generated JSON manually.

### Step 5: Run tests and commit

```bash
cd backend-school
cargo fmt --all -- --check
cargo test api_contract::tests --bin backend-school
cargo test --test static_architecture
cargo check --bin backend-school
```

```bash
git add backend-school/src/modules/staff/handlers/roles.rs \
  backend-school/src/main.rs \
  backend-school/src/api_contract.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(api): expose access deactivation contracts"
```

## Task 7: Regenerate contracts and type the frontend API wrappers

**Files:**

- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/roles.ts`
- Modify: `frontend-school/src/lib/api/staff.ts`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Create: `frontend-school/tests/static/access-soft-deactivation.test.mjs`

### Step 1: Write failing frontend contract tests

Update the authorization expected operation list to 32 and assert the two delete operations. Remove obsolete assertions that both operations are undefined.

In the new focused test, require:

- `roleAPI.listRoles({ include_inactive: true })` support;
- `listOrganizationUnits({ include_inactive: true })` support;
- default calls omit the query string;
- delete wrappers return `ApiResponse<EmptyData>` rather than a handwritten empty record;
- `Role` and `OrganizationUnit` generated schemas expose required `is_system: boolean`;
- the management pages opt into inactive records while assignment callers do not.

Run:

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs tests/static/access-soft-deactivation.test.mjs
```

Expected: FAIL before generation and wrapper changes.

### Step 2: Generate, do not hand-edit, artifacts

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
```

Inspect the diff to confirm only the expected operations, query parameters, response status entries, and `is_system` fields changed.

### Step 3: Update stable API wrappers

Use generated aliases and typed options:

```ts
type EmptyData = Schemas['EmptyData'];
type ManagedListOptions = { include_inactive?: boolean };
```

Build the query string with `URLSearchParams`, returning the unchanged route when the option is absent or false. Change delete wrappers to `ApiResponse<EmptyData>`.

### Step 4: Run tests and commit

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/api-response-contract.test.mjs tests/static/access-soft-deactivation.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/roles.ts \
  frontend-school/src/lib/api/staff.ts \
  frontend-school/tests/static/api-response-contract.test.mjs \
  frontend-school/tests/static/access-soft-deactivation.test.mjs
git commit -m "feat(frontend): type access deactivation APIs"
```

## Task 8: Update role management UX

**Files:**

- Modify: `frontend-school/src/routes/(app)/staff/roles/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/roles/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/access-soft-deactivation.test.mjs`

### Step 1: Add failing UI assertions

Require:

- the list calls `listRoles({ include_inactive: true })`;
- the action and dialog use `ปิดใช้งาน`, not permanent-delete wording;
- the dialog explains immediate permission loss and reversible restoration;
- `role.is_system` prevents deactivation UI;
- inactive roles expose `เปิดใช้งาน` to users with `roles.update.all`;
- active roles require `roles.delete.all` before switching off or using the button;
- the old `การกระทำนี้ไม่สามารถย้อนกลับได้` text is absent.

Run the focused Node test. Expected: FAIL.

### Step 2: Implement role UI state rules

- Load all management roles with `include_inactive`.
- Show a protected badge for `is_system`.
- Rename delete state/functions only where it improves clarity (`deactivating`, `showDeactivateDialog`); avoid unrelated page cleanup.
- Keep regular fields editable with `roles.update.all`.
- Disable a transition from active to inactive unless the actor also has `roles.delete.all`.
- Use the delete endpoint for the explicit deactivate action.
- Reactivate through the generated update request with `{ is_active: true }`.
- Refresh or navigate to the management list after success.

### Step 3: Validate Svelte and tests

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/roles/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/roles/[id]/+page.svelte' --svelte-version 5
node --test tests/static/access-soft-deactivation.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: no new autofixer issue and passing tests.

### Step 4: Commit

```bash
git add 'frontend-school/src/routes/(app)/staff/roles/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/roles/[id]/+page.svelte' \
  frontend-school/tests/static/access-soft-deactivation.test.mjs
git commit -m "feat(frontend): manage inactive roles safely"
```

## Task 9: Update organization management UX

**Files:**

- Modify: `frontend-school/src/routes/(app)/staff/organization/+page.svelte`
- Modify: `frontend-school/tests/static/access-soft-deactivation.test.mjs`
- Modify if necessary: `frontend-school/tests/static/organization-permission-ui.test.mjs`

### Step 1: Add failing UI assertions

Require:

- the overview calls `listOrganizationUnits({ include_inactive: true })`;
- it imports and calls `deleteOrganizationUnit`;
- active non-system units expose `ปิดใช้งาน` only with `roles.delete.all`;
- inactive units expose `เปิดใช้งาน` only with `roles.update.all`;
- system units do not expose deactivation;
- confirmation explains that organization-derived permissions stop but history remains;
- server conflict messages are shown without replacing them with a generic error.

Run the focused test. Expected: FAIL.

### Step 2: Implement the detail-panel actions

- Add `canDeleteOrganizationUnit` from the generated permission registry.
- Use `selectedUnit.is_system` as the authoritative protection flag; the existing root helper may retain its code/type fallback for tree behavior.
- Add shadcn-svelte confirmation dialog state for deactivation.
- On success, reload departments and preserve selection when possible.
- Reactivate with `updateOrganizationUnit(id, { is_active: true })`.
- Surface the backend conflict string for active-child and protected-record cases.
- Do not rewrite the page's unrelated existing member-loading `$effect` as part of this feature.

### Step 3: Validate Svelte and tests

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/organization/+page.svelte' --svelte-version 5
node --test tests/static/access-soft-deactivation.test.mjs tests/static/organization-permission-ui.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: no new autofixer issue. Pre-existing unrelated `$effect` suggestions may remain documented.

### Step 4: Commit

```bash
git add 'frontend-school/src/routes/(app)/staff/organization/+page.svelte' \
  frontend-school/tests/static/access-soft-deactivation.test.mjs \
  frontend-school/tests/static/organization-permission-ui.test.mjs
git commit -m "feat(frontend): manage inactive organization units"
```

## Task 10: Update documentation and checkpoint counts

**Files:**

- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `.rules`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

### Step 1: Make documentation tests fail on the new checkpoint

Update the static expectations from:

```text
66 unique operations / 30 auth-authorization / 36 read-oriented
```

to:

```text
68 unique operations / 32 auth-authorization / 36 read-oriented
```

Replace the test that requires an “unsupported delete discrepancy” with one that requires documented soft-deactivation semantics and the two implemented routes.

Run:

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs
```

Expected: FAIL until docs are updated.

### Step 2: Update repository guidance

- Mark M-6 complete in `IMPROVEMENT_PLAN.md` with behavior, security, cache, audit, frontend, and verification summary.
- Record 68/32/36 in `.rules`, `docs/TESTING.md`, and `API_DEVELOPMENT.md`.
- Document that role/unit `DELETE` means deactivation and that `is_system` is migration/provisioning-owned.
- Keep Phase 4 remaining mutation batches and SSE/WebSocket/binary exclusions explicit.

### Step 3: Run tests and commit

```bash
cd frontend-school
node --test tests/static/api-response-contract.test.mjs
npm run test:static
```

```bash
git add IMPROVEMENT_PLAN.md .rules docs/TESTING.md \
  docs/backend-school/API_DEVELOPMENT.md \
  frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "docs(api): record access deactivation contract"
```

## Task 11: Full verification and review handoff

**Files:** No planned production edits; only focused fixes if verification reveals a defect.

### Step 1: Verify generated and permission contracts

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
```

Expected: PASS with no generated drift.

### Step 2: Verify backend

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --bin backend-school
cargo test api_contract::tests --bin backend-school
cargo test --test static_architecture
cargo test --bin backend-school
```

Expected: all non-database checks pass. The existing auth database tests and the new status database tests require `TEST_DATABASE_URL`.

When credentials are available:

```bash
TEST_DATABASE_URL='postgresql://.../schoolorbit_test' \
  cargo test modules::staff::services::status_tests --bin backend-school -- --nocapture
```

Expected: PASS against an isolated test schema with migration 027 applied.

### Step 3: Verify frontend

```bash
cd frontend-school
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/roles/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/roles/[id]/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/organization/+page.svelte' --svelte-version 5
```

Expected: PASS. If repository-wide lint reports unrelated pre-existing files, isolate and report them; do not bulk-format unrelated code.

### Step 4: Inspect migration and generated diffs

```bash
git diff main...HEAD -- backend-school/migrations
git diff main...HEAD -- contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git diff --check
git status --short
```

Confirm:

- no existing migration changed;
- generated artifacts were produced by the generator;
- only two operations increased the checkpoint from 66 to 68;
- no plaintext personal or credential data appears in audit/test fixtures;
- worktree is clean after final commits.

### Step 5: Request review before integration

Use `superpowers:requesting-code-review` after all verification passes. Address only technically verified findings. Then use `superpowers:finishing-a-development-branch` to present/perform the user-approved integration path.

Do not merge or push merely because the implementation compiles; integration follows successful verification and review.
