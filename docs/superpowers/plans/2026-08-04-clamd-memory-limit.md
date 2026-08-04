# Clamd Memory Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise the clamd memory ceiling from 1536 MB to 3 GB in local and production Compose definitions while enforcing the value through the deployment contract test.

**Architecture:** Keep the existing ClamAV image, healthcheck, scanner network, CPU/PID limits, and concurrent signature reload behavior. Resolve both Compose files through Docker Compose in the static test and assert their normalized clamd memory limit is exactly 3 GiB.

**Tech Stack:** Docker Compose YAML, Podman Compose production topology, Node.js built-in test runner, GitHub Actions deployment invariants

## Global Constraints

- Set clamd `mem_limit` to `3g` in both `docker-compose.yml` and `podman-compose.yml`.
- Keep concurrent database reload enabled.
- Do not change CPU, PID, healthcheck, network, volume, scanner timeout, or ClamAV image settings.
- Do not change database, API, permission, frontend, realtime, or sensitive-data behavior.

---

### Task 1: Enforce and apply the 3 GB clamd memory limit

**Files:**

- Modify: `frontend-school/tests/static/deployment-installer.test.mjs:13-64`
- Modify: `docker-compose.yml:149`
- Modify: `podman-compose.yml:140`
- Test: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: Docker Compose JSON output for the `clamd` service in the local and production Compose definitions.
- Produces: An invariant requiring `services.clamd.mem_limit === "3221225472"` for both definitions.

- [ ] **Step 1: Write the failing deployment invariant**

Add this test after `the resolved production topology has one owner and private backend ports`:

```js
test("local and production clamd allow 3 GiB for concurrent signature reloads", async () => {
  for (const [file, extraArguments] of [
    ["docker-compose.yml", []],
    [
      "podman-compose.yml",
      ["--env-file", "scripts/tests/installer/fixtures/runtime.env"],
    ],
  ]) {
    const { stdout } = await execFileAsync(
      "docker",
      ["compose", ...extraArguments, "-f", file, "config", "--format", "json"],
      { cwd: repoRoot },
    );
    const topology = JSON.parse(stdout);

    assert.equal(
      topology.services.clamd.mem_limit,
      String(3 * 1024 * 1024 * 1024),
      `${file} must preserve enough memory for concurrent ClamAV database reloads`,
    );
  }
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run from the repository root:

```bash
node --test --test-name-pattern='local and production clamd allow 3 GiB' frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because the resolved limit is `1610612736`, not `3221225472`.

- [ ] **Step 3: Apply the minimal Compose change**

In both `docker-compose.yml` and `podman-compose.yml`, change only:

```yaml
mem_limit: 1536m
```

to:

```yaml
mem_limit: 3g
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
node --test --test-name-pattern='local and production clamd allow 3 GiB' frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: PASS with zero failed tests.

- [ ] **Step 5: Run the complete deployment verification matrix**

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
git diff --check
git status --short
```

Expected: Every command exits `0`; the deployment static test reports zero failures; `git diff --check` prints nothing; `git status --short` lists only the intended two Compose files, the deployment test, and this plan if it is not already committed.

- [ ] **Step 6: Commit the implementation**

```bash
git add docker-compose.yml podman-compose.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "fix: raise clamd memory limit"
```
