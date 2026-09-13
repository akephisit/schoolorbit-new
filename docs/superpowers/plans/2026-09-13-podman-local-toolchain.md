# Podman Local Toolchain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Standardize Podman as SchoolOrbit's local, test, verification, and production container runtime while retaining Docker Buildx only for GitHub Actions image builds.

**Architecture:** Rename the local topology to `compose.local.yml`, invoke `podman-compose` explicitly for local Compose operations, and make the disposable backend-school database runner use rootless local Podman directly. Update executable tests first, then the runtime scripts/configuration, durable `.rules`, and canonical documentation; keep production `podman-compose.yml` and backend image-build actions unchanged.

**Tech Stack:** Bash, Podman 5.x, podman-compose 1.x, Compose YAML, Node.js test runner, `yaml` npm package, Rust static architecture tests, GitHub Actions

**Spec:** `docs/superpowers/specs/2026-09-13-podman-local-toolchain-design.md`

## Global Constraints

- Podman is the only repository-supported runtime for local development, database tests, deployment verification, and production.
- `compose.local.yml` owns local PostgreSQL and source-build topology; `podman-compose.yml` remains the sole production Compose owner.
- Repository commands invoke `podman` and `podman-compose` directly; do not add a Docker fallback, engine abstraction, Podman Docker socket, or `docker=podman` alias.
- GitHub backend image builds retain `docker/login-action`, `docker/metadata-action`, `docker/setup-buildx-action`, and `docker/build-push-action`.
- Database-test cleanup removes only the uniquely named test container and its anonymous volume; never prune global Podman state or remove the cached PostgreSQL image.
- Never print inherited database URLs, container environments, credentials, or production secrets.
- Do not edit migrations or generated permission/API contracts.
- Work inline on `chore/podman-local-toolchain`; do not dispatch subagents.

---

### Task 1: Convert the Disposable PostgreSQL Runner to Rootless Podman

**Files:**
- Modify: `scripts/tests/backend-school-test-database.test.mjs`
- Modify: `scripts/test_backend_school.sh`

**Interfaces:**
- Consumes: the existing CLI contract `./scripts/test_backend_school.sh [cargo-test-arguments...]`
- Produces: the same CLI contract backed by local rootless `podman`; recognizes `CONTAINER_HOST` and `CONTAINER_CONNECTION` as forbidden remote-selection inputs
- Produces: exact cleanup through `podman rm --force --volumes <generated-container-name>`

- [ ] **Step 1: Rewrite the fake-engine fixture and expectations to specify Podman behavior**

Rename fixture fields such as `dockerLog` to `podmanLog`, create a fake executable named `podman`, and log every direct Podman operation. The fake must implement these observable calls:

```bash
case "$command_name" in
    info)
        case " $* " in
            *' --format '*) printf '%s\n' "${FAKE_PODMAN_ROOTLESS:-true}" ;;
        esac
        exit "${FAKE_PODMAN_INFO_STATUS:-0}"
        ;;
    run)    # record the generated name and return a fake container ID
        ;;
    exec)   # emulate pg_isready and extension bootstrap
        ;;
    port)   printf '%s\n' "${FAKE_PODMAN_BINDING:-127.0.0.1:55432}" ;;
    container) # support `exists` and `inspect`
        ;;
    rm)     # remove only the fixture's generated container state
        ;;
    logs)   printf '%s\n' 'fake postgres startup log' ;;
esac
```

Update existing assertions from Docker to Podman and add focused cases proving:

```js
test('missing Podman fails before Cargo runs', async (t) => {
    const f = await fixture(t);
    await rm(path.join(f.bin, 'podman'));
    const result = runRunner(f);
    assert.equal(result.status, 127);
    assert.match(result.stderr, /Podman is required/);
    await assert.rejects(read(f.cargoLog));
});

test('configured remote Podman connections are rejected', async (t) => {
    const f = await fixture(t);
    for (const env of [
        { CONTAINER_HOST: 'ssh://server.example/run/user/1000/podman/podman.sock' },
        { CONTAINER_CONNECTION: 'production' }
    ]) {
        const result = runRunner(f, [], env);
        assert.equal(result.status, 64);
        assert.match(result.stderr, /local Podman engine/);
    }
});

test('a non-rootless Podman engine is rejected before creating a container', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_ROOTLESS: 'false' });
    assert.equal(result.status, 64);
    assert.match(result.stderr, /rootless Podman/);
    await assert.rejects(read(f.containerState));
});
```

Keep the existing success, argument forwarding, inherited URL redaction, startup/readiness/bootstrap/port failures, signal cleanup, and cleanup-failure coverage. Assert the cleanup log contains `command=rm`, `arg=--force`, `arg=--volumes`, and only the generated container name.

- [ ] **Step 2: Run the focused test and verify the Docker implementation fails the new contract**

Run:

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
```

Expected: FAIL because `scripts/test_backend_school.sh` still requires and invokes `docker`.

- [ ] **Step 3: Implement direct rootless Podman execution in the runner**

Replace Docker-specific discovery and operations with:

```bash
if ! command -v podman >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: Podman is required for backend-school database tests' >&2
    exit 127
fi

if [[ -n ${CONTAINER_HOST-} || -n ${CONTAINER_CONNECTION-} ]]; then
    printf '%s\n' 'ERROR: backend-school tests require a local Podman engine' >&2
    exit 64
fi

if ! podman info >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: the local Podman engine is not reachable' >&2
    exit 69
fi

if ! podman_rootless="$(podman info --format '{{.Host.Security.Rootless}}')" ||
    [[ $podman_rootless != true ]]; then
    printf '%s\n' 'ERROR: backend-school tests require rootless Podman' >&2
    exit 64
fi
```

Use `podman run`, `podman exec`, `podman container exists`, `podman container inspect`,
`podman logs`, `podman port`, and `podman rm --force --volumes` everywhere else. Keep the pinned
PostgreSQL digest, random loopback port, anonymous volume, readiness loop, extension SQL, local-only
URL construction, Cargo invocation, signal traps, and original-status precedence unchanged.

- [ ] **Step 4: Run focused tests and shell checks**

Run:

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
shellcheck scripts/test_backend_school.sh
shfmt -d -i 4 -ci scripts/test_backend_school.sh
```

Expected: all commands PASS.

- [ ] **Step 5: Commit the Podman database runner**

```bash
git add scripts/test_backend_school.sh scripts/tests/backend-school-test-database.test.mjs
git commit -m "test: run disposable school database with Podman"
```

---

### Task 2: Rename the Local Topology and Make Topology Tests Podman-Native

**Files:**
- Delete: `docker-compose.yml`
- Create: `compose.local.yml`
- Modify: `frontend-school/package.json`
- Modify: `frontend-school/package-lock.json`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: installed `podman-compose` CLI and the existing local/production Compose definitions
- Produces: local command `podman-compose -f compose.local.yml ...`
- Produces: `loadComposeConfig(file, extraArguments): Promise<object>` in the deployment static test, parsing `podman-compose config` YAML

- [ ] **Step 1: Add failing assertions for the new filename and Podman Compose command**

In `deployment-installer.test.mjs`, import the parser and introduce:

```js
import { parse as parseYaml } from 'yaml';

async function loadComposeConfig(file, extraArguments = []) {
    const { stdout } = await execFileAsync(
        'podman-compose',
        [...extraArguments, '-f', file, 'config'],
        { cwd: repoRoot }
    );
    return parseYaml(stdout);
}
```

Change the file inventory and topology cases to require `compose.local.yml` and
`podman-compose.yml`, reject the former `docker-compose.yml`, and call `loadComposeConfig` for both
topologies. Assert the parsed port strings exactly as authored:

```js
assert.deepEqual(topology.services['backend-admin'].ports, ['127.0.0.1:8080:8080']);
assert.deepEqual(topology.services['backend-school'].ports, ['127.0.0.1:8081:8081']);
```

In `static_architecture.rs`, replace local filename references and require source builds in
`compose.local.yml` while retaining production GHCR-image and loopback-binding assertions.

- [ ] **Step 2: Run focused tests and verify they fail before the rename/dependency change**

Run:

```bash
cd frontend-school
node --test tests/static/deployment-installer.test.mjs
cd ../backend-school
cargo test --test static_architecture compose -- --nocapture
```

Expected: FAIL because `yaml` is not a direct dependency and `compose.local.yml` does not exist.

- [ ] **Step 3: Add the direct YAML development dependency**

From `frontend-school`, run:

```bash
npm install --save-dev --save-exact yaml@2.8.1
```

Expected manifest entry:

```json
"yaml": "2.8.1"
```

- [ ] **Step 4: Rename the local Compose file without changing its topology**

Move the complete tracked content from `docker-compose.yml` to `compose.local.yml`. Update its
leading comment to identify it as the rootless Podman local topology. Do not change service names,
development defaults, build contexts, healthchecks, ports, networks, or volumes in this task.

- [ ] **Step 5: Run focused topology tests**

Run:

```bash
cd frontend-school
node --test tests/static/deployment-installer.test.mjs
cd ../backend-school
cargo test --test static_architecture compose -- --nocapture
```

Expected: both commands PASS with Podman Compose installed.

- [ ] **Step 6: Commit the local topology cutover**

```bash
git add compose.local.yml docker-compose.yml \
  frontend-school/package.json frontend-school/package-lock.json \
  frontend-school/tests/static/deployment-installer.test.mjs \
  backend-school/tests/static_architecture.rs
git commit -m "chore: make local topology Podman native"
```

---

### Task 3: Convert Installer and Verification Contracts to Podman

**Files:**
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Modify: `.github/workflows/installer.yml`
- Modify: `.rules`

**Interfaces:**
- Consumes: pinned actionlint image `rhysd/actionlint:1.7.7`
- Produces: `podman run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7`
- Preserves: Docker Buildx action references in backend image-build workflows

- [ ] **Step 1: Add failing verification-policy assertions**

Update the deployment static guard to require `podman run` for actionlint, require Podman-native
commands in `.rules`, and explicitly retain assertions for all four Docker Buildx action families:

```js
assert.match(installerWorkflow, /podman run --rm .*rhysd\/actionlint:1\.7\.7/);
assert.doesNotMatch(installerWorkflow, /run: docker run/);
for (const action of [
    'docker/login-action@v4',
    'docker/metadata-action@v6',
    'docker/setup-buildx-action@v4',
    'docker/build-push-action@v7'
]) {
    assert.match(backendWorkflow, new RegExp(action.replace('/', '\\/')));
}
```

- [ ] **Step 2: Run the deployment static guard and verify it fails**

Run:

```bash
cd frontend-school
node --test tests/static/deployment-installer.test.mjs
```

Expected: FAIL because installer verification and `.rules` still contain Docker runtime commands.

- [ ] **Step 3: Update the installer workflow and durable rules**

In `.github/workflows/installer.yml`, replace only the actionlint runtime command with `podman run`.
Do not alter backend Docker Buildx jobs.

In `.rules`, update runtime ownership and the installer verification runtime commands:

```bash
podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
podman run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

State explicitly that Buildx remains the CI builder and Podman is the runtime/test verifier.

- [ ] **Step 4: Run the focused guard**

```bash
cd frontend-school
node --test tests/static/deployment-installer.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit executable verification and rule changes**

```bash
git add .github/workflows/installer.yml .rules \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "ci: verify deployment tooling with Podman"
```

---

### Task 4: Update Canonical Setup, Testing, and Operations Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Verify: `frontend-school/tests/static/documentation-policy.test.mjs`
- Verify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**
- Consumes: the finalized `compose.local.yml`, Podman database runner, and `.rules` commands
- Produces: one consistent local setup and verification workflow with no Docker runtime instructions

- [ ] **Step 1: Run the existing documentation and topology guards as a baseline**

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs \
  tests/static/deployment-installer.test.mjs
```

Expected: the documentation-policy test passes; topology failures, if any, must be attributable to
the not-yet-installed Podman prerequisite rather than prose inspection. Human-facing prose does not
receive source-text change-detector tests.

- [ ] **Step 2: Update canonical documentation**

In `README.md`, install/run local services with:

```bash
sudo apt update
sudo apt install -y podman podman-compose
podman info
podman-compose -f compose.local.yml up --build
```

In `docs/OPERATIONS.md`, define `compose.local.yml` as local-only and
`podman-compose.yml` as production-only, both executed by rootless Podman.

In `docs/TESTING.md`, replace Docker Desktop and Docker runtime commands with Podman equivalents,
document the rootless/local-engine requirement, and retain explicit cleanup semantics. State that
Docker Buildx remains limited to GitHub image-build jobs.

- [ ] **Step 3: Run documentation and executable topology guards**

```bash
cd frontend-school
node --test tests/static/documentation-policy.test.mjs \
  tests/static/deployment-installer.test.mjs
```

Expected: PASS.

- [ ] **Step 4: Commit canonical documentation**

```bash
git add README.md docs/TESTING.md docs/OPERATIONS.md \
  frontend-school/tests/static/documentation-policy.test.mjs \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "docs: standardize local development on Podman"
```

---

### Task 5: Install Prerequisites and Run the Complete Verification Matrix

**Files:**
- Verify only; no planned source changes

**Interfaces:**
- Consumes: Debian packages `podman`, `podman-compose`, `bats`, `shellcheck`, `shfmt`, and `jq`
- Produces: fresh evidence for the full Podman cutover and a clean final diff

- [ ] **Step 1: Install required host tools with human-entered sudo authentication**

Run in the user's terminal; never collect or transmit the sudo password:

```bash
sudo apt update
sudo apt install -y podman podman-compose bats shellcheck shfmt jq
```

Then verify rootless operation:

```bash
podman --version
podman-compose version
podman info --format '{{.Host.Security.Rootless}}'
```

Expected: version commands succeed and rootless output is `true`.

- [ ] **Step 2: Run focused runner and topology tests**

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

Expected: PASS.

- [ ] **Step 3: Run shell and installer verification from the repository root**

```bash
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
  scripts/test_backend_school.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/prune_runtime_images.sh scripts/clamd_runtime_matches.sh \
  scripts/test_backend_school.sh \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test scripts/tests/prune-ghcr-versions.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml --dry-run up -d >/dev/null
podman run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

Expected: all commands PASS without contacting production or pruning global state.

- [ ] **Step 4: Validate local Compose and runtime images**

```bash
podman-compose -f compose.local.yml config >/dev/null
podman build --target runtime -t schoolorbit/backend-admin:verification backend-admin
podman build --target runtime -t schoolorbit/backend-school:verification backend-school
podman image inspect schoolorbit/backend-admin:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
podman image inspect schoolorbit/backend-school:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
```

Expected: Compose parses, both images build, and each inspection reports user `appuser` with the
expected backend command.

- [ ] **Step 5: Run the actual disposable PostgreSQL static architecture suite**

```bash
./scripts/test_backend_school.sh \
  modules::auth::session_schema_tests -- --nocapture
```

Expected: Podman creates one uniquely named PostgreSQL container and anonymous volume, tests PASS,
and the exact container and attached anonymous volume no longer exist afterward.

- [ ] **Step 6: Run frontend matrices**

```bash
cd frontend-admin
npm run lint
PUBLIC_API_URL=http://localhost:8080 npm run check
npm run test:unit
npm run build

cd ../frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
npm run build
```

Expected: all commands PASS.

- [ ] **Step 7: Run backend matrices**

```bash
cd backend-admin
cargo fmt --all -- --check
cargo test
cargo check --bin backend-admin

cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Expected: all commands PASS. If the existing backend-admin auxiliary `create_admin` SQLx macro still
requires `DATABASE_URL`, report the uncredentialed all-target check separately; the main
`backend-admin` binary check must pass.

- [ ] **Step 8: Review the final repository state**

```bash
git diff --check origin/main...HEAD
git diff --stat origin/main...HEAD
git diff origin/main...HEAD
git status --short
git rev-list --left-right --count origin/main...HEAD
```

Expected: no whitespace errors, only scoped Podman/tooling changes, a clean working tree, and the
feature branch ahead of `origin/main` with no missing upstream commits.
