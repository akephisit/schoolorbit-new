# Backend School Disposable Neon Child Branch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the manual Backend School Neon compatibility gate use ordinary disposable child branches inside the dedicated test-only project without requesting plan-restricted compute settings, so branch creation, migration tests, and cleanup complete reliably.

**Architecture:** Keep the existing manually confirmed workflow, pinned Neon action, direct endpoint, unique run-attempt name, two-hour expiration, and exact-ID finalizer. Use ordinary branch mode against an empty test-only parent and the Free-plan-compatible 300-second compute suspension; each Rust test creates an isolated schema and runs the active migrations itself.

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

The production change that makes this test pass is removal of the `branch_type: schema-only` action input. The later ordinary-branch run proved that this input was not the root cause of HTTP 412, but the guard preserves the selected test-only branch architecture.

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

- [x] **Step 3: Push `main` without force**

Fetch and confirm that `origin/main` is not ahead, then run:

```bash
git push origin main
```

- [x] **Step 4: Dispatch and monitor the compatibility gate**

Dispatch `backend-school-neon-compatibility.yml` on the pushed `main` with
`confirm_disposable_branch=true`. Monitor the exact new run through completion.

Expected successful step sequence:

```text
Create disposable Neon branch
Verify this run created a fresh branch
Run direct-endpoint compatibility tests
Delete disposable Neon branch
```

Run `31494176489` reached the create step but returned HTTP 412 before producing a branch. Because
that pushed workflow already used ordinary branch mode, the result disproved schema-only creation
as the cause and triggered Task 4.

- [ ] **Step 5: Verify cleanup and repository state**

Confirm from the run's job steps that deletion completed successfully after both Rust schema suites. Fetch `origin/main`, confirm its SHA equals local `HEAD`, confirm the worktree is clean, and report the run URL and exact verification counts.

---

### Task 4: Replace the Plan-Restricted Suspension Override

**Files:**

- Modify: `.github/workflows/backend-school-neon-compatibility.yml`
- Modify: `scripts/tests/backend-school-test-database.test.mjs`
- Modify: `docs/TESTING.md`
- Modify: `docs/superpowers/specs/2026-08-11-backend-school-disposable-neon-child-branch-design.md`

**Interfaces:**

- Consumes: the ordinary child-branch workflow and the Neon Free project's fixed five-minute
  scale-to-zero interval.
- Produces: a create request with a supported 300-second suspension value, followed by the
  unchanged direct-endpoint tests and exact-ID cleanup.

- [x] **Step 1: Diagnose the ordinary-branch live failure**

Compare the failed run with the pinned action request and Neon plan behavior. The workflow's
explicit `suspend_timeout: 60` requests a one-minute interval that the configured Free project
cannot apply; branch mode is no longer a shared condition. The pinned action maps an omitted value
to `0` (disabled auto-suspend), so omission is not a safe representation of the project default.

- [x] **Step 2: Add a regression guard and demonstrate RED**

Add this assertion to the existing Neon workflow contract, then run the focused test while the
input is still present:

```js
assert.match(workflow, /^\s+suspend_timeout:\s*300\s*$/m);
```

Expected and observed: the focused test fails until the workflow contains exactly the supported
300-second value; both the original 60-second value and an omitted input violate the contract.

- [x] **Step 3: Replace the override and make the focused contract GREEN**

Replace `suspend_timeout: 60` with `suspend_timeout: 300`. Keep the branch expiration, direct
endpoint, and exact cleanup unchanged. Run the focused Node test and actionlint.

- [x] **Step 4: Run fresh verification, commit, and push `main`**

Run the complete workflow contract, documentation policy, actionlint, `git diff --check`, and final
scope review. Commit only the approved backend-school test infrastructure and documentation, fetch
without force, and push `main`.

- [ ] **Step 5: Re-dispatch the live gate and verify cleanup**

Dispatch the pushed corrective commit with `confirm_disposable_branch=true`. Require successful
create, fresh-branch verification, both Rust schema suites, and exact branch deletion. Then confirm
remote/local SHA equality and a clean worktree.

Run `31495216927` used ordinary branch mode and `suspend_timeout: 300` but still returned the same
opaque HTTP 412. This disproved the suspension value as a complete explanation. Upstream issue
`neondatabase/create-branch-action#233` confirms that the pinned action drops the Neon API response
body, so the next change must expose a sanitized error instead of changing another input blindly.

---

### Task 5: Surface the Neon API Rejection Safely

**Files:**

- Create: `scripts/neon-branch-create-diagnostic.mjs`
- Create: `scripts/tests/neon-branch-create-diagnostic.test.mjs`
- Modify: `.github/workflows/backend-school-neon-compatibility.yml`
- Modify: `scripts/tests/backend-school-test-database.test.mjs`

**Interfaces:**

- Consumes: the same test project, parent, expiration, and API credential as the failed create step.
- Produces: only a bounded, sanitized Neon status/code/message on rejection. If the diagnostic probe
  unexpectedly creates a branch, it deletes that exact branch before failing the already-failed job.

- [x] **Step 1: Write diagnostic behavior tests and demonstrate RED**

Cover a rejected request, an unexpectedly successful create with exact cleanup, malformed
configuration, and redaction of API keys and connection URLs. Require the probe payload to use the
unique diagnostic branch name, configured parent, two-hour expiration, and 300-second endpoint.

- [x] **Step 2: Implement the minimal dependency-free diagnostic**

Use Node's built-in `fetch` and JSON support. Never print a request body, response body, API key,
password, or database URL. Extract only scalar `code` and `message` fields, collapse control
characters, redact URL-like text, and cap their length.

- [x] **Step 3: Wire the diagnostic only after create failure**

Run it with `if: failure() && steps.create_branch.outcome == 'failure'`, a run-attempt-scoped probe
name, and the existing expiration output. Keep all successful-path behavior unchanged.

- [ ] **Step 4: Verify, commit, push, and run one diagnostic gate**

Run both Node suites, documentation policy, actionlint, and diff checks. Commit and push `main`,
dispatch one gate, and record the sanitized Neon message. Do not change another create parameter
until that evidence identifies the failed precondition.
