# Admin API Proxy Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route `admin-api.schoolorbit.app` to backend-admin and make all-tenant deployment discover schools from the owning Backend Admin API.

**Architecture:** Keep one tracked Nginx server definition for the admin API, install it transactionally from the backend-admin deployment workflow, and verify the public service identity after reload. Keep tenant discovery in a tested fail-closed script that calls Backend Admin directly and retries only bounded rollout/transient failures.

**Tech Stack:** Nginx, GitHub Actions, Bash, Node.js test runner, Axum internal API.

## Global Constraints

- Do not expose or log `INTERNAL_API_SECRET` or internal API response bodies.
- Proxy to `schoolorbit-backend-admin:8080`, never `localhost` or backend-school.
- Preserve the active proxy when `nginx -t`, reload, or public identity verification fails.
- Do not modify backend or frontend application behavior.

---

### Task 1: Lock the proxy and discovery contracts

**Files:**

- Modify: `frontend-school/tests/static/deploy-all-tenant-discovery.test.mjs`
- Create: `frontend-school/tests/static/admin-api-proxy.test.mjs`

**Interfaces:**

- Consumes: `scripts/discover_school_tenants.sh`, `.github/workflows/deploy-backend-admin.yml`, and the tracked admin proxy.
- Produces: regression coverage for the Backend Admin endpoint, caller header, proxy upstream, installation validation, and fail-closed behavior.

- [ ] **Step 1: Change the discovery success test**

Require `BACKEND_ADMIN_URL`, request `/internal/schools?status=active`, send caller `deploy-all-schools`, and publish only validated subdomains.

- [ ] **Step 2: Add the proxy installation test**

Assert the tracked server is `admin-api.schoolorbit.app`, proxies to `schoolorbit-backend-admin:8080`, and that the backend-admin workflow stages, validates, reloads, verifies, and restores the config on failure.

- [ ] **Step 3: Run the focused tests**

Run:

```bash
node --test frontend-school/tests/static/deploy-all-tenant-discovery.test.mjs frontend-school/tests/static/admin-api-proxy.test.mjs
```

Expected: FAIL because discovery still calls backend-school and no tracked admin proxy exists.

### Task 2: Install the correct admin proxy safely

**Files:**

- Move: `backend-admin/nginx.conf.example` to `nginx-configs/admin-api.schoolorbit.app.conf`
- Modify: `.github/workflows/deploy-backend-admin.yml`
- Modify: `docs/OPERATIONS.md`
- Modify: `docs/PODMAN_SETUP.md`

**Interfaces:**

- Consumes: `/opt/stack/nginx/conf.d`, `schoolorbit-nginx`, and `schoolorbit-backend-admin`.
- Produces: a public `https://admin-api.schoolorbit.app` whose root identifies Backend Admin and whose internal routes reach the protected Axum handlers.

- [ ] **Step 1: Replace the stale example with the production proxy**

Use the shared certificate path documented for the combined API certificate, a dedicated CORS map, `server_name admin-api.schoolorbit.app`, and `proxy_pass http://schoolorbit-backend-admin:8080`.

- [ ] **Step 2: Stage and install the proxy**

Checkout and upload the tracked config. Resolve zero or one existing matching config, back it up when present, install the new file, run `nginx -t`, reload, and restore the prior state on failure.

- [ ] **Step 3: Verify the public identity**

Poll the public root until it contains `"service":"SchoolOrbit Backend Admin"`; restore the prior config if verification never succeeds.

- [ ] **Step 4: Update canonical operations references**

Point both operations documents to `nginx-configs/admin-api.schoolorbit.app.conf`.

### Task 3: Call Backend Admin directly from Deploy All

**Files:**

- Modify: `scripts/discover_school_tenants.sh`
- Modify: `.github/workflows/deploy-all-schools.yml`

**Interfaces:**

- Consumes: `BACKEND_ADMIN_URL`, `INTERNAL_API_SECRET`, and `GET /internal/schools?status=active`.
- Produces: `schools=<compact JSON matrix>` in `GITHUB_OUTPUT`.

- [ ] **Step 1: Change the discovery endpoint**

Use `BACKEND_ADMIN_URL`, call the owning internal route, and keep response validation and redaction.

- [ ] **Step 2: Add bounded rollout retry**

Retry connection failures and `404`, `429`, and `5xx` responses with a fixed bounded delay; fail immediately for authentication and other permanent responses.

- [ ] **Step 3: Wire the workflow secret**

Pass `secrets.BACKEND_ADMIN_URL` and keep checkout plus the tested discovery script.

- [ ] **Step 4: Run focused tests**

Run the two tests from Task 1 and expect PASS.

### Task 4: Verify the complete change

**Files:**

- Verify all files above.

**Interfaces:**

- Consumes: the completed proxy and discovery changes.
- Produces: evidence that the repository is ready for an ordered backend-admin then tenant deployment.

- [ ] **Step 1: Run shell and formatting checks**

```bash
bash -n scripts/discover_school_tenants.sh
cd frontend-school
npx prettier --check ../.github/workflows/deploy-backend-admin.yml ../.github/workflows/deploy-all-schools.yml ../nginx-configs/admin-api.schoolorbit.app.conf tests/static/admin-api-proxy.test.mjs tests/static/deploy-all-tenant-discovery.test.mjs
```

- [ ] **Step 2: Run frontend verification**

```bash
npm run lint
npm run test:static
```

- [ ] **Step 3: Review repository state**

```bash
git diff --check
git diff
git status --short
```
