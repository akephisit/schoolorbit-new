# Teaching Supervision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the first release of the teaching supervision system from the approved design spec.

**Architecture:** Add a dedicated `supervision` domain module rather than overloading generic work/workflow. Backend owns schema, resource-aware policy, services, typed handlers, and permission registry. Frontend adds one academic workspace route with typed API client, permission metadata, and role-aware panels for teacher booking, request approval, evaluator scoring, acknowledgement, and reports.

**Tech Stack:** Rust Axum, sqlx/PostgreSQL, SvelteKit 5 CSR, TypeScript, Tailwind/shadcn-svelte, static architecture tests.

---

## File Map

- Create `backend-school/migrations/005_teaching_supervision.sql`: schema, indexes, comments.
- Modify `backend-school/src/permissions/registry.rs`: add supervision permissions.
- Create `backend-school/src/policies/supervision_access_policy.rs`: resource access helpers.
- Modify `backend-school/src/policies.rs`: export policy.
- Create `backend-school/src/modules/supervision.rs`: route root.
- Create `backend-school/src/modules/supervision/models.rs`: typed DTOs and row models.
- Create `backend-school/src/modules/supervision/services.rs`: DB services and pure workflow helpers.
- Create `backend-school/src/modules/supervision/handlers.rs`: thin Axum handlers.
- Modify `backend-school/src/modules.rs`: export module.
- Modify `backend-school/src/main.rs`: mount `/api/supervision`.
- Modify `backend-school/tests/static_architecture.rs`: guard new module and SSE-free assumptions.
- Modify `frontend-school/src/lib/permissions/registry.ts`: add supervision permissions/module.
- Create `frontend-school/src/lib/api/supervision.ts`: typed API client.
- Create `frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts`: menu metadata.
- Create `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`: first-release UI.
- Modify `frontend-school/tests/static/api-response-contract.test.mjs`: guard typed supervision API/page contract.

## Task 1: Schema And Permission Registry

- [x] **Step 1: Write failing backend static test**

Add assertions in `backend-school/tests/static_architecture.rs` that expect:

```rust
let registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
assert!(registry.contains("SUPERVISION_REQUEST_OWN"));
assert!(registry.contains("supervision.request.own"));
assert!(registry.contains("SUPERVISION_APPROVE_SCHOOL"));

let modules = read_source(manifest_dir().join("src/modules.rs"));
assert!(modules.contains("pub mod supervision;"));
```

- [x] **Step 2: Verify RED**

Run: `cd backend-school && cargo test --test static_architecture supervision`

Expected: FAIL because supervision registry/module does not exist.

- [x] **Step 3: Add schema and permission registry**

Create migration `backend-school/migrations/005_teaching_supervision.sql` with the supervision tables from the design. Add permission constants and `PermissionDef` entries for:

```rust
SUPERVISION_READ_OWN
SUPERVISION_READ_ASSIGNED
SUPERVISION_READ_ORGANIZATION_UNIT
SUPERVISION_READ_ORGANIZATION_TREE
SUPERVISION_READ_SCHOOL
SUPERVISION_REQUEST_OWN
SUPERVISION_MANAGE_SCHOOL
SUPERVISION_EVALUATE_ASSIGNED
SUPERVISION_APPROVE_SCHOOL
```

- [x] **Step 4: Add module root placeholders**

Create module root files with exports so compile can discover the module:

```rust
// backend-school/src/modules/supervision.rs
pub mod handlers;
pub mod models;
pub mod services;

use axum::Router;
use crate::AppState;

pub fn supervision_routes() -> Router<AppState> {
    handlers::routes()
}
```

- [x] **Step 5: Verify GREEN**

Run: `cd backend-school && cargo test --test static_architecture supervision`

Expected: PASS for the new static checks.

- [x] **Step 6: Commit**

Run:

```bash
git add backend-school/migrations/005_teaching_supervision.sql backend-school/src/permissions/registry.rs backend-school/src/modules.rs backend-school/src/modules/supervision.rs backend-school/tests/static_architecture.rs
git commit -m "feat: add teaching supervision schema"
```

## Task 2: Backend Supervision Logic, Policy, And Services

- [ ] **Step 1: Write failing service tests**

Add tests in `backend-school/src/modules/supervision/services.rs` for pure helpers:

- target specificity prefers staff over subject group over organization unit over school
- teachers may edit requests only in `requested`
- average rating uses equal weights and ignores missing/non-rating values
- all required evaluators must submit before review
- invalid status transitions are rejected

- [ ] **Step 2: Verify RED**

Run: `cd backend-school && cargo test modules::supervision::services::tests --bin backend-school`

Expected: FAIL because helper functions/types are missing.

- [ ] **Step 3: Implement typed models and pure helpers**

Implement enums and structs in `models.rs` and pure helper functions in `services.rs`.

- [ ] **Step 4: Implement DB service functions**

Implement first-release service functions:

```rust
list_cycles
create_cycle
update_cycle
list_templates
create_template
update_template
get_template
list_observations
get_observation
request_observation
update_requested_observation
cancel_requested_observation
approve_observation_request
return_observation_request
save_my_evaluation
submit_my_evaluation
submit_observation_for_review
approve_observation
return_observation
publish_observation
acknowledge_observation
cycle_progress
```

- [ ] **Step 5: Implement resource policy**

Create `backend-school/src/policies/supervision_access_policy.rs` with helpers for:

```rust
can_manage_school
can_request_own
can_evaluate_assigned
can_approve_school
resolve_observation_read_access
```

- [ ] **Step 6: Verify GREEN**

Run: `cd backend-school && cargo test modules::supervision::services::tests --bin backend-school`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add backend-school/src/modules/supervision/models.rs backend-school/src/modules/supervision/services.rs backend-school/src/policies.rs backend-school/src/policies/supervision_access_policy.rs
git commit -m "feat: add teaching supervision services"
```

## Task 3: Backend Handlers And Routes

- [ ] **Step 1: Write failing static architecture checks**

Extend `backend-school/tests/static_architecture.rs` to assert the new handler:

```rust
let handler = read_source(manifest_dir().join("src/modules/supervision/handlers.rs"));
assert!(handler.contains("actor_tenant_context"));
assert!(handler.contains("ApiResponse::ok"));
assert!(!handler.contains("sqlx::query"));
```

- [ ] **Step 2: Verify RED**

Run: `cd backend-school && cargo test --test static_architecture supervision`

Expected: FAIL until handlers exist and match the architecture.

- [ ] **Step 3: Implement handlers**

Implement typed request/response handlers using `actor_tenant_context`, policy checks, service calls, and `ApiResponse`.

- [ ] **Step 4: Mount routes**

Modify `backend-school/src/main.rs`:

```rust
.nest(
    "/api/supervision",
    modules::supervision::supervision_routes()
        .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)),
)
```

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cd backend-school
cargo test --test static_architecture supervision
cargo check
```

Expected: both commands pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add backend-school/src/modules/supervision/handlers.rs backend-school/src/main.rs backend-school/tests/static_architecture.rs
git commit -m "feat: add teaching supervision API"
```

## Task 4: Frontend API Contract And Route Shell

- [ ] **Step 1: Write failing frontend static test**

Add static checks in `frontend-school/tests/static/api-response-contract.test.mjs` expecting:

```js
const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
const supervisionRoute = await readRepoFile('frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts');
const supervisionPage = await readRepoFile('frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte');

assert.match(supervisionApi, /export\s+type\s+SupervisionObservationStatus/);
assert.match(supervisionApi, /apiClient\.get<\{\s*items:\s*SupervisionCycle\[\]\s*\}>/);
assert.match(supervisionApi, /apiClient\.post<SupervisionObservation>/);
assert.doesNotMatch(supervisionApi, /ApiResponse<unknown>/);
assert.doesNotMatch(supervisionApi, /Record<string,\s*unknown>/);
assert.match(supervisionRoute, /PERMISSION_MODULES\.SUPERVISION/);
assert.match(supervisionPage, /listSupervisionCycles/);
```

- [ ] **Step 2: Verify RED**

Run: `cd frontend-school && npm run test:static`

Expected: FAIL because frontend supervision files do not exist.

- [ ] **Step 3: Add frontend registry and typed API**

Add `SUPERVISION` to `PERMISSION_MODULES`, add the supervision permission constants, and create `src/lib/api/supervision.ts` with typed DTOs and API calls.

- [ ] **Step 4: Add route metadata**

Create `frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts` with `_meta.menu` using `PERMISSION_MODULES.SUPERVISION`.

- [ ] **Step 5: Verify GREEN**

Run: `cd frontend-school && npm run test:static`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add frontend-school/src/lib/permissions/registry.ts frontend-school/src/lib/api/supervision.ts frontend-school/src/routes/'(app)'/staff/academic/supervision/+page.ts frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "feat: add teaching supervision frontend contract"
```

## Task 5: Frontend Supervision UI

- [ ] **Step 1: Write failing static checks for UI flows**

Extend the supervision static test to expect the page uses:

```js
assert.match(supervisionPage, /requestSupervisionObservation/);
assert.match(supervisionPage, /approveSupervisionObservationRequest/);
assert.match(supervisionPage, /saveMySupervisionEvaluation/);
assert.match(supervisionPage, /acknowledgeSupervisionObservation/);
assert.match(supervisionPage, /getMyTimetable/);
assert.doesNotMatch(supervisionPage, /\bfetch\s*\(/);
```

- [ ] **Step 2: Verify RED**

Run: `cd frontend-school && npm run test:static`

Expected: FAIL until the Svelte page implements the flows.

- [ ] **Step 3: Implement Svelte page**

Build one route page with tabs:

- `ของฉัน`
- `คำขอจอง`
- `ประเมิน`
- `รอบนิเทศ`
- `แบบประเมิน`
- `รายงาน`

The page must use typed API functions, shadcn-svelte primitives, and `can` store gating. Do not use raw `fetch`.

- [ ] **Step 4: Verify frontend**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add frontend-school/src/routes/'(app)'/staff/academic/supervision/+page.svelte frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "feat: add teaching supervision workspace"
```

## Task 6: Final Verification

- [ ] **Step 1: Run backend checks**

Run:

```bash
cd backend-school
cargo test modules::supervision::services::tests --bin backend-school
cargo test --test static_architecture
cargo check
```

- [ ] **Step 2: Run frontend checks**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

- [ ] **Step 3: Repository checks**

Run from repo root:

```bash
git diff --check
git status -sb
```

- [ ] **Step 4: Push if all checks pass**

Run:

```bash
git push origin main
```
