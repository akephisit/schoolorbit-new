# Auth and Authorization API Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the backend-owned OpenAPI contract from the `/api/auth/me` pilot to every implemented auth, role, permission, user-role, organization-permission, delegation, and organization-member JSON endpoint, then replace matching handwritten frontend DTOs with generated types.

**Architecture:** Rust response/request structs and Axum handlers remain the source of truth. `utoipa` derives schemas and operations, the existing deterministic exporter writes `contracts/openapi/school-api.json`, and `openapi-typescript` generates frontend transport DTOs. Frontend API wrappers keep their domain-facing interfaces where mapping is useful, but transport shapes come from `components['schemas']`.

**Tech Stack:** Rust, Axum, serde, utoipa 5.5, Node.js, openapi-typescript, TypeScript, SvelteKit 5.

## Global Constraints

- Do not change endpoint behavior while documenting it.
- Do not add undocumented backend delete semantics for roles or organization units.
- JSON names, nullability, required fields, status codes, and envelope types must match current serde/runtime behavior.
- `national_id` remains encrypted at rest and must never be logged in plaintext.
- OpenAPI export and generated TypeScript must remain deterministic and failure-atomic.
- Each operation gets a unique camelCase `operationId`.
- Existing permissions remain sourced from `contracts/permissions.json`; this plan documents permission APIs but does not create a second permission registry.

## Contract Inventory

This phase covers 29 implemented operations in addition to the completed `GET /api/auth/me` pilot:

| Area | Operations |
|---|---|
| Auth | `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/me/profile`, `PUT /api/auth/me/profile`, `POST /api/auth/me/change-password` |
| Roles | `GET /api/roles`, `GET /api/roles/{id}`, `POST /api/roles`, `PUT /api/roles/{id}` |
| User roles | `GET /api/users/{id}/roles`, `POST /api/users/{id}/roles`, `DELETE /api/users/{id}/roles/{role_id}`, `GET /api/users/{id}/permissions` |
| Permissions | `GET /api/permissions`, `GET /api/permissions/modules` |
| Organization units | `GET /api/organization/units`, `GET /api/organization/units/{id}`, `POST /api/organization/units`, `PUT /api/organization/units/{id}` |
| Organization permissions | `GET /api/organization/units/{id}/permissions`, `PUT /api/organization/units/{id}/permissions` |
| Delegations | `GET /api/organization/units/{id}/delegatable-permissions`, `GET /api/organization/units/{id}/delegations`, `POST /api/organization/units/{id}/delegations`, `DELETE /api/organization/delegations/{id}` |
| Members | `GET /api/organization/units/{id}/members`, `POST /api/organization/units/{id}/members`, `PUT /api/organization/units/{id}/members/{user_id}`, `DELETE /api/organization/units/{id}/members/{user_id}` |

Two frontend-only calls are explicitly outside this contract because no backend route exists:

- `DELETE /api/roles/{id}` is actively exposed by the roles UI. Keep it recorded as a behavior discrepancy; do not invent deletion rules in a contract-only change.
- `DELETE /api/organization/units/{id}` exists as an unused API helper. Keep it out of OpenAPI and remove or implement it only in a separately reviewed behavior change.

---

### Task 1: Shared contract schemas and coverage gates

**Files:**
- Modify: `backend-school/src/api_response.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `ApiResponse<T>`, `ApiErrorResponse`, `EmptyData`, and `IdData<T>`.
- Produces: `ToSchema` implementations for empty and identifier envelopes plus reusable test helpers for later operation inventory checks.

- [ ] **Step 1: Write failing shared-envelope tests**

Add exact schema assertions for `ApiResponse_EmptyData` and `ApiResponse_IdData_Uuid`. Add reusable helpers that accept an expected `(path, method, operationId)` slice and assert each operation exists.

- [ ] **Step 2: Prove the tests fail before implementation**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture`

Expected: FAIL because the empty/id component schemas are absent.

- [ ] **Step 3: Make envelope DTOs schema-capable**

Change the existing derives without changing serialization:

```rust
#[derive(Debug, Default, Serialize, ToSchema)]
pub struct EmptyData {}

#[derive(Debug, Serialize, ToSchema)]
pub struct IdData<T> {
    pub id: T,
}
```

Do not add the complete inventory assertion yet; each later task adds and satisfies its own operation slice before committing.

- [ ] **Step 4: Run focused tests**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture && cargo test --test static_architecture`

Expected: shared envelope schema assertions PASS.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/api_response.rs backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "test(api): define authorization contract inventory"
```

### Task 2: Auth request/response contracts

**Files:**
- Modify: `backend-school/src/modules/auth/models.rs`
- Modify: `backend-school/src/modules/auth/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Consumes: `ApiResponse<T>`, `ApiErrorResponse`, `EmptyData`, existing JWT/cookie behavior.
- Produces: schemas `LoginRequest`, `LoginData`, `ProfileResponse`, `UpdateProfileRequest`, `ChangePasswordRequest`; operation IDs `login`, `logout`, `getCurrentUserProfile`, `updateCurrentUserProfile`, `changeCurrentUserPassword`.

- [ ] **Step 1: Add failing auth shape assertions**

Assert login requires `username` and `password`, keeps `rememberMe` optional, and returns `ApiResponse_LoginData`. Assert profile response fields serialized as null are required-and-nullable while `primaryRoleName` remains optional because serde omits it. Assert update/change-password request properties use camelCase.

- [ ] **Step 2: Run the focused test and observe RED**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture`

Expected: FAIL because auth schemas and paths are not exported.

- [ ] **Step 3: Derive schemas and annotate response nullability**

Add `ToSchema` to the five auth DTOs. For every `ProfileResponse: Option<_>` field that is always serialized, add `#[schema(required = true)]`; retain `#[serde(skip_serializing_if = "Option::is_none")]` and non-null schema semantics for `primary_role_name`.

- [ ] **Step 4: Annotate and register the five handlers**

Use these exact success contracts:

```text
login: 200 ApiResponse<LoginData>, body LoginRequest
logout: 200 ApiResponse<EmptyData>
getCurrentUserProfile: 200 ApiResponse<ProfileResponse>
updateCurrentUserProfile: 200 ApiResponse<ProfileResponse>, body UpdateProfileRequest
changeCurrentUserPassword: 200 ApiResponse<EmptyData>, body ChangePasswordRequest
```

Declare current runtime error envelopes with `ApiErrorResponse`, then add all five handlers and schemas to `SchoolApiDoc`.

- [ ] **Step 5: Verify and commit**

Run: `cd backend-school && cargo fmt --all -- --check && cargo test api_contract::tests -- --nocapture`

Expected: all auth assertions PASS.

```bash
git add backend-school/src/modules/auth backend-school/src/api_contract.rs
git commit -m "feat(api): export auth contracts"
```

### Task 3: Roles, permissions, and user-role contracts

**Files:**
- Modify: `backend-school/src/modules/staff/models.rs`
- Modify: `backend-school/src/modules/staff/handlers/roles.rs`
- Modify: `backend-school/src/modules/staff/handlers/permissions.rs`
- Modify: `backend-school/src/modules/staff/handlers/user_roles.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Consumes: existing role/permission services and registry checks unchanged.
- Produces: exact schemas for `Role`, `Permission`, `CreateRoleRequest`, `UpdateRoleRequest`, `AssignRoleRequest`, `UserRoleAssignmentResponse`; 10 registered operations.

- [ ] **Step 1: Add failing assertions for the ten operations**

Assert path parameters are UUID strings, role create returns HTTP 201 with `ApiResponse_IdData_Uuid`, mutation success uses `ApiResponse_EmptyData`, assign-role documents 201/400/404, and list-by-module uses an object with permission-array values.

- [ ] **Step 2: Run RED test**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture`

Expected: FAIL on the first missing role operation/schema.

- [ ] **Step 3: Derive exact DTO schemas**

Add `ToSchema` to the six DTOs. Response-side optional fields that serde always emits are required-and-nullable: `Role.name_en`, `Role.description`, `UserRoleAssignmentResponse.organization_unit_id`, `ended_at`, and `notes`; request-side `Option` fields stay optional.

- [ ] **Step 4: Add handler contracts**

Register these operation IDs and success bodies:

```text
listRoles -> ApiResponse<Vec<Role>>
getRole -> ApiResponse<Role>
createRole -> 201 ApiResponse<IdData<Uuid>>
updateRole -> ApiResponse<EmptyData>
listPermissions -> ApiResponse<Vec<Permission>>
listPermissionsByModule -> ApiResponse<HashMap<String, Vec<Permission>>>
getUserRoles -> ApiResponse<Vec<UserRoleAssignmentResponse>>
assignUserRole -> 201 ApiResponse<IdData<Uuid>>
removeUserRole -> ApiResponse<EmptyData>
getUserPermissions -> ApiResponse<Vec<String>>
```

Document 401/403 for protected operations and the already implemented 400/404 outcomes. Do not add `deleteRole`.

- [ ] **Step 5: Verify and commit**

Run: `cd backend-school && cargo fmt --all -- --check && cargo test api_contract::tests -- --nocapture`

Expected: role/permission assertions PASS.

```bash
git add backend-school/src/modules/staff/models.rs backend-school/src/modules/staff/handlers/{roles,permissions,user_roles}.rs backend-school/src/api_contract.rs
git commit -m "feat(api): export role and permission contracts"
```

### Task 4: Organization unit and grant contracts

**Files:**
- Modify: `backend-school/src/modules/staff/models.rs`
- Modify: `backend-school/src/modules/staff/services/organization_permission_service.rs`
- Modify: `backend-school/src/modules/staff/handlers/roles.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_permissions.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Consumes: current organization services and permission-cache invalidation.
- Produces: `OrganizationUnit`, create/update request, grant input/output schemas; six registered operations.

- [ ] **Step 1: Add failing organization contract assertions**

Assert UUID path parameters, 201 identifier response on create, empty response on updates, request bodies, and `position_code` nullability for permission grants.

- [ ] **Step 2: Run RED test**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture`

Expected: FAIL because organization paths are absent.

- [ ] **Step 3: Add exact schemas**

Derive `ToSchema` for `OrganizationUnit`, `CreateOrganizationUnitRequest`, `UpdateOrganizationUnitRequest`, `OrganizationPermissionGrantInput`, `UpdateOrganizationPermissionsRequest`, and `OrganizationPermissionGrant`. Mark every always-serialized optional response property required-and-nullable.

- [ ] **Step 4: Register operations**

Use operation IDs `listOrganizationUnits`, `getOrganizationUnit`, `createOrganizationUnit`, `updateOrganizationUnit`, `getOrganizationPermissions`, and `updateOrganizationPermissions`. Declare success envelopes from the actual handlers and 401/403/404 error envelopes. Do not add `deleteOrganizationUnit`.

- [ ] **Step 5: Verify and commit**

Run: `cd backend-school && cargo fmt --all -- --check && cargo test api_contract::tests -- --nocapture`

Expected: all organization unit/grant assertions PASS.

```bash
git add backend-school/src/modules/staff backend-school/src/api_contract.rs
git commit -m "feat(api): export organization unit contracts"
```

### Task 5: Delegation and organization-member contracts

**Files:**
- Modify: `backend-school/src/modules/staff/handlers/organization_delegations.rs`
- Modify: `backend-school/src/modules/staff/handlers/organization_members.rs`
- Modify: `backend-school/src/modules/staff/services/organization_delegation_service.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Consumes: policy checks and cache invalidation unchanged.
- Produces: delegation/member request, query, response schemas; eight registered operations.

- [ ] **Step 1: Add failing final-inventory and path/query/body assertions**

Add the complete expected 30-operation `(path, method, operationId)` table, prove every operation exists, and prove operation IDs are unique with a `HashSet`. Assert `include_children` is an optional boolean query, all IDs are UUID path/body properties, delegation timestamps are RFC 3339 date-time strings, member `started_at` is a date, and each mutation has the current empty/id envelope. Add a static architecture assertion that every auth/authorization handler registered in the router appears in `SchoolApiDoc`.

- [ ] **Step 2: Run RED test**

Run: `cd backend-school && cargo test api_contract::tests -- --nocapture`

Expected: FAIL on missing delegation/member operations.

- [ ] **Step 3: Derive schemas**

Add `ToSchema` to `DelegationItem`, `CreateDelegationRequest`, `DelegationIdData`, `DelegatablePermission`, `OrganizationMemberItem`, `ListMembersQuery`, `AddMemberRequest`, and `UpdateMemberRequest`. Publicize `DelegationIdData` fields only as required by schema generation, not application behavior. Mark always-emitted optional response properties required-and-nullable.

- [ ] **Step 4: Register eight operations**

Use IDs `listDelegatablePermissions`, `listOrganizationDelegations`, `createOrganizationDelegation`, `revokeOrganizationDelegation`, `listOrganizationMembers`, `addOrganizationMember`, `updateOrganizationMember`, and `removeOrganizationMember`. Verify both PUT and DELETE methods on `/members/{user_id}` independently.

Success bodies must be `ApiResponse<Vec<DelegatablePermission>>`, `ApiResponse<Vec<DelegationItem>>`, `ApiResponse<DelegationIdData>`, `ApiResponse<EmptyData>`, and `ApiResponse<Vec<OrganizationMemberItem>>` as applicable. Declare implemented 400/401/403/404 errors.

- [ ] **Step 5: Verify and commit**

Run: `cd backend-school && cargo fmt --all -- --check && cargo test api_contract::tests -- --nocapture && cargo test --test static_architecture`

Expected: complete 30-operation inventory and operation-ID uniqueness PASS.

```bash
git add backend-school/src/modules/staff backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "feat(api): export organization access contracts"
```

### Task 6: Generate and adopt auth/authorization DTOs in the frontend

**Files:**
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/auth.ts`
- Modify: `frontend-school/src/lib/api/roles.ts`
- Modify: `frontend-school/src/lib/api/staff.ts`
- Modify: `frontend-school/tests/static/api-contracts.test.ts`
- Modify: `scripts/tests/generate-api-contracts.test.mjs`

**Interfaces:**
- Consumes: exported `components['schemas']` and existing `apiClient` envelope unwrapping.
- Produces: generated transport aliases for all schemas in this phase; no handwritten duplicate transport DTOs in auth/roles/organization APIs.

- [ ] **Step 1: Add failing frontend duplication/shape guards**

Assert generated paths include all 30 operations, generated request/response schemas are non-`unknown`, and `auth.ts`, `roles.ts`, and the organization section of `staff.ts` import `components` instead of declaring duplicate transport interfaces. Assert the two unsupported DELETE paths are absent from the OpenAPI document.

- [ ] **Step 2: Run RED tests**

Run: `cd frontend-school && npm run test:api-contracts && npm run test:static`

Expected: FAIL because generated contracts and aliases for the new operations are absent.

- [ ] **Step 3: Generate artifacts**

Run: `cd frontend-school && npm run generate:api-contracts`

Expected: both tracked generated files update atomically.

- [ ] **Step 4: Replace transport DTO declarations**

Import `components` and define explicit aliases such as:

```ts
type Schemas = components['schemas'];
type LoginRequestDto = Schemas['LoginRequest'];
export type Role = Schemas['Role'];
export type Permission = Schemas['Permission'];
export type OrganizationUnit = Schemas['OrganizationUnit'];
```

Use generated request aliases at API call boundaries. Preserve explicit DTO-to-domain mapping for current-user state, and preserve public exports where UI code imports these types.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cd frontend-school
npm run test:api-contracts
npm run check:api-contracts
npm run check
PUBLIC_API_URL=http://127.0.0.1:3000 PUBLIC_WS_URL=ws://127.0.0.1:3000/ws npm run test:static
```

Expected: contract test PASS, generated files clean, Svelte check reports 0 errors/0 warnings, static tests PASS.

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api frontend-school/tests scripts/tests
git commit -m "refactor(api): consume authorization contract DTOs"
```

### Task 7: Document discrepancies and run the phase gate

**Files:**
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `docs/TESTING.md`
- Modify: `IMPROVEMENT_PLAN.md`

**Interfaces:**
- Consumes: completed 30-operation contract and current CI generator gate.
- Produces: contributor instructions plus explicit follow-up entries for frontend-only delete calls.

- [ ] **Step 1: Add documentation checks**

Extend static tests to require documentation of unsupported role/unit delete calls and the rule that the OpenAPI exporter describes implemented routes only.

- [ ] **Step 2: Update contributor and improvement docs**

Record the 30-operation phase, generated DTO import pattern, commands, and these separate behavior decisions: define safe role deletion/deactivation semantics for the active roles UI; remove or implement the unused organization-unit deletion helper after dependency analysis.

- [ ] **Step 3: Run full phase verification**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test static_architecture
cargo test --lib

cd ../frontend-school
npm run test:api-contracts
npm run check:api-contracts
npm run check
PUBLIC_API_URL=http://127.0.0.1:3000 PUBLIC_WS_URL=ws://127.0.0.1:3000/ws npm run test:static
PUBLIC_API_URL=http://127.0.0.1:3000 PUBLIC_WS_URL=ws://127.0.0.1:3000/ws npm run build
```

Expected: all non-database checks PASS. If `TEST_DATABASE_URL` is absent, report database-backed login tests as not run rather than passed. Run smoke/E2E only when their documented credentials are available.

- [ ] **Step 4: Commit and review**

```bash
git add docs IMPROVEMENT_PLAN.md backend-school/tests/static_architecture.rs
git commit -m "docs(api): record authorization contract rollout"
```

Request a code review focused on serde/OpenAPI parity, unsupported-route handling, generated DTO adoption, and deterministic generation. Resolve Important findings, rerun affected gates, then use `superpowers:verification-before-completion` before claiming the phase complete.
