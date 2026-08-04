# Clamd Runtime Recreation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make backend-school deployment explicitly recreate `schoolorbit-clamd` and fail unless the running container has the exact 3 GiB memory limit before it becomes healthy.

**Architecture:** Keep `podman-compose.yml` as the production source of truth, but stop relying on implicit Compose reconciliation for the scanner. The backend-school workflow will pull the image, remove only the existing clamd container, recreate it from Compose, assert `HostConfig.Memory`, and then use the existing health wait before continuing.

**Tech Stack:** GitHub Actions YAML, rootless Podman, Podman Compose, Node.js built-in test runner, frontend static deployment invariants

## Global Constraints

- Preserve the pinned `docker.io/clamav/clamav-debian:1.5.3` image.
- Preserve the named `schoolorbit-clamav-signatures` volume; never remove or prune it.
- Require the runtime memory limit to equal `3221225472` bytes (3 GiB).
- Recreate only `schoolorbit-clamd` at this stage; do not restart backend-admin or implicitly recreate backend-school.
- Keep the existing scanner healthcheck and wait for `healthy` before backend-school deployment continues.
- Do not change Compose topology, CPU/PID limits, backend code, database schema, API contracts, permissions, realtime behavior, or sensitive-data handling.

---

### Task 1: Recreate clamd and verify its runtime memory limit

**Files:**

- Modify: `frontend-school/tests/static/deployment-installer.test.mjs:86`
- Modify: `.github/workflows/deploy-backend-school.yml:293-312`
- Test: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: the promoted `/opt/stack/podman-compose.yml`, the existing `compose_up_quiet` helper, `podman container exists`, and Podman inspect field `HostConfig.Memory`.
- Produces: an ordered deployment contract that recreates only `schoolorbit-clamd`, requires `3221225472` runtime bytes, and reaches the existing scanner health loop only after the assertion passes.

- [ ] **Step 1: Write the failing deployment invariant**

Add this test immediately after `local and production clamd allow 3 GiB for concurrent signature reloads`:

```js
test("backend-school deployment recreates clamd and verifies runtime memory before health", async () => {
  const workflow = await readRepo(
    ".github/workflows/deploy-backend-school.yml",
  );
  const scannerStart = workflow.indexOf(
    "# The scanner gets an isolated container network and no published port.",
  );
  const scannerEnd = workflow.indexOf("\n            jq_image=", scannerStart);

  assert.ok(scannerStart >= 0 && scannerEnd > scannerStart);
  const scannerDeployment = workflow.slice(scannerStart, scannerEnd);
  const orderedMarkers = [
    "podman pull docker.io/clamav/clamav-debian:1.5.3",
    "if podman container exists schoolorbit-clamd; then",
    "podman stop schoolorbit-clamd",
    "podman rm schoolorbit-clamd",
    "compose_up_quiet clamd",
    "expected_clamd_memory_bytes=$((3 * 1024 * 1024 * 1024))",
    `clamd_memory_bytes="$(podman inspect --format '{{.HostConfig.Memory}}' schoolorbit-clamd)"`,
    'if [ "$clamd_memory_bytes" != "$expected_clamd_memory_bytes" ]; then',
    "scanner_ready=false",
  ];
  let previousIndex = -1;
  for (const marker of orderedMarkers) {
    const markerIndex = scannerDeployment.indexOf(marker);
    assert.ok(
      markerIndex > previousIndex,
      `${marker} must appear in deployment order`,
    );
    previousIndex = markerIndex;
  }
  assert.doesNotMatch(scannerDeployment, /podman volume (?:rm|prune)/);
});
```

Production change that will make this test pass: add the explicit existing-container check, stop/remove sequence, exact runtime memory assertion, and preserve their order before the existing health loop.

- [ ] **Step 2: Run the focused test and verify RED**

Run from the repository root:

```bash
node --test \
  --test-name-pattern='backend-school deployment recreates clamd and verifies runtime memory before health' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because `if podman container exists schoolorbit-clamd; then` is absent from the scanner deployment block.

- [ ] **Step 3: Implement the minimal controlled recreation**

In `.github/workflows/deploy-backend-school.yml`, replace the current pull/start sequence immediately after the scanner comment with:

```bash
podman pull docker.io/clamav/clamav-debian:1.5.3
if podman container exists schoolorbit-clamd; then
  podman stop schoolorbit-clamd
  podman rm schoolorbit-clamd
fi
compose_up_quiet clamd

expected_clamd_memory_bytes=$((3 * 1024 * 1024 * 1024))
clamd_memory_bytes="$(podman inspect --format '{{.HostConfig.Memory}}' schoolorbit-clamd)"
if [ "$clamd_memory_bytes" != "$expected_clamd_memory_bytes" ]; then
  echo "Clamd memory limit mismatch: expected ${expected_clamd_memory_bytes} bytes, got ${clamd_memory_bytes} bytes"
  exit 1
fi
```

Leave the existing `scanner_ready=false` loop directly after this block. Do not use `--force-recreate`, `podman update`, `podman volume rm`, or a Compose down command.

- [ ] **Step 4: Format the test and verify GREEN**

Run from `frontend-school`:

```bash
npx prettier --write tests/static/deployment-installer.test.mjs
```

Then run from the repository root:

```bash
node --test \
  --test-name-pattern='backend-school deployment recreates clamd and verifies runtime memory before health' \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: PASS with zero failed tests.

- [ ] **Step 5: Run the frontend verification matrix**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: lint exits `0`; Svelte check reports zero errors and zero warnings; all static tests pass.

- [ ] **Step 6: Run the deployment verification matrix**

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
```

Expected: every command exits `0`; installer tests and deployment static tests report zero failures; the Podman Compose dry-run and Actionlint produce no errors.

- [ ] **Step 7: Run the readiness and cross-service smoke test**

Run from the repository root with credentials supplied only by `SMOKE_*` variables or `.env.smoke.local`:

```bash
scripts/smoke_test.sh
```

Expected: all configured readiness, CORS, realtime, authentication, and optional File Platform checks pass without printing credentials.

- [ ] **Step 8: Review the final tree and commit**

Run from the repository root:

```bash
git diff --check
git diff -- .github/workflows/deploy-backend-school.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git status --short
```

Expected: no whitespace errors; the diff contains only the explicit clamd recreation/runtime assertion and its static regression test; status lists only those two implementation files because the spec and plan are committed separately.

Commit:

```bash
git add .github/workflows/deploy-backend-school.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "fix: recreate clamd during deployment"
```

- [ ] **Step 9: Verify production after integration**

After the implementation is integrated into `main`, monitor `Deploy Backend School` until success. Confirm the runtime evidence from the VPS:

```bash
podman inspect --format \
  'memory={{.HostConfig.Memory}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' \
  schoolorbit-clamd
```

Expected: `memory=3221225472 health=healthy`. Cockpit may display the same 3 GiB ceiling as approximately `3.22 GB`.
