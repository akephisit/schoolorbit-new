# Deployment Runtime Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bound VPS and GHCR image growth while reducing backend image/build/deploy overhead without weakening any production gate.

**Architecture:** Add tested, repository-owned cleanup and ClamAV convergence scripts, then wire them into the existing serialized backend workflows at their current acceptance boundaries. Keep BuildKit dependency caching, add deterministic build tools and optional secret-mounted sccache, export Cargo timings outside the runtime image, and perform GHCR lifecycle maintenance in a separate fail-closed workflow.

**Tech Stack:** Bash, Bats, Node.js test runner, GitHub Actions, Docker Buildx/BuildKit, Rust/Cargo, sccache, rootless Podman

**Spec:** `docs/superpowers/specs/2026-09-05-deployment-runtime-optimization-design.md`

## Global Constraints

- Retain exactly the newest three local 40-character lowercase hexadecimal release tags per backend repository.
- Retain the newest 30 GHCR SHA releases per backend package and every version carrying `latest`.
- Never invoke Podman system, volume, container, or image prune.
- Preserve `schoolorbit-clamav-signatures` and every existing migration, readiness, proxy, R2, origin, and authenticated-smoke gate.
- Pin Rust to `1.98.0-slim-bookworm`, cargo-chef to `0.1.78`, and sccache to `0.17.0` with SHA-256 `67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006`.
- Build cache credentials must be optional BuildKit secret mounts and must never enter arguments, layers, artifacts, cache keys, or logs.
- Local verification must not mutate the production VPS, push an image, or delete a GHCR package version.

---

### Task 1: Production image retention selector

**Files:**

- Create: `scripts/prune_runtime_images.sh`
- Create: `scripts/tests/installer/runtime_image_retention.bats`

**Interfaces:**

- Consumes: `scripts/prune_runtime_images.sh REPOSITORY KEEP_COUNT`, `podman images --sort created`, `podman ps -aq`, `podman inspect`, and `podman image inspect`.
- Produces: bounded `runtime_image_cleanup` status lines and removal of old exact SHA references.

- [ ] **Step 1: Write failing behavior tests**

Create Bats fixtures whose fake `podman` returns five SHA-tagged releases in newest-first order,
`latest` and `rollback` references, and an active container image ID. Assert that the script:

```bash
run "$BATS_TEST_DIRNAME/../../prune_runtime_images.sh" \
    ghcr.io/akephisit/schoolorbit-backend-school 3
[ "$status" -eq 0 ]
[[ "$output" == *'runtime_image_cleanup repository=ghcr.io/akephisit/schoolorbit-backend-school before=5 retained=3 removed=2'* ]]
grep -Fxq 'image rm ghcr.io/akephisit/schoolorbit-backend-school:1111111111111111111111111111111111111111' "$FAKE_COMMAND_LOG"
! grep -Eq 'volume|system prune|image prune|rm --force' "$FAKE_COMMAND_LOG"
```

Add separate tests for active/`latest`/`rollback` protection, idempotency with three releases,
repository allowlisting, invalid counts, malformed listing output, and enumeration failure before the
first `image rm` call.

- [ ] **Step 2: Verify RED**

Run:

```bash
bats scripts/tests/installer/runtime_image_retention.bats
```

Expected: FAIL because `scripts/prune_runtime_images.sh` does not exist.

- [ ] **Step 3: Implement the minimal fail-closed script**

Implement a Bash script with `set -euo pipefail`, exact repository allowlisting for the two backend
repositories, integer retention validation, full pre-validation of the newest-first image listing,
normalized image IDs, and an in-memory protected-ID set. Recognize release tags with:

```bash
[[ $tag =~ ^[0-9a-f]{40}$ ]]
```

Collect all candidates before mutation, skip the first `KEEP_COUNT` SHA releases, skip every ID used
by a container or resolved through `latest`/`rollback`, then call:

```bash
podman image rm "${repository}:${tag}"
```

Print only repository, counts, exact safe SHA tags, and `podman system df` totals. Do not invoke any
prune command; exact final-reference removal is sufficient for Podman to reclaim unshared layers.

- [ ] **Step 4: Verify GREEN and shell quality**

Run:

```bash
bats scripts/tests/installer/runtime_image_retention.bats
shellcheck scripts/prune_runtime_images.sh
shfmt -d -i 4 -ci scripts/prune_runtime_images.sh
```

Expected: every test and command exits 0.

- [ ] **Step 5: Commit Task 1**

```bash
git add scripts/prune_runtime_images.sh scripts/tests/installer/runtime_image_retention.bats
git commit -m "feat(ops): retain bounded runtime images"
```

---

### Task 2: Runtime diagnostics, workflow cleanup placement, and phase timing

**Files:**

- Create: `scripts/lib/schoolorbit-installer/remote/deployment_timing.sh`
- Create: `scripts/tests/installer/deployment_timing.bats`
- Modify: `.github/workflows/runtime-diagnostics.yml`
- Modify: `.github/workflows/deploy-backend-admin.yml`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: Task 1's `prune_runtime_images.sh`; `schoolorbit_timer_now`; `schoolorbit_timer_report PHASE START_EPOCH`.
- Produces: safe disk reports, `deployment_timing phase=... seconds=...` markers, and cleanup only after existing acceptance gates.

- [ ] **Step 1: Write failing timer and workflow tests**

Add Bats tests that source the timing helper, set `SCHOOLORBIT_TIMER_NOW=110`, and verify:

```bash
run schoolorbit_timer_report image_pull 100
[ "$status" -eq 0 ]
[ "$output" = 'deployment_timing phase=image_pull seconds=10' ]
```

Reject phase names outside `^[a-z0-9_]+$`, non-integer timestamps, and end times earlier than the
start. Extend the deployment static test by behavior boundary: cleanup script upload and invocation
must occur after admin origin verification and after school migration/audit verification; no cleanup
command may occur in failure/rollback functions. Runtime diagnostics must contain `podman system df`
and SHA counts while remaining free of environment and application-log commands.

- [ ] **Step 2: Verify RED**

Run:

```bash
bats scripts/tests/installer/deployment_timing.bats
node --test --test-name-pattern='runtime image cleanup|runtime diagnostics|deployment timing' \
    frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because the helper, reports, timing markers, and workflow cleanup calls are absent.

- [ ] **Step 3: Implement the timing helper and diagnostics**

Implement `schoolorbit_timer_now` using `${SCHOOLORBIT_TIMER_NOW:-$(date +%s)}` and
`schoolorbit_timer_report` with strict phase/timestamp validation and integer subtraction. Extend
Runtime Diagnostics with the graph-root filesystem capacity, `podman system df`, verbose accounting,
and exact repository/SHA counts. Do not add environment inspection or logs.

- [ ] **Step 4: Wire deployment assets and accepted-release cleanup**

Add both new scripts to each backend workflow's SCP source. Source the timer helper remotely. Wrap
the image pull, replacement/readiness, migration/status, scanner, smoke, and origin phases applicable
to each workflow. Invoke:

```bash
/opt/stack/deployment/scripts/prune_runtime_images.sh \
    ghcr.io/akephisit/schoolorbit-backend-school 3
```

after the school rollback tag advances, and the corresponding admin repository after admin origin
verification. Replace admin's unbounded dangling-only cleanup with the shared script.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
bats scripts/tests/installer/deployment_timing.bats
node --test frontend-school/tests/static/deployment-installer.test.mjs
shellcheck scripts/prune_runtime_images.sh \
    scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
shfmt -d -i 4 -ci scripts/prune_runtime_images.sh \
    scripts/lib/schoolorbit-installer/remote/deployment_timing.sh
```

Expected: zero failures and no shell warnings.

- [ ] **Step 6: Commit Task 2**

```bash
git add .github/workflows/runtime-diagnostics.yml \
    .github/workflows/deploy-backend-admin.yml \
    .github/workflows/deploy-backend-school.yml \
    scripts/lib/schoolorbit-installer/remote/deployment_timing.sh \
    scripts/tests/installer/deployment_timing.bats \
    frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "feat(ops): report and bound deployment storage"
```

---

### Task 3: Smaller deterministic backend images and build evidence

**Files:**

- Modify: `backend-admin/Dockerfile`
- Modify: `backend-school/Dockerfile`
- Modify: `.github/workflows/deploy-backend-admin.yml`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: optional BuildKit secrets `sccache_gha_url` and `sccache_gha_token` mapped from GitHub's cache runtime values.
- Produces: unchanged runtime commands and UID 1000, Cargo timing HTML targets, and bounded sccache statistics.

- [ ] **Step 1: Write failing Docker/workflow contract tests**

Extend the deployment static test to require both Dockerfiles to use
`rust:1.98.0-slim-bookworm`, `cargo install cargo-chef --version 0.1.78 --locked`, the exact sccache
release/checksum, Cargo `--timings`, a non-runtime `build-timings` stage, runtime-user creation before
copies, and `COPY --chown=1000:1000`. Reject `RUN chown -R`.

Require each workflow to export the GitHub cache runtime through `actions/github-script`, pass both
values with `secret-envs`, keep the existing BuildKit scope, export only the timing stage in a second
non-pushing build, and upload the timing HTML with short retention. Reject cache tokens in
`build-args`, `secrets:` literals, summaries, or artifacts.

- [ ] **Step 2: Verify RED**

Run:

```bash
node --test --test-name-pattern='deterministic backend images|Cargo timing|sccache' \
    frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL on the floating Rust/cargo-chef versions and absent timing/cache contract.

- [ ] **Step 3: Implement deterministic Docker builders**

In both Dockerfiles, add Dockerfile syntax 1.10, pin Rust and cargo-chef, fetch the official sccache
0.17.0 musl archive by immutable URL with the approved checksum, and install only its executable.
Use optional secret environment mounts around the final application build. When both values are
non-empty, set distinct `SCCACHE_GHA_CACHE_TO`/`SCCACHE_GHA_CACHE_FROM` keys and
`RUSTC_WRAPPER=/usr/local/bin/sccache`; otherwise invoke Cargo directly. Both paths run release
builds with `--timings`, and the cached path prints `sccache --show-stats`.

Add:

```dockerfile
FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html
```

Create UID 1000 before runtime copies, use numeric `--chown` on the binary and migrations, and retain
the existing runtime package list, `USER`, port, and command.

- [ ] **Step 4: Wire cache runtime and timing artifacts**

Use `actions/github-script` to export only `ACTIONS_RESULTS_URL` and `ACTIONS_RUNTIME_TOKEN`, pass
them as BuildKit `secret-envs`, and add a second `docker/build-push-action` call with
`target: build-timings`, `push: false`, and local output under `${{ runner.temp }}`. Upload only
`cargo-timing.html` with a seven-day retention.

- [ ] **Step 5: Verify GREEN and build runtime images**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker build --target runtime -t schoolorbit/backend-admin:verification backend-admin
docker build --target runtime -t schoolorbit/backend-school:verification backend-school
docker image inspect schoolorbit/backend-admin:verification \
    --format '{{.Config.User}} {{json .Config.Cmd}}'
docker image inspect schoolorbit/backend-school:verification \
    --format '{{.Config.User}} {{json .Config.Cmd}}'
```

Expected: tests/builds pass; users are `appuser`; commands remain the matching backend executable.

- [ ] **Step 6: Commit Task 3**

```bash
git add backend-admin/Dockerfile backend-school/Dockerfile \
    .github/workflows/deploy-backend-admin.yml \
    .github/workflows/deploy-backend-school.yml \
    frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "perf(ci): cache and measure Rust image builds"
```

---

### Task 4: Conditional ClamAV convergence

**Files:**

- Create: `scripts/clamd_runtime_matches.sh`
- Create: `scripts/tests/installer/clamd_runtime_matches.bats`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: `scripts/clamd_runtime_matches.sh IMAGE CONTAINER`; canonical ClamAV image and container names.
- Produces: exit 0 with `clamd_action=reused`, or exit 1 with exactly one bounded `clamd_drift reason=<code>` line.

- [ ] **Step 1: Write failing ClamAV behavior tests**

Use a fake Podman boundary to return complete inspect fixtures. Assert a healthy exact runtime exits
0. Add one table-driven Bats case per drift: missing container, image, memory, CPU, PID, restart,
security option, published port, signature volume, network, running state, and health. Assert every
drift exits 1 with a bounded reason and never mutates Podman state.

- [ ] **Step 2: Verify RED**

Run:

```bash
bats scripts/tests/installer/clamd_runtime_matches.bats
```

Expected: FAIL because the matcher does not exist.

- [ ] **Step 3: Implement the read-only matcher**

Validate exact image/container arguments, resolve and normalize desired/running image IDs, and query
only individual safe inspect templates. Compare 3 GiB memory, 1,000,000,000 NanoCPUs, 256 PIDs,
`unless-stopped`, semantic `no-new-privileges`, empty port bindings, the named signature mount, both
required networks, running status, and healthy status. Convert any command/read failure into a
bounded drift code; never print raw inspect output.

- [ ] **Step 4: Replace unconditional recreation with convergence**

Upload the matcher in backend-school. After pulling the pinned scanner image, run the matcher. Reuse
on exit 0. On exit 1, stop/remove only `schoolorbit-clamd`, recreate it from Compose, assert memory,
and retain the existing health wait. Update the static deployment test from unconditional recreation
to reuse-or-recreate ordering and preserve the no-volume-prune assertion.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
bats scripts/tests/installer/clamd_runtime_matches.bats
node --test frontend-school/tests/static/deployment-installer.test.mjs
shellcheck scripts/clamd_runtime_matches.sh
shfmt -d -i 4 -ci scripts/clamd_runtime_matches.sh
```

Expected: zero failures.

- [ ] **Step 6: Commit Task 4**

```bash
git add scripts/clamd_runtime_matches.sh \
    scripts/tests/installer/clamd_runtime_matches.bats \
    .github/workflows/deploy-backend-school.yml \
    frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "perf(deploy): reuse matching ClamAV runtime"
```

---

### Task 5: Fail-closed GHCR retention

**Files:**

- Create: `scripts/prune_ghcr_versions.mjs`
- Create: `scripts/tests/prune-ghcr-versions.test.mjs`
- Create: `.github/workflows/ghcr-retention.yml`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**

- Consumes: `node scripts/prune_ghcr_versions.mjs --owner OWNER --package PACKAGE --keep 30 [--execute]`, `GITHUB_TOKEN`, and optional `GITHUB_API_URL`.
- Produces: dry-run candidate records or DELETE requests for revalidated old SHA releases only.

- [ ] **Step 1: Write failing selector and HTTP-boundary tests**

Test exported `selectDeletionCandidates(versions, 30)` with 35 newest-first SHA releases, a `latest`
version, non-SHA tags, and untagged attestation versions. Hand-derived expectation: exactly the five
oldest SHA-only versions are candidates; protected and unknown versions are absent.

Start a local Node HTTP server that implements paginated inventory, per-version reread, and delete.
Assert dry-run makes no DELETE requests, execute deletes only revalidated candidates oldest first,
and changed/protected metadata, malformed pagination, unauthorized responses, or unsupported package
names fail closed.

- [ ] **Step 2: Verify RED**

Run:

```bash
node --test scripts/tests/prune-ghcr-versions.test.mjs
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the retention CLI**

Use global `fetch`, validate owner/package/count, allow only the two SchoolOrbit package names,
paginate 100 versions per page, classify exact SHA container tags, and preserve `latest`, the newest
30 release versions, and all unknown/untagged versions. Before each DELETE, GET that exact version
again and confirm it remains an unprotected candidate. Cap deletions at 100 per invocation and print
only version ID, timestamp, and safe tags.

- [ ] **Step 4: Add the maintenance workflow and static contract**

Create a weekly workflow plus manual dispatch with `dry_run` defaulting to true. Grant only
`contents: read` and `packages: write`, checkout the repository, and call the CLI once per exact
backend package. Scheduled runs pass `--execute` only when `vars.GHCR_RETENTION_ENABLED` is exactly
`true`; manual runs pass it only when `dry_run` is false. The workflow receives only `GITHUB_TOKEN`
and no deployment secrets.

- [ ] **Step 5: Verify GREEN**

Run:

```bash
node --test scripts/tests/prune-ghcr-versions.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: zero failures and local tests make no GitHub requests.

- [ ] **Step 6: Commit Task 5**

```bash
git add scripts/prune_ghcr_versions.mjs scripts/tests/prune-ghcr-versions.test.mjs \
    .github/workflows/ghcr-retention.yml \
    frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "feat(ci): retain bounded GHCR releases"
```

---

### Task 6: Durable operations guidance and full verification

**Files:**

- Modify: `docs/OPERATIONS.md`
- Modify: `docs/TESTING.md`

**Interfaces:**

- Consumes: all prior tasks.
- Produces: operator guidance for diagnostics, retention, dry-run, rollout observation, and rollback.

- [ ] **Step 1: Update canonical documentation**

Document local three-release retention, protected images/volume, Runtime Diagnostics output, weekly
GHCR 30-release policy, manual dry-run, Cargo timing artifacts, sccache fallback/evaluation, ClamAV
reuse/recreation, and the exact post-merge observation sequence. Add focused and full verification
commands to `docs/TESTING.md`; do not duplicate the design history.

- [ ] **Step 2: Run focused suites**

```bash
bats scripts/tests/installer/runtime_image_retention.bats \
    scripts/tests/installer/deployment_timing.bats \
    scripts/tests/installer/clamd_runtime_matches.bats
node --test scripts/tests/prune-ghcr-versions.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: zero failures.

- [ ] **Step 3: Run the complete deployment verification matrix**

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
    scripts/lib/schoolorbit-installer/*.sh \
    scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
    scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
    scripts/lib/schoolorbit-installer/*.sh \
    scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
    podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: every command exits 0.

- [ ] **Step 4: Run applicable frontend and repository checks**

From `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Then from the repository root:

```bash
git diff --check
git status --short
```

Expected: zero frontend errors/warnings/test failures; only intentional branch changes appear.

- [ ] **Step 5: Review artifacts and commit documentation**

Review the full diff for migration changes, secrets, broad prune commands, lost deployment gates,
workflow permissions, and generated artifacts. Then run:

```bash
git add docs/OPERATIONS.md docs/TESTING.md
git commit -m "docs: document deployment retention and timing"
```

- [ ] **Step 6: Record post-merge checks without triggering production**

Confirm the handoff states that the next ordinary deployments—not a benchmark-only deployment—must
verify disk before/after, three retained local SHA tags, active/rollback availability, ClamAV action,
Cargo timing artifact, sccache statistics, and all readiness/smoke gates. Run the GHCR workflow
manually in its default dry-run mode before enabling scheduled deletion through merge.
