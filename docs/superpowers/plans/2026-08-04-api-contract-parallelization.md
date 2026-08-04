# API Contract Parallelization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce warm API Contract wall-clock time by running artifact, backend, and frontend validation concurrently while preserving every existing gate.

**Architecture:** Replace the single API Contract job with three independent jobs that check the same tracked commit and exchange no artifacts. The artifact and backend jobs restore the existing shared Rust cache, but only the artifact job may save it; the frontend and artifact jobs retain npm caching.

**Tech Stack:** GitHub Actions, Rust/Cargo, Node.js/npm, `Swatinem/rust-cache`, Node.js static tests, actionlint, Bash/Bats, Podman Compose

## Global Constraints

- API Contract job IDs are exactly `artifacts`, `backend`, and `frontend`.
- None of the three jobs declares `needs`; all validation groups remain independently schedulable.
- Keep every existing command for generator tests, artifact comparison, sanitized offline export, formatting, backend API tests, logging-boundary tests, `cargo check`, frontend API tests, and frontend checking.
- Keep `env -i PATH="$PATH" HOME="$HOME"` on the sanitized export without adding runtime or database variables.
- Both Rust jobs use pinned action commit `e18b497796c12c097a38f9edb9d0641fb99eee32`, shared key `backend-school-contracts`, and workspace `backend-school -> target`.
- `artifacts` saves the Rust cache only on `refs/heads/main`; `backend` always uses `save-if: "false"`.
- Both Rust jobs publish exact cache-hit summaries without secrets.
- `artifacts` and `frontend` retain Node 22, npm caching, and `frontend-school/package-lock.json` as the cache dependency path.
- Workflow permissions remain `contents: read`.
- Permission Contract, generated contracts, Rust/frontend application code, migrations, permissions, runtime deployment, and production services remain unchanged.
- Do not add `sccache`, a self-hosted runner, or a persistent build host in this stage.
- Updating the cross-stack static guard triggers the existing all-tenant frontend deployment; observe it, but do not trigger either backend deployment.

---

### Task 1: Guard and Implement Three Independent API Contract Jobs

**Files:**
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs:179-240`
- Modify: `.github/workflows/api-contract.yml:48-131`

**Interfaces:**
- Consumes: the existing tracked OpenAPI artifacts, frontend-school npm package, backend-school Cargo workspace, and `backend-school-contracts` cache created in Stage 1.
- Produces: three GitHub jobs named `artifacts`, `backend`, and `frontend`; one trusted-main API cache writer; two exact-hit Rust summaries; unchanged validation commands.

- [ ] **Step 1: Replace the broad contract test with job-ownership guards**

Replace `contract workflows share a main-writable Rust dependency cache without removing gates` in `frontend-school/tests/static/deployment-installer.test.mjs` with these two tests:

```javascript
test('API contract runs artifact backend and frontend gates in independent jobs', async () => {
	const workflow = await readRepo('.github/workflows/api-contract.yml');
	const jobsStart = workflow.indexOf('\njobs:\n');
	assert.ok(jobsStart >= 0);
	const jobs = workflow.slice(jobsStart + '\njobs:\n'.length);
	const jobNames = [...jobs.matchAll(/^  ([a-z][a-z0-9_-]*):\s*$/gm)].map(
		(match) => match[1]
	);
	assert.deepEqual(jobNames, ['artifacts', 'backend', 'frontend']);
	assert.doesNotMatch(jobs, /^    needs:/gm);

	const jobBlock = (name, nextName) => {
		const start = jobs.indexOf(`  ${name}:\n`);
		assert.ok(start >= 0, `missing ${name} job`);
		const end = nextName ? jobs.indexOf(`\n  ${nextName}:\n`, start) : jobs.length;
		assert.ok(end > start, `invalid ${name} job boundary`);
		return jobs.slice(start, end);
	};
	const artifacts = jobBlock('artifacts', 'backend');
	const backend = jobBlock('backend', 'frontend');
	const frontend = jobBlock('frontend');

	for (const command of [
		'npm run test:api-contracts',
		'npm run check:api-contracts',
		'env -i PATH="$PATH" HOME="$HOME" cargo run --quiet --bin backend-school -- export-openapi'
	]) {
		assert.ok(artifacts.includes(command), `artifacts must retain ${command}`);
	}
	for (const command of [
		'cargo fmt --all -- --check',
		'cargo test api_contract::tests --bin backend-school',
		'cargo test structured_logging --test static_architecture',
		'cargo check --bin backend-school'
	]) {
		assert.ok(backend.includes(command), `backend must retain ${command}`);
	}
	for (const command of [
		'node --test tests/static/api-response-contract.test.mjs',
		'npm run check'
	]) {
		assert.ok(frontend.includes(command), `frontend must retain ${command}`);
	}

	for (const nodeJob of [artifacts, frontend]) {
		assert.match(nodeJob, /uses: actions\/setup-node@v6/);
		assert.match(nodeJob, /node-version: "22"/);
		assert.match(nodeJob, /cache: npm/);
		assert.match(nodeJob, /cache-dependency-path: frontend-school\/package-lock\.json/);
		assert.match(nodeJob, /working-directory: frontend-school\n\s+run: npm ci/);
	}
	assert.doesNotMatch(backend, /uses: actions\/setup-node@v6/);

	for (const rustJob of [artifacts, backend]) {
		assert.match(rustJob, /uses: dtolnay\/rust-toolchain@stable/);
		assert.match(
			rustJob,
			/uses: Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/
		);
		assert.match(rustJob, /id: rust_cache/);
		assert.match(rustJob, /shared-key: backend-school-contracts/);
		assert.match(rustJob, /workspaces: backend-school -> target/);
		assert.match(rustJob, /steps\.rust_cache\.outputs\.cache-hit/);
		assert.match(rustJob, />> "\$GITHUB_STEP_SUMMARY"/);
	}
	assert.match(artifacts, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
	assert.match(backend, /save-if: "false"/);
	assert.doesNotMatch(frontend, /Swatinem\/rust-cache/);
	assert.equal((jobs.match(/save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/g) ?? []).length, 1);
	assert.equal((jobs.match(/save-if: "false"/g) ?? []).length, 1);
});

test('Permission Contract keeps its cached validation gates unchanged', async () => {
	const workflow = await readRepo('.github/workflows/permission-contract.yml');
	assert.match(workflow, /^  verify:\s*$/m);
	assert.match(
		workflow,
		/uses: Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/
	);
	assert.match(workflow, /shared-key: backend-school-contracts/);
	assert.match(workflow, /workspaces: backend-school -> target/);
	assert.match(workflow, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
	assert.match(workflow, /steps\.rust_cache\.outputs\.cache-hit/);
	assert.match(workflow, /cache: npm/);
	for (const command of [
		'node scripts/generate-permissions.mjs --check',
		'node --test scripts/tests/generate-permissions.test.mjs',
		'cargo fmt --all -- --check',
		'cargo check --bin backend-school',
		'cargo test --test static_architecture',
		'npm run test:static',
		'npm run check'
	]) {
		assert.ok(workflow.includes(command), `Permission Contract must retain ${command}`);
	}
});
```

The mutation this catches is a return to one sequential `verify` job, an added `needs`, a moved or removed gate, a second API cache writer, a missing summary, or an accidental Permission Contract change.

- [ ] **Step 2: Run the focused test and verify RED**

Run from the repository root:

```bash
node --test \
  --test-name-pattern='API contract runs artifact backend and frontend gates in independent jobs|Permission Contract keeps its cached validation gates unchanged' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: the Permission test passes and the API test fails because the workflow still exposes only `verify`.

- [ ] **Step 3: Replace the single API Contract job with the parallel layout**

Keep the existing triggers and `permissions` block unchanged. Replace the complete `jobs:` block in `.github/workflows/api-contract.yml` with:

```yaml
jobs:
  artifacts:
    name: Contract artifacts and offline export
    runs-on: ubuntu-24.04
    env:
      FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
      PUBLIC_BACKEND_URL: http://localhost:3000
      PUBLIC_VAPID_KEY: test

    steps:
      - name: Checkout repository
        uses: actions/checkout@v6

      - name: Setup Node.js
        uses: actions/setup-node@v6
        with:
          node-version: "22"
          cache: npm
          cache-dependency-path: frontend-school/package-lock.json

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt

      - name: Restore Rust dependency cache
        id: rust_cache
        uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32
        with:
          shared-key: backend-school-contracts
          workspaces: backend-school -> target
          save-if: ${{ github.ref == 'refs/heads/main' }}

      - name: Summarize Rust cache
        if: always()
        env:
          RUST_CACHE_HIT: ${{ steps.rust_cache.outputs.cache-hit }}
        run: |
          printf '%s\n' \
            '### Rust build cache' \
            '- Shared key: backend-school-contracts' \
            "- Exact cache hit: ${RUST_CACHE_HIT:-false}" \
            >> "$GITHUB_STEP_SUMMARY"

      - name: Install frontend dependencies
        working-directory: frontend-school
        run: npm ci

      - name: Test API contract generator
        working-directory: frontend-school
        run: npm run test:api-contracts

      - name: Check generated API contract artifacts
        working-directory: frontend-school
        run: npm run check:api-contracts

      - name: Test offline OpenAPI export
        working-directory: backend-school
        run: |
          env -i PATH="$PATH" HOME="$HOME" cargo run --quiet --bin backend-school -- export-openapi > /tmp/school-api.json
          node -e "JSON.parse(require('node:fs').readFileSync('/tmp/school-api.json', 'utf8'))"

  backend:
    name: Backend contract verification
    runs-on: ubuntu-24.04
    env:
      FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

    steps:
      - name: Checkout repository
        uses: actions/checkout@v6

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt

      - name: Restore Rust dependency cache
        id: rust_cache
        uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32
        with:
          shared-key: backend-school-contracts
          workspaces: backend-school -> target
          save-if: "false"

      - name: Summarize Rust cache
        if: always()
        env:
          RUST_CACHE_HIT: ${{ steps.rust_cache.outputs.cache-hit }}
        run: |
          printf '%s\n' \
            '### Rust build cache' \
            '- Shared key: backend-school-contracts' \
            "- Exact cache hit: ${RUST_CACHE_HIT:-false}" \
            >> "$GITHUB_STEP_SUMMARY"

      - name: Check backend formatting
        working-directory: backend-school
        run: cargo fmt --all -- --check

      - name: Test backend API contract
        working-directory: backend-school
        run: cargo test api_contract::tests --bin backend-school

      - name: Test exporter logging boundary
        working-directory: backend-school
        run: cargo test structured_logging --test static_architecture

      - name: Check backend
        working-directory: backend-school
        run: cargo check --bin backend-school

  frontend:
    name: Frontend contract verification
    runs-on: ubuntu-24.04
    env:
      FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
      PUBLIC_BACKEND_URL: http://localhost:3000
      PUBLIC_VAPID_KEY: test

    steps:
      - name: Checkout repository
        uses: actions/checkout@v6

      - name: Setup Node.js
        uses: actions/setup-node@v6
        with:
          node-version: "22"
          cache: npm
          cache-dependency-path: frontend-school/package-lock.json

      - name: Install frontend dependencies
        working-directory: frontend-school
        run: npm ci

      - name: Test frontend API contract
        working-directory: frontend-school
        run: node --test tests/static/api-response-contract.test.mjs

      - name: Check frontend
        working-directory: frontend-school
        run: npm run check
```

Do not add `needs`, workflow artifacts, generated-file writes, matrix jobs, secret inputs, or changes to the trigger paths.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
node --test \
  --test-name-pattern='API contract runs artifact backend and frontend gates in independent jobs|Permission Contract keeps its cached validation gates unchanged' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: both tests pass; API exposes exactly three independent jobs with the approved command/cache ownership, and Permission remains unchanged.

- [ ] **Step 5: Run the complete static workflow guards**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: the complete deployment suite and actionlint exit 0 without warnings.

- [ ] **Step 6: Review and commit the parallel workflow**

Run:

```bash
git diff --check
git diff -- .github/workflows/api-contract.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git status --short
git add .github/workflows/api-contract.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "ci: parallelize API contract checks"
```

Expected: one focused commit with only the API workflow and cross-stack guard.

### Task 2: Document, Verify, Push, and Measure the Parallel Contract

**Files:**
- Modify: `.rules:224-230`
- Modify: `docs/TESTING.md:101-110`
- Verify: `.github/workflows/api-contract.yml`
- Verify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**
- Consumes: the three-job workflow and static ownership guard from Task 1.
- Produces: durable development policy, full local/CI evidence, and first/warm parallel timing evidence.

- [ ] **Step 1: Extend the static guard to require the durable rule**

At the end of `API contract runs artifact backend and frontend gates in independent jobs`, add:

```javascript
	const rules = await readRepo('.rules');
	assert.match(
		rules,
		/API Contract runs artifact, backend, and frontend validation in independent jobs without `needs`/
	);
```

Run:

```bash
node --test \
  --test-name-pattern='API contract runs artifact backend and frontend gates in independent jobs' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because `.rules` does not yet own the approved independent-job sentence.

- [ ] **Step 2: Add the authoritative parallel-job policy**

Add this bullet in `.rules` immediately after the existing CI cache policy bullet:

```markdown
- API Contract runs artifact, backend, and frontend validation in independent jobs without `needs`. Keep every contract gate in its owned job; only the artifact job may save the API workflow's shared Rust cache, while the backend job is restore-only.
```

Extend the CI cache paragraph in `docs/TESTING.md` with:

```markdown
API Contract keeps artifact generation/offline export, backend validation, and frontend validation in independent jobs without `needs`; the static guard owns this division and its single-writer Rust cache policy.
```

Rerun the focused API test from Step 1. Expected: PASS because the authoritative rule now matches the static guard.

- [ ] **Step 3: Run documentation and complete workflow guards**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
git diff --check
```

Expected: documentation policy passes 4/4, the deployment suite passes all tests, actionlint emits no findings, and no whitespace errors exist.

- [ ] **Step 4: Commit the durable policy**

Run:

```bash
git add .rules docs/TESTING.md frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "docs: define parallel API contract ownership"
```

Expected: one documentation/guard commit; no workflow, application, generated contract, or runtime files are included.

- [ ] **Step 5: Run the full installer and production-topology matrix**

Run from the repository root:

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: every available command exits 0. Report absent local ShellCheck, shfmt, Bats, or Podman Compose binaries as unrun and require the Installer workflow to supply that evidence after push; do not substitute a weaker check.

- [ ] **Step 6: Perform the final repository review**

Run:

```bash
git diff --check
git status --short --branch
git log --oneline --decorate -6
git diff origin/main...HEAD -- .github/workflows/api-contract.yml \
  frontend-school/tests/static/deployment-installer.test.mjs \
  .rules docs/TESTING.md docs/superpowers
```

Expected: a clean working tree with only the approved Stage 2 spec/plan, API workflow, static guard, and durable policy ahead of `origin/main`; no secrets or generated outputs.

- [ ] **Step 7: Push main and observe all automatic workflows**

Run:

```bash
git push origin main
```

Observe the pushed commit with `gh run list`. Require success from API Contract, Permission Contract, Installer Verification, Documentation, and Deploy Frontend School to All Tenants. Confirm no Backend Admin or Backend School deployment run is created for this commit.

- [ ] **Step 8: Rerun only API Contract and measure the warm attempt**

Resolve the API run at the pushed commit, rerun it, and watch the same run ID:

```bash
head_sha="$(git rev-parse HEAD)"
api_run_id="$(gh run list --workflow api-contract.yml --commit "$head_sha" \
  --limit 1 --json databaseId --jq '.[0].databaseId')"
test -n "$api_run_id"
gh run rerun "$api_run_id"
gh run watch "$api_run_id" --exit-status
gh run view "$api_run_id" --attempt 2 --json conclusion,jobs,url
```

Expected: all three jobs succeed; artifacts and backend report exact Rust cache hits; frontend reports an npm cache hit; overall warm wall-clock trends toward three to four minutes.

- [ ] **Step 9: Record the measured critical path**

Report:

```text
API Contract workflow: first parallel attempt duration, warm attempt duration
artifacts: duration, Rust exact-hit result, longest command
backend: duration, Rust exact-hit result, longest command
frontend: duration, npm cache result, longest command
Permission Contract: unchanged workflow result
Automatic frontend deployment: result
Backend deployments: no run created
```

If warm wall-clock remains materially above four minutes, use logs from the longest job to distinguish Cargo workspace compilation from queueing, npm, and the approximately 869 MB Rust cache restore before proposing Stage 3.
