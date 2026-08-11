# Backend School Disposable Neon Child Branch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the manual Backend School Neon compatibility gate use ordinary disposable child branches inside the dedicated test-only project so branch creation, migration tests, and cleanup complete reliably.

**Architecture:** Keep the existing manually confirmed workflow, pinned Neon action, direct endpoint, unique run-attempt name, two-hour expiration, and exact-ID finalizer. Remove only the schema-only branch mode; the empty test-only parent supplies no production rows, while each Rust test creates an isolated schema and runs the active migrations itself.

**Tech Stack:** GitHub Actions YAML, Neon create-branch action v6.4.0, Node.js built-in test runner, Cargo/SQLx, actionlint.

## Global Constraints

- Work directly on the user-approved `main` branch; never force-push.
- Do not modify backend-admin, frontend-admin, frontend-school application code, or any migration.
- Keep the Neon project and parent branch test-only and free of production data.
- Keep the workflow `workflow_dispatch` only and require explicit boolean confirmation.
- Never print the API key, role password, or direct/pooled database URL.
- Consume only `steps.create_branch.outputs.db_url`; never consume `db_url_pooled`.
- Delete only a branch that this run reports as newly created, and retain the two-hour expiration fallback.
- Follow RED, GREEN, refactor, then fresh verification before commit and push.

---

### Task 1: Change the Neon Branch Mode with a Regression Guard

**Files:**

- Modify: `scripts/tests/backend-school-test-database.test.mjs`
- Modify: `.github/workflows/backend-school-neon-compatibility.yml`

**Interfaces:**

- Consumes: the existing `NEON_TEST_*` secret/variables and pinned create-branch action.
- Produces: an ordinary child branch named `schoolorbit-test-<run_id>-<run_attempt>` plus the action's direct `db_url` and exact created branch ID.

- [x] **Step 1: Change the workflow contract first**

Replace the existing schema-only assertion with a contract that rejects any explicit branch type:

```js
assert.doesNotMatch(workflow, /^\s*branch_type:/m);
```

The production change that makes this test pass is removal of the `branch_type: schema-only` action input. Reintroducing that input recreates the HTTP 412 path and must fail this guard.

- [x] **Step 2: Run the focused test and verify RED**

Run:

```bash
node --test --test-name-pattern='Neon gate is manual' \
  scripts/tests/backend-school-test-database.test.mjs
```

Expected: the Neon gate test fails because the workflow still contains `branch_type: schema-only`; unrelated cases are skipped.

- [x] **Step 3: Make the smallest workflow change**

Remove this action input and make no other lifecycle change:

```yaml
branch_type: schema-only
```

Omitting the input selects the pinned action's ordinary default child-branch mode.

- [x] **Step 4: Run the focused test and actionlint for GREEN**

Run:

```bash
node --test --test-name-pattern='Neon gate is manual' \
  scripts/tests/backend-school-test-database.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7 \
  .github/workflows/backend-school-neon-compatibility.yml
```

Expected: the focused test passes and actionlint exits zero.

---

### Task 2: Align Canonical Testing Documentation

**Files:**

- Modify: `docs/TESTING.md`

**Interfaces:**

- Consumes: the workflow behavior established in Task 1.
- Produces: a durable operator contract that requires a dedicated empty test project and describes ordinary disposable child branches accurately.

- [x] **Step 1: Update the manual Neon gate description**

State that the project and parent are dedicated to testing and contain no production data. Replace the schema-only wording with ordinary copy-on-write child-branch behavior. Preserve the direct endpoint, exact branch deletion, and two-hour expiration requirements.

- [x] **Step 2: Run canonical documentation guards**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: all documentation-policy tests pass.

---

### Task 3: Verify, Publish, and Exercise the Live Gate

**Files:**

- Verify: `.github/workflows/backend-school-neon-compatibility.yml`
- Verify: `scripts/tests/backend-school-test-database.test.mjs`
- Verify: `docs/TESTING.md`

**Interfaces:**

- Consumes: Tasks 1-2 and the already configured test-only Neon repository settings.
- Produces: a pushed `main` commit and one completed compatibility run whose create, Rust tests, and exact cleanup steps all succeed.

- [x] **Step 1: Run the complete local workflow verification**

Run:

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7 \
  .github/workflows/backend-school-neon-compatibility.yml
git diff --check
git status --short
```

Expected: 17 runner/workflow tests pass, 5 documentation-policy tests pass, actionlint exits zero, and only the approved workflow, test, documentation, spec, and plan scope is present.

- [x] **Step 2: Review and commit the implementation**

Review the complete diff, then commit the workflow, regression guard, documentation, and plan:

```bash
git add .github/workflows/backend-school-neon-compatibility.yml \
  scripts/tests/backend-school-test-database.test.mjs \
  docs/TESTING.md \
  docs/superpowers/plans/2026-08-11-backend-school-disposable-neon-child-branch.md
git commit -m "ci(backend-school): use disposable Neon child branches"
```

- [ ] **Step 3: Push `main` without force**

Fetch and confirm that `origin/main` is not ahead, then run:

```bash
git push origin main
```

- [ ] **Step 4: Dispatch and monitor the compatibility gate**

Dispatch `backend-school-neon-compatibility.yml` on the pushed `main` with
`confirm_disposable_branch=true`. Monitor the exact new run through completion.

Expected successful step sequence:

```text
Create disposable Neon branch
Verify this run created a fresh branch
Run direct-endpoint compatibility tests
Delete disposable Neon branch
```

- [ ] **Step 5: Verify cleanup and repository state**

Confirm from the run's job steps that deletion completed successfully after both Rust schema suites. Fetch `origin/main`, confirm its SHA equals local `HEAD`, confirm the worktree is clean, and report the run URL and exact verification counts.
