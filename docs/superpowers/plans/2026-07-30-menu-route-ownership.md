# Menu Route Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Limit route synchronization to frontend-owned menu records while preserving school customization and making synchronization an explicit failing deployment step.

**Architecture:** A forward migration adds constrained ownership to `menu_items`. Backend synchronization validates and applies one transactional desired-state diff, while a dedicated frontend command scans and registers routes after deployment.

**Tech Stack:** PostgreSQL/SQLx, Rust/Axum, Node.js/TypeScript, Vite, GitHub Actions

## Global Constraints

- Never edit an applied migration; add `033_menu_route_ownership.sql`.
- Route identity and permission remain system-owned; label, icon, placement, active state, and ordering remain school-owned.
- Do not expose the deploy key to Vite client variables or Cloudflare Worker runtime variables.
- Use `TEST_DATABASE_URL` with isolated schemas for database-backed tests.

---

### Task 1: Transactional route ownership

**Files:**

- Create: `backend-school/migrations/033_menu_route_ownership.sql`
- Modify: `backend-school/src/modules/system/services/route_registration_service.rs`

**Interfaces:**

- Consumes: `RouteRegistration.routes`
- Produces: `sync_routes(&PgPool, &RouteRegistration) -> Result<RouteRegistrationOutcome, AppError>`

- [ ] **Step 1: Write failing database tests**

Add tests that create isolated schemas, simulate the future ownership column when absent, and assert preservation, scoped cleanup, rollback, and invalid desired-state rejection against the real service.

- [ ] **Step 2: Verify the tests fail for the current unscoped behavior**

Run:

```bash
cd backend-school
cargo test modules::system::services::route_registration_service::tests --bin backend-school -- --nocapture
```

Expected: assertions fail because school-owned rows are deleted, per-route errors are swallowed, or empty/duplicate scans are accepted.

- [ ] **Step 3: Add the forward migration**

Add `managed_by varchar(20) NOT NULL DEFAULT 'school'`, constrain it to `frontend`, `school`, or `integration`, document it, and add a partial cleanup index for frontend-owned rows.

- [ ] **Step 4: Implement the transactional desired-state diff**

Validate non-empty unique route codes before opening a transaction. Upsert active routes with `managed_by = 'frontend'`, preserve school-owned fields, propagate every error, delete only stale `frontend` rows, and commit only after all steps succeed.

- [ ] **Step 5: Verify focused tests pass**

Run the focused backend test command from Step 2 and confirm every ownership and rollback assertion passes.

### Task 2: Explicit fail-closed route scan command

**Files:**

- Create: `frontend-school/scripts/register-menu-routes.ts`
- Create: `frontend-school/tests/runtime/menu-route-registration.test.mjs`
- Modify: `frontend-school/scripts/menu-helpers.ts`
- Modify: `frontend-school/package.json`
- Modify: `frontend-school/vite.config.ts`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/modules/system/handlers/register_routes.rs`

**Interfaces:**

- Consumes: `PUBLIC_BACKEND_URL`, `DEPLOY_KEY`, `SUBDOMAIN`, and route `_meta.menu`
- Produces: `npm run sync:menu-routes`

- [ ] **Step 1: Write failing runtime tests**

Use temporary route trees and a local HTTP server to assert malformed metadata rejects, an empty scan rejects, a complete payload is sent with deployment headers, and a non-success response rejects.

- [ ] **Step 2: Verify runtime tests fail**

Run:

```bash
cd frontend-school
node --experimental-strip-types --test tests/runtime/menu-route-registration.test.mjs
```

Expected: failure because the explicit registration module and fail-closed scanner do not exist.

- [ ] **Step 3: Implement the scanner and registration command**

Make metadata parse failures throw with a file-specific error, validate non-empty unique routes,
post them to the new ownership-aware `/api/admin/routes/sync` endpoint without logging secrets,
and exit non-zero for any failure. Do not retain the legacy endpoint path, so an old backend
fails safely without applying its unscoped cleanup.

- [ ] **Step 4: Remove the Vite build side effect**

Delete `menuRegistryPlugin` and its Vite registration. Add `sync:menu-routes` and the focused runtime test command to `package.json`.

- [ ] **Step 5: Verify runtime and static tests**

Run:

```bash
cd frontend-school
npm run test:menu-sync
npm run test:static
```

Expected: all tests pass.

### Task 3: Deployment and operational contract

**Files:**

- Modify: `.github/workflows/deploy-school-tenant.yml`
- Modify: `.github/workflows/deploy-all-schools.yml`
- Modify: `frontend-school/.env.example`
- Modify: `frontend-school/README.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `TODO.md`

**Interfaces:**

- Consumes: deployed tenant, backend URL, deploy secret, subdomain
- Produces: post-deployment menu synchronization that fails the workflow on error

- [ ] **Step 1: Add a failing workflow contract test**

Add a static test that requires an explicit post-deploy `npm run sync:menu-routes` step and forbids `VITE_DEPLOY_KEY` in Vite and Worker runtime configuration.

- [ ] **Step 2: Verify the workflow contract fails**

Run the focused static test and confirm the current build-hook configuration violates it.

- [ ] **Step 3: Update both deployment workflows**

Remove `VITE_DEPLOY_KEY` from build and Worker variables. Run `npm run sync:menu-routes` after Cloudflare deployment with server-only `DEPLOY_KEY`, `PUBLIC_BACKEND_URL`, and `SUBDOMAIN`.

- [ ] **Step 4: Update canonical operational documentation**

Document the explicit post-deployment step and failure behavior, update the local environment template/README, and remove completed `SEC-009` from `TODO.md`.

- [ ] **Step 5: Run the applicable verification matrix**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::system::services::route_registration_service::tests --bin backend-school -- --nocapture
cargo test --test static_architecture
cargo check

cd ../frontend-school
npm run lint
npm run test:menu-sync
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check

cd ..
git diff --check
git status --short
```

Review the full diff and report any environment-dependent test that could not run.
