# CI Cache Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce ordinary warm-cache backend deployment and contract verification time without removing or weakening any correctness, security, migration, or readiness gate.

**Architecture:** Give the backend-admin and backend-school Docker builds separate GitHub Actions BuildKit cache scopes so their image layers cannot replace each other. Give the API and permission contract jobs one dependency-oriented backend-school Rust cache that pull requests may restore but only trusted `main` runs may save; retain npm caching and all existing workflow commands.

**Tech Stack:** GitHub Actions, Docker Buildx/BuildKit GHA cache backend, `Swatinem/rust-cache`, Node.js static tests, actionlint, Bash/Bats, Podman Compose

## Global Constraints

- Use the exact BuildKit scopes `backend-admin` and `backend-school` for the corresponding image workflows.
- Use the exact Rust shared key `backend-school-contracts` and workspace mapping `backend-school -> target` in both contract workflows.
- Pin `Swatinem/rust-cache` to immutable commit `e18b497796c12c097a38f9edb9d0641fb99eee32`, the commit referenced by the reviewed `v2` tag on 2026-08-04.
- Only `refs/heads/main` may save the shared Rust cache; pull request and other runs are restore-only.
- Keep the existing npm cache and every API generation, permission generation, backend test, frontend test, migration, readiness, proxy, R2, ClamAV, and deployment gate.
- Keep `group: deploy-schoolorbit-runtime` and `cancel-in-progress: false` unchanged.
- Cache metadata and summaries must not contain VPS, SSH, database, R2, Cloudflare, application, or runtime secrets.
- A cache miss performs the complete build or verification and is not a workflow failure.
- Do not add `sccache`, a self-hosted runner, a persistent build host, or a new production service in this implementation.
- Treat approximately three minutes for warm backend deploys and two to three minutes for warm contract workflows as diagnostic targets, not permission to skip work.
- Do not rerun a production backend deployment solely to benchmark cache performance.

---

### Task 1: Isolate Backend Image BuildKit Caches

**Files:**
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs:103-161`
- Modify: `.github/workflows/deploy-backend-admin.yml:61-70`
- Modify: `.github/workflows/deploy-backend-school.yml:63-72`

**Interfaces:**
- Consumes: the existing `docker/build-push-action@v7` build steps and Docker's `type=gha` cache backend.
- Produces: explicit `backend-admin` and `backend-school` import/export scopes plus a non-secret scope summary in each build job.

- [ ] **Step 1: Write the failing BuildKit cache policy test**

Add this test after `backend workflows deploy the canonical target and verify the selected origin` in `frontend-school/tests/static/deployment-installer.test.mjs`:

```javascript
test('backend image workflows use distinct BuildKit cache scopes', async () => {
	const workflowScopes = new Map([
		['.github/workflows/deploy-backend-admin.yml', 'backend-admin'],
		['.github/workflows/deploy-backend-school.yml', 'backend-school']
	]);

	assert.equal(new Set(workflowScopes.values()).size, workflowScopes.size);
	for (const [file, scope] of workflowScopes) {
		const workflow = await readRepo(file);
		assert.ok(workflow.includes(`cache-from: type=gha,scope=${scope}`));
		assert.ok(workflow.includes(`cache-to: type=gha,scope=${scope},mode=max`));
		assert.ok(workflow.includes('- name: Summarize Docker cache scope'));
		assert.ok(workflow.includes(`'- Scope: ${scope}'`));
		assert.ok(workflow.includes('Docker build record'));
		assert.ok(workflow.includes('>> "$GITHUB_STEP_SUMMARY"'));
	}
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run from the repository root:

```bash
node --test \
  --test-name-pattern='backend image workflows use distinct BuildKit cache scopes' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because both workflows still contain the unnamed `type=gha` scope and neither has a cache summary.

- [ ] **Step 3: Add the Admin-specific cache scope and summary**

In `.github/workflows/deploy-backend-admin.yml`, replace the two cache lines in `Build and push Docker image` with:

```yaml
          cache-from: type=gha,scope=backend-admin
          cache-to: type=gha,scope=backend-admin,mode=max
```

Immediately after that build step and before `deploy:`, add:

```yaml
      - name: Summarize Docker cache scope
        if: always()
        run: |
          printf '%s\n' \
            '### Docker build cache' \
            '- Scope: backend-admin' \
            '- Layer hits: inspect the Docker build record attached to this job.' \
            >> "$GITHUB_STEP_SUMMARY"
```

Do not change the image context, tags, labels, push behavior, permissions, deploy job, or shared runtime concurrency group.

- [ ] **Step 4: Add the School-specific cache scope and summary**

In `.github/workflows/deploy-backend-school.yml`, replace the two cache lines in `Build and push Docker image` with:

```yaml
          cache-from: type=gha,scope=backend-school
          cache-to: type=gha,scope=backend-school,mode=max
```

Immediately after that build step and before `deploy:`, add:

```yaml
      - name: Summarize Docker cache scope
        if: always()
        run: |
          printf '%s\n' \
            '### Docker build cache' \
            '- Scope: backend-school' \
            '- Layer hits: inspect the Docker build record attached to this job.' \
            >> "$GITHUB_STEP_SUMMARY"
```

Do not change R2 validation, ClamAV readiness, tenant migration, maintenance proxy, origin verification, or runtime deployment behavior.

- [ ] **Step 5: Run the focused test and verify GREEN**

Run:

```bash
node --test \
  --test-name-pattern='backend image workflows use distinct BuildKit cache scopes' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: PASS with two distinct scopes, matching import/export configuration, and both non-secret summaries.

- [ ] **Step 6: Validate workflow syntax and preserved deployment guards**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: both commands exit 0. The pre-existing deployment assertions must continue to find the canonical runtime, shared concurrency group, selected-origin checks, readiness loops, and proxy recovery logic.

- [ ] **Step 7: Review and commit the backend cache change**

Run:

```bash
git diff --check
git diff -- frontend-school/tests/static/deployment-installer.test.mjs \
  .github/workflows/deploy-backend-admin.yml \
  .github/workflows/deploy-backend-school.yml
git status --short
git add frontend-school/tests/static/deployment-installer.test.mjs \
  .github/workflows/deploy-backend-admin.yml \
  .github/workflows/deploy-backend-school.yml
git commit -m "ci: isolate backend image caches"
```

Expected: one focused commit containing only the static cache guard and the two backend workflow changes.

### Task 2: Share a Trusted Rust Dependency Cache Across Contract Workflows

**Files:**
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs:161-220`
- Modify: `.github/workflows/api-contract.yml:67-72`
- Modify: `.github/workflows/permission-contract.yml:53-58`
- Modify: `.rules:218-240`
- Modify: `docs/TESTING.md:82-112`

**Interfaces:**
- Consumes: the Rust toolchain installed by `dtolnay/rust-toolchain@stable`, the backend-school Cargo manifests and lockfile, and the `cache-hit` output from `Swatinem/rust-cache`.
- Produces: one dependency-oriented cache contract named `backend-school-contracts`, restore-only pull requests, trusted-main saves, and exact cache-hit summaries for both workflows.

- [ ] **Step 1: Write the failing Rust cache and gate-preservation test**

Add this test immediately after the BuildKit cache policy test:

```javascript
test('contract workflows share a main-writable Rust dependency cache without removing gates', async () => {
	const requiredCommands = new Map([
		[
			'.github/workflows/api-contract.yml',
			[
				'npm run test:api-contracts',
				'npm run check:api-contracts',
				'cargo fmt --all -- --check',
				'cargo test api_contract::tests --bin backend-school',
				'env -i PATH="$PATH" HOME="$HOME" cargo run --quiet --bin backend-school -- export-openapi',
				'cargo test structured_logging --test static_architecture',
				'cargo check --bin backend-school',
				'node --test tests/static/api-response-contract.test.mjs',
				'npm run check'
			]
		],
		[
			'.github/workflows/permission-contract.yml',
			[
				'node scripts/generate-permissions.mjs --check',
				'node --test scripts/tests/generate-permissions.test.mjs',
				'cargo fmt --all -- --check',
				'cargo check --bin backend-school',
				'cargo test --test static_architecture',
				'npm run test:static',
				'npm run check'
			]
		]
	]);

	for (const [file, commands] of requiredCommands) {
		const workflow = await readRepo(file);
		const setupRustIndex = workflow.indexOf('- name: Setup Rust');
		const rustCacheIndex = workflow.indexOf('- name: Restore Rust dependency cache');
		const firstCargoCommandIndex = workflow.indexOf('cargo ');

		assert.ok(setupRustIndex >= 0 && setupRustIndex < rustCacheIndex);
		assert.ok(rustCacheIndex >= 0 && rustCacheIndex < firstCargoCommandIndex);
		assert.match(
			workflow,
			/uses: Swatinem\/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32/
		);
		assert.match(workflow, /id: rust_cache/);
		assert.match(workflow, /shared-key: backend-school-contracts/);
		assert.match(workflow, /workspaces: backend-school -> target/);
		assert.match(workflow, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
		assert.match(workflow, /steps\.rust_cache\.outputs\.cache-hit/);
		assert.match(workflow, />> "\$GITHUB_STEP_SUMMARY"/);
		assert.match(workflow, /cache: npm/);
		assert.match(workflow, /cache-dependency-path: frontend-school\/package-lock\.json/);
		for (const command of commands) {
			assert.ok(workflow.includes(command), `${file} must retain ${command}`);
		}
	}

	const rules = await readRepo('.rules');
	assert.match(rules, /distinct GHA BuildKit cache scopes/);
	assert.match(rules, /Only trusted `main` runs may save the shared Rust dependency cache/);
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
node --test \
  --test-name-pattern='contract workflows share a main-writable Rust dependency cache without removing gates' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because neither contract workflow restores a Rust cache and `.rules` does not yet own the cache policy.

- [ ] **Step 3: Add the pinned cache and summary to API Contract**

In `.github/workflows/api-contract.yml`, immediately after `Setup Rust` and before `Install frontend dependencies`, add:

```yaml
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
```

Keep every existing API contract step in its current order. In particular, retain the sanitized `env -i` OpenAPI export, generated artifact comparison, exporter logging test, backend `cargo check`, frontend static test, and frontend check.

- [ ] **Step 4: Add the identical cache contract to Permission Contract**

In `.github/workflows/permission-contract.yml`, immediately after `Setup Rust` and before `Check generated permission artifacts`, add the same block:

```yaml
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
```

Keep generated permission checks, generator tests, backend formatting/check/architecture tests, npm installation, frontend static contracts, and frontend checking unchanged.

- [ ] **Step 5: Document the durable cache policy**

Add this bullet to `.rules` under `## 10. Deployment Constraints`, immediately after the canonical production Compose ownership bullet:

```markdown
- Backend image workflows use distinct GHA BuildKit cache scopes. API and permission contract workflows share only the dependency-oriented backend-school Rust cache; pull requests are restore-only. Only trusted `main` runs may save the shared Rust dependency cache. A cache miss must run every existing build, contract, test, migration, readiness, and deployment gate.
```

Add this paragraph after the deployment static guard description in `docs/TESTING.md`:

```markdown
The guard also owns CI cache policy: backend-admin and backend-school use distinct BuildKit scopes, while API and permission contract jobs share a dependency-oriented backend-school Rust cache. Pull requests are restore-only, and only trusted `main` runs may save it. A cache miss must execute the complete workflow rather than bypassing a gate.
```

- [ ] **Step 6: Run the focused test and verify GREEN**

Run:

```bash
node --test \
  --test-name-pattern='contract workflows share a main-writable Rust dependency cache without removing gates' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: PASS for both workflows, including cache placement, immutable pin, main-only save policy, npm cache retention, summaries, and every listed gate.

- [ ] **Step 7: Validate documentation and all workflow syntax**

Run:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: all commands exit 0; actionlint accepts the pinned action, expressions, summary steps, and cache inputs.

- [ ] **Step 8: Review and commit the contract cache policy**

Run:

```bash
git diff --check
git diff -- frontend-school/tests/static/deployment-installer.test.mjs \
  .github/workflows/api-contract.yml \
  .github/workflows/permission-contract.yml \
  .rules docs/TESTING.md
git status --short
git add frontend-school/tests/static/deployment-installer.test.mjs \
  .github/workflows/api-contract.yml \
  .github/workflows/permission-contract.yml \
  .rules docs/TESTING.md
git commit -m "ci: cache Rust contract dependencies"
```

Expected: one focused commit with the two contract workflow changes, static guard, and durable policy documentation; no generated contract or runtime file changes.

### Task 3: Run the Full Workflow Matrix and Measure Warm Runs

**Files:**
- Verify: `.github/workflows/deploy-backend-admin.yml`
- Verify: `.github/workflows/deploy-backend-school.yml`
- Verify: `.github/workflows/api-contract.yml`
- Verify: `.github/workflows/permission-contract.yml`
- Verify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Verify: `.rules`
- Verify: `docs/TESTING.md`

**Interfaces:**
- Consumes: the two focused implementation commits and the repository verification matrix in `.rules`.
- Produces: local evidence that the complete deployment/workflow guard passes, then first-run and warm-run GitHub Actions evidence without an artificial production redeploy.

- [ ] **Step 1: Run the installer and production-topology matrix**

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

Expected: every command exits 0. If Docker, Bats, or Podman Compose is unavailable, report that command as unrun with the exact missing dependency; do not replace it with a weaker check.

- [ ] **Step 2: Perform the final repository review**

Run:

```bash
git diff --check
git log --oneline --decorate -5
git diff origin/main...HEAD -- .github/workflows \
  frontend-school/tests/static/deployment-installer.test.mjs \
  .rules docs/TESTING.md docs/superpowers
git status --short --branch
```

Expected: no whitespace errors, no uncommitted files, no secrets or generated artifacts, and only the approved spec, plan, four workflow changes, static assertions, and cache policy documentation ahead of `origin/main`.

- [ ] **Step 3: Push the reviewed commits to main**

Run:

```bash
git push origin main
```

Expected: the push succeeds without force. Because each workflow file changed, GitHub schedules Backend Admin, Backend School, API Contract, and Permission Contract at the pushed commit; Installer and documentation policy may also run because their watched files changed.

- [ ] **Step 4: Observe every naturally triggered first run**

Run:

```bash
head_sha="$(git rev-parse HEAD)"
for workflow in \
  deploy-backend-admin.yml \
  deploy-backend-school.yml \
  api-contract.yml \
  permission-contract.yml
do
  run_id="$(gh run list --workflow "$workflow" --commit "$head_sha" \
    --limit 1 --json databaseId --jq '.[0].databaseId')"
  test -n "$run_id"
  gh run watch "$run_id" --exit-status
done
```

Expected: all four runs finish successfully. Docker logs show the new explicit scopes; contract summaries show the first Rust cache result. Treat these new-scope/cache-population runs as cold establishment runs, not warm timing failures.

- [ ] **Step 5: Rerun only the contract workflows for warm measurements**

Run:

```bash
head_sha="$(git rev-parse HEAD)"
for workflow in api-contract.yml permission-contract.yml
do
  run_id="$(gh run list --workflow "$workflow" --commit "$head_sha" \
    --limit 1 --json databaseId --jq '.[0].databaseId')"
  test -n "$run_id"
  gh run rerun "$run_id"
  gh run watch "$run_id" --exit-status
  gh run view "$run_id" --json conclusion,jobs,url
done
```

Expected: both reruns succeed, their summaries report an exact Rust cache hit, and their latest-attempt job timestamps provide the warm duration. Do not invoke `gh run rerun` for either backend deploy workflow.

- [ ] **Step 6: Record the measured outcome and decide whether a second stage is justified**

Report separately:

```text
Backend Admin: first scoped run build time, deploy time, cache-layer evidence
Backend School: first scoped run build time, deploy time, cache-layer evidence
API Contract: first run time, warm rerun time, exact cache hit
Permission Contract: first run time, warm rerun time, exact cache hit
```

Expected: ordinary warm backend deployments trend toward three minutes or less on their next normal deploy, while contract reruns trend toward two to three minutes. If a warm contract run remains materially slower, inspect its step timings before proposing `sccache` or job parallelism; do not remove a validation gate. If backend deployment time rather than build time dominates, treat it as a separate runtime/deployment optimization.
