# Backend School Ephemeral Test Databases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make routine `backend-school` database tests run faster against disposable PostgreSQL on the developer's computer, while retaining an explicit disposable Neon compatibility gate.

**Architecture:** A root shell runner owns one loopback-only Docker PostgreSQL container for each Cargo invocation and removes it through an exit trap. A separate manually dispatched GitHub workflow creates a schema-only Neon test branch, runs direct-endpoint migration/schema checks, and deletes the exact branch in an unconditional finalizer.

**Tech Stack:** Bash, Docker Desktop, PostgreSQL 18.4, Cargo/SQLx, Node.js built-in test runner, GitHub Actions, Neon create-branch action and REST API.

## Global Constraints

- Read `.rules` before every implementation batch and run its applicable verification matrix.
- Do not modify `backend-admin`, `frontend-admin`, `frontend-school` application code, runtime topology, deployment workflows, or any applied migration.
- Routine tests execute on the machine where the command is invoked. The runner must not use SSH, a TCP/SSH Docker endpoint, a remote Docker context, Podman, the VPS, or Neon.
- Pin the local image to `docker.io/library/postgres:18.4-alpine@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15`, matching the observed PostgreSQL 18.4 test endpoint.
- A routine invocation owns exactly one generated container and no named volume. It removes that container on success, failure, `INT`, `TERM`, and `HUP`.
- The routine runner always replaces an inherited/dotenv `TEST_DATABASE_URL` with a loopback URL and never prints either URL.
- The Neon gate is `workflow_dispatch` only, uses dedicated `NEON_TEST_*` configuration, and consumes the create action's direct `db_url`, never `db_url_pooled`.
- Preserve Cargo's failure status when cleanup succeeds. If Cargo succeeds but cleanup fails, return non-zero. If both fail, report cleanup failure but retain Cargo's status.
- Follow TDD: add a focused failing test, observe RED, implement the minimum behavior, observe GREEN, then commit.

## File Structure

- Create `scripts/test_backend_school.sh`: local container and Cargo lifecycle owner.
- Create `scripts/tests/backend-school-test-database.test.mjs`: fake-Docker/fake-Cargo behavior tests and the static Neon workflow guard.
- Create `.github/workflows/backend-school-neon-compatibility.yml`: manual disposable Neon lifecycle.
- Modify `.rules`, `docs/TESTING.md`, `backend-school/README.md`, and `TODO.md`: durable policy and commands.
- Do not modify `backend-school/src/test_helpers.rs`; existing schema isolation continues to consume `TEST_DATABASE_URL`.

---

### Task 1: Local Disposable PostgreSQL Runner

**Files:**

- Create: `scripts/tests/backend-school-test-database.test.mjs`
- Create: `scripts/test_backend_school.sh`

**Interfaces:**

- Consumes: local Docker CLI, Cargo, and the current `TEST_DATABASE_URL` test-helper contract.
- Produces: `scripts/test_backend_school.sh [cargo-test-arguments...]`; default command `cargo test --bin backend-school`; loopback test URL scoped only to the Cargo child.

- [x] **Step 1: Write the fake command fixture and first failing behavior test**

Create the Node test with temporary executable `docker`, `cargo`, and `sleep` commands. The fake Docker must implement `info`, `context inspect`, `run`, `exec`, `port`, `container ls`, `rm`, and `logs`; the fake Cargo records its environment and each argument on a separate line:

```js
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { access, chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '../..');
const runner = path.join(repoRoot, 'scripts/test_backend_school.sh');
const read = (file) => readFile(file, 'utf8');

async function writeExecutable(file, source) {
    await writeFile(file, source);
    await chmod(file, 0o755);
}

async function fixture(t) {
    const root = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-test-db-'));
    const bin = path.join(root, 'bin');
    const dockerLog = path.join(root, 'docker.log');
    const cargoLog = path.join(root, 'cargo.log');
    const containerState = path.join(root, 'container.exists');
    await mkdir(bin);
    t.after(() => rm(root, { recursive: true, force: true }));

    await writeExecutable(path.join(bin, 'docker'), `#!/usr/bin/env bash
set -u
command_name=\${1-}
if ((\$# > 0)); then shift; fi
{
    printf 'command=%s\\n' "\$command_name"
    for argument in "\$@"; do printf 'arg=%s\\n' "\$argument"; done
} >> "\$FAKE_DOCKER_LOG"
case "\$command_name" in
    info) exit "\${FAKE_DOCKER_INFO_STATUS:-0}" ;;
    context) printf '%s\\n' "\${FAKE_DOCKER_ENDPOINT:-unix:///var/run/docker.sock}" ;;
    run)
        previous=''
        container_name=''
        for argument in "\$@"; do
            if [[ \$previous == --name ]]; then container_name="\$argument"; fi
            previous="\$argument"
        done
        printf '%s\\n' "\$container_name" > "\$FAKE_CONTAINER_STATE"
        if [[ "\${FAKE_DOCKER_RUN_STATUS:-0}" != 0 ]]; then
            exit "\$FAKE_DOCKER_RUN_STATUS"
        fi
        printf '%s\\n' fake-container-id
        ;;
    exec) exit "\${FAKE_DOCKER_READY_STATUS:-0}" ;;
    port) printf '127.0.0.1:%s\\n' "\${FAKE_DOCKER_PORT:-55432}" ;;
    container)
        if [[ "\${1-}" == ls && -f "\$FAKE_CONTAINER_STATE" ]]; then
            /usr/bin/cat "\$FAKE_CONTAINER_STATE"
        fi
        ;;
    rm)
        if [[ "\${FAKE_DOCKER_REMOVE_STATUS:-0}" != 0 ]]; then
            exit "\$FAKE_DOCKER_REMOVE_STATUS"
        fi
        /usr/bin/rm -f "\$FAKE_CONTAINER_STATE"
        ;;
    logs) printf '%s\\n' 'fake postgres startup log' ;;
    *) exit 64 ;;
esac
`);
    await writeExecutable(path.join(bin, 'cargo'), `#!/usr/bin/env bash
{
    printf 'url=%s\\n' "\${TEST_DATABASE_URL-}"
    for argument in "\$@"; do printf 'arg=%s\\n' "\$argument"; done
} > "\$FAKE_CARGO_LOG"
if [[ -n \${FAKE_CARGO_BLOCK_FILE-} ]]; then
    : > "\$FAKE_CARGO_BLOCK_FILE"
    trap 'exit 143' TERM
    while :; do /bin/sleep 1; done
fi
exit "\${FAKE_CARGO_STATUS:-0}"
`);
    await writeExecutable(path.join(bin, 'sleep'), '#!/usr/bin/env bash\nexit 0\n');

    return {
        root,
        dockerLog,
        cargoLog,
        containerState,
        env: {
            ...process.env,
            PATH: `${bin}:${process.env.PATH}`,
            FAKE_DOCKER_LOG: dockerLog,
            FAKE_CARGO_LOG: cargoLog,
            FAKE_CONTAINER_STATE: containerState,
            TEST_DATABASE_URL: 'postgresql://must-not-survive.example/remote'
        }
    };
}

function runRunner(f, args = [], extraEnv = {}) {
    return spawnSync(runner, args, {
        cwd: f.root,
        env: { ...f.env, ...extraEnv },
        encoding: 'utf8'
    });
}

test('runner overrides remote URL, forwards arguments, and cleans up', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [
        'modules::auth::session_repository_tests',
        '--',
        '--nocapture'
    ]);

    assert.equal(result.status, 0, result.stderr);
    assert.equal(
        await read(f.cargoLog),
        [
            'url=postgresql://schoolorbit_test:schoolorbit_test@127.0.0.1:55432/schoolorbit_test?sslmode=disable',
            'arg=test',
            'arg=--bin',
            'arg=backend-school',
            'arg=modules::auth::session_repository_tests',
            'arg=--',
            'arg=--nocapture',
            ''
        ].join('\n')
    );
    await assert.rejects(read(f.containerState));
    assert.doesNotMatch(`${result.stdout}${result.stderr}${await read(f.dockerLog)}`, /must-not-survive/);
});
```

- [x] **Step 2: Run the test and verify RED**

Run:

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
```

Expected: FAIL because `scripts/test_backend_school.sh` does not exist.

- [x] **Step 3: Add failing lifecycle and safety cases**

Add tests with these exact outcomes:

```js
test('no arguments select the backend-school binary target', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f);
    assert.equal(result.status, 0, result.stderr);
    assert.match(await read(f.cargoLog), /arg=test\narg=--bin\narg=backend-school\n$/);
});

test('cargo failure survives successful cleanup', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_CARGO_STATUS: '23' });
    assert.equal(result.status, 23);
    await assert.rejects(read(f.containerState));
});

test('readiness failure skips cargo and cleans up', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_DOCKER_READY_STATUS: '1' });
    assert.notEqual(result.status, 0);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /PostgreSQL did not become ready/);
});

test('startup failure skips cargo and removes a partially created container', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_DOCKER_RUN_STATUS: '17' });
    assert.notEqual(result.status, 0);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
});

for (const endpoint of ['tcp://db.example:2376', 'ssh://schoolorbit@vps.example']) {
    test(`remote Docker endpoint is rejected: ${endpoint}`, async (t) => {
        const f = await fixture(t);
        const result = runRunner(f, [], { FAKE_DOCKER_ENDPOINT: endpoint });
        assert.notEqual(result.status, 0);
        await assert.rejects(read(f.cargoLog));
        assert.match(result.stderr, /local Docker engine/);
        assert.doesNotMatch(await read(f.dockerLog), /command=info/);
    });
}

test('cleanup failure makes success fail but preserves a cargo failure', async (t) => {
    const successfulCargo = await fixture(t);
    const cleanupOnly = runRunner(successfulCargo, [], {
        FAKE_DOCKER_REMOVE_STATUS: '19'
    });
    assert.equal(cleanupOnly.status, 1);

    const failedCargo = await fixture(t);
    const both = runRunner(failedCargo, [], {
        FAKE_CARGO_STATUS: '23',
        FAKE_DOCKER_REMOVE_STATUS: '19'
    });
    assert.equal(both.status, 23);
    assert.match(both.stderr, /failed to remove disposable PostgreSQL container/);
});

test('container is loopback-only, volume-free, and durability-tuned', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f);
    assert.equal(result.status, 0, result.stderr);
    const docker = await read(f.dockerLog);
    assert.match(docker, /arg=127\.0\.0\.1::5432/);
    assert.match(docker, /arg=\/var\/lib\/postgresql:rw,size=1g/);
    assert.match(docker, /arg=fsync=off/);
    assert.match(docker, /arg=synchronous_commit=off/);
    assert.match(docker, /arg=full_page_writes=off/);
    assert.doesNotMatch(docker, /arg=--volume|arg=-v/);
});

test('TERM preserves signal status and removes the owned container', async (t) => {
    const f = await fixture(t);
    const cargoStarted = path.join(f.root, 'cargo.started');
    const child = spawn(runner, [], {
        cwd: f.root,
        env: { ...f.env, FAKE_CARGO_BLOCK_FILE: cargoStarted },
        detached: true,
        stdio: ['ignore', 'pipe', 'pipe']
    });
    t.after(() => {
        try {
            process.kill(-child.pid, 'SIGKILL');
        } catch (error) {
            if (error.code !== 'ESRCH') throw error;
        }
    });

    for (let attempt = 0; attempt < 200; attempt += 1) {
        try {
            await access(cargoStarted);
            break;
        } catch {
            await new Promise((resolve) => setTimeout(resolve, 10));
        }
    }
    await access(cargoStarted);
    process.kill(-child.pid, 'SIGTERM');
    const exit = await new Promise((resolve) => {
        child.once('exit', (code, signal) => resolve({ code, signal }));
    });

    assert.deepEqual(exit, { code: 143, signal: null });
    await assert.rejects(read(f.containerState));
});
```

- [x] **Step 4: Run the expanded tests and confirm RED remains attributable to the absent runner**

Run the Node command again. Fix only fixture defects; do not weaken any assertion.

- [x] **Step 5: Implement the minimal runner**

Implement these exact lifecycle elements in `scripts/test_backend_school.sh`:

```bash
#!/usr/bin/env bash
set -uo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPOSITORY_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
readonly BACKEND_DIR="$REPOSITORY_ROOT/backend-school"
readonly POSTGRES_IMAGE='docker.io/library/postgres:18.4-alpine@sha256:9a8afca54e7861fd90fab5fdf4c42477a6b1cb7d293595148e674e0a3181de15'
readonly POSTGRES_USER='schoolorbit_test'
readonly POSTGRES_PASSWORD='schoolorbit_test'
readonly POSTGRES_DATABASE='schoolorbit_test'
readonly CONTAINER_NAME="schoolorbit-backend-school-test-$$-${RANDOM}"
cleanup_armed=false

cleanup() {
    local original_status=$?
    local cleanup_status=0
    trap - EXIT
    if [[ $cleanup_armed == true ]]; then
        local existing_container
        if ! existing_container="$(
            docker container ls --all \
                --filter "name=^/${CONTAINER_NAME}$" \
                --format '{{.Names}}'
        )"; then
            printf 'ERROR: failed to inspect disposable PostgreSQL container %s\n' \
                "$CONTAINER_NAME" >&2
            cleanup_status=1
        elif [[ $existing_container == "$CONTAINER_NAME" ]] && \
            ! docker rm --force "$CONTAINER_NAME" >/dev/null; then
            printf 'ERROR: failed to remove disposable PostgreSQL container %s\n' \
                "$CONTAINER_NAME" >&2
            cleanup_status=1
        fi
    fi
    if ((original_status != 0)); then
        exit "$original_status"
    fi
    exit "$cleanup_status"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
```

Check `command -v docker` first. Determine the effective endpoint from `DOCKER_HOST` when it is
set; otherwise read local context metadata with
`docker context inspect --format '{{(index .Endpoints "docker").Host}}'`. Accept only
`unix://*` and `npipe://*`. Reject every other endpoint before calling `docker info`, so validation
cannot contact a remote daemon. Call `docker info` only after the endpoint passes.

```bash
if ! command -v docker >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: Docker Desktop is required for backend-school database tests' >&2
    exit 127
fi

if [[ -n ${DOCKER_HOST-} ]]; then
    docker_endpoint=$DOCKER_HOST
elif ! docker_endpoint="$(
    docker context inspect --format '{{(index .Endpoints "docker").Host}}'
)"; then
    printf '%s\n' 'ERROR: unable to inspect the active Docker context' >&2
    exit 69
fi

case "$docker_endpoint" in
    unix://* | npipe://*) ;;
    *)
        printf 'ERROR: backend-school tests require a local Docker engine; got %s\n' \
            "$docker_endpoint" >&2
        exit 64
        ;;
esac

if ! docker info >/dev/null 2>&1; then
    printf '%s\n' 'ERROR: the local Docker engine is not reachable' >&2
    exit 69
fi
```

Set `cleanup_armed=true` immediately before attempting `docker run`, then start the container with
these exact material arguments inside `if ! ...; then exit 70; fi` so an image/startup failure
cannot fall through into readiness polling. Do not pass `--rm`: the exit trap remains the single
cleanup owner, and a container that exits during PostgreSQL 18 startup must remain inspectable long
enough to print its local logs before removal:

```bash
cleanup_armed=true
if ! docker run --detach \
    --name "$CONTAINER_NAME" \
    --publish '127.0.0.1::5432' \
    --tmpfs '/var/lib/postgresql:rw,size=1g' \
    --env "POSTGRES_USER=$POSTGRES_USER" \
    --env "POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
    --env "POSTGRES_DB=$POSTGRES_DATABASE" \
    "$POSTGRES_IMAGE" \
    postgres \
    -c fsync=off \
    -c synchronous_commit=off \
    -c full_page_writes=off \
    -c max_connections=200 \
    >/dev/null; then
    printf '%s\n' 'ERROR: failed to start disposable PostgreSQL' >&2
    exit 70
fi
```

Poll `docker exec "$CONTAINER_NAME" pg_isready --quiet --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE"` at 250 ms for at most 30 seconds. After a failed readiness probe, inspect
`.State.Running`; if PostgreSQL exited, print the last 50 local container log lines and fail
immediately. On timeout, print the same bounded logs and exit non-zero.

Before resolving the host port or starting Cargo, provision the baseline extensions in `public`.
This preserves the immutable baseline's explicit `public.uuid_generate_v4()` references on a fresh
database even though each SQLx test migration uses an isolated schema-first search path:

```bash
readonly TEST_EXTENSION_SQL='CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public; CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;'
if ! docker exec "$CONTAINER_NAME" \
    psql --no-psqlrc --username "$POSTGRES_USER" --dbname "$POSTGRES_DATABASE" \
    --set ON_ERROR_STOP=1 --command "$TEST_EXTENSION_SQL" \
    >/dev/null; then
    printf '%s\n' 'ERROR: failed to provision PostgreSQL test extensions' >&2
    exit 70
fi
```

Resolve and validate `docker port "$CONTAINER_NAME" 5432/tcp` against
`^127\.0\.0\.1:[0-9]+$`, then run:

```bash
readonly LOCAL_TEST_DATABASE_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${postgres_port}/${POSTGRES_DATABASE}?sslmode=disable"
cd "$BACKEND_DIR"
TEST_DATABASE_URL="$LOCAL_TEST_DATABASE_URL" cargo test --bin backend-school "$@"
```

Print the generated container name, but never print `LOCAL_TEST_DATABASE_URL`. Mark the file executable.

- [x] **Step 6: Make the focused tests GREEN and validate shell syntax/style**

Run:

```bash
bash -n scripts/test_backend_school.sh
node --test scripts/tests/backend-school-test-database.test.mjs
docker run --rm -v "$PWD:/repo:ro" -w /repo koalaman/shellcheck-alpine:v0.11.0 \
    /bin/shellcheck scripts/test_backend_school.sh
docker run --rm -v "$PWD:/repo:ro" -w /repo mvdan/shfmt:v3.11.0 \
    -d -i 4 -ci scripts/test_backend_school.sh
```

Expected: PASS. Report an unavailable validator image as unrun rather than replacing a failure.

- [x] **Step 7: Run one real focused test and prove the container is gone**

```bash
/usr/bin/time -f 'elapsed=%e seconds' \
    ./scripts/test_backend_school.sh modules::auth::session_schema_tests -- --nocapture
docker ps --all --filter 'name=schoolorbit-backend-school-test-' --format '{{.Names}}'
```

Expected: the Rust test passes and the Docker query prints nothing.

- [x] **Step 8: Commit the local runner**

```bash
git add scripts/test_backend_school.sh scripts/tests/backend-school-test-database.test.mjs
git commit -m "test(backend-school): use disposable local postgres"
```

---

### Task 2: Explicit Disposable Neon Compatibility Gate

**Files:**

- Modify: `scripts/tests/backend-school-test-database.test.mjs`
- Create: `.github/workflows/backend-school-neon-compatibility.yml`

**Interfaces:**

- Consumes: secret `NEON_TEST_API_KEY`; variables `NEON_TEST_PROJECT_ID`, `NEON_TEST_PARENT_BRANCH_ID`, `NEON_TEST_DATABASE`, and `NEON_TEST_ROLE`.
- Produces: manual workflow `Backend School Neon Compatibility`; unique schema-only branch `schoolorbit-test-<run_id>-<run_attempt>`; masked direct `TEST_DATABASE_URL`; unconditional delete by branch ID.

- [x] **Step 1: Add a failing static workflow contract**

Append this test before creating the workflow:

```js
test('Neon gate is manual, direct, disposable, and test-scoped', async () => {
    const workflow = await read(
        path.join(repoRoot, '.github/workflows/backend-school-neon-compatibility.yml')
    );

    assert.match(workflow, /workflow_dispatch:/);
    assert.doesNotMatch(workflow, /^\s{2}(?:push|pull_request|schedule):/m);
    for (const name of [
        'NEON_TEST_API_KEY',
        'NEON_TEST_PROJECT_ID',
        'NEON_TEST_PARENT_BRANCH_ID',
        'NEON_TEST_DATABASE',
        'NEON_TEST_ROLE'
    ]) {
        assert.match(workflow, new RegExp(name));
    }
    assert.match(
        workflow,
        /neondatabase\/create-branch-action@72ed4f69a12b6be9c16aebfad893f6a21e9aba8b/
    );
    assert.match(workflow, /branch_type:\s*schema-only/);
    assert.match(workflow, /expires_at:/);
    assert.match(
        workflow,
        /TEST_DATABASE_URL:\s*\$\{\{ steps\.create_branch\.outputs\.db_url \}\}/
    );
    assert.doesNotMatch(workflow, /db_url_pooled/);

    const createAt = workflow.indexOf('id: create_branch');
    const testAt = workflow.indexOf('name: Run direct-endpoint compatibility tests');
    const deleteAt = workflow.indexOf('name: Delete disposable Neon branch');
    assert.ok(createAt >= 0 && testAt > createAt && deleteAt > testAt);
    const deletion = workflow.slice(deleteAt);
    assert.match(deletion, /if:\s*\$\{\{ always\(\)/);
    assert.match(deletion, /steps\.create_branch\.outputs\.created == 'true'/);
    assert.match(
        deletion,
        /\/projects\/\$\{NEON_TEST_PROJECT_ID\}\/branches\/\$\{NEON_BRANCH_ID\}/
    );
    assert.match(deletion, /200\|204/);
    assert.doesNotMatch(workflow, /SERVER_|SSH_|podman|deploy/i);
});
```

- [x] **Step 2: Run the Node test and verify RED**

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
```

Expected: only the Neon workflow case fails with `ENOENT`.

- [x] **Step 3: Create the manual workflow**

Use this exact lifecycle in `.github/workflows/backend-school-neon-compatibility.yml`:

```yaml
name: Backend School Neon Compatibility

on:
  workflow_dispatch:
    inputs:
      confirm_disposable_branch:
        description: Create and delete a disposable Neon test branch
        required: true
        default: false
        type: boolean

permissions:
  contents: read

jobs:
  compatibility:
    if: ${{ inputs.confirm_disposable_branch }}
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    env:
      FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

    steps:
      - name: Checkout repository
        uses: actions/checkout@v6

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Restore Rust dependency cache
        uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32
        with:
          shared-key: backend-school-contracts
          workspaces: backend-school -> target
          save-if: "false"

      - name: Validate test-only Neon configuration
        env:
          NEON_TEST_API_KEY: ${{ secrets.NEON_TEST_API_KEY }}
          NEON_TEST_PROJECT_ID: ${{ vars.NEON_TEST_PROJECT_ID }}
          NEON_TEST_PARENT_BRANCH_ID: ${{ vars.NEON_TEST_PARENT_BRANCH_ID }}
          NEON_TEST_DATABASE: ${{ vars.NEON_TEST_DATABASE }}
          NEON_TEST_ROLE: ${{ vars.NEON_TEST_ROLE }}
        run: |
          set -euo pipefail
          for name in NEON_TEST_API_KEY NEON_TEST_PROJECT_ID \
            NEON_TEST_PARENT_BRANCH_ID NEON_TEST_DATABASE NEON_TEST_ROLE
          do
            if [[ -z ${!name} ]]; then
              printf 'ERROR: missing test-only configuration: %s\n' "$name" >&2
              exit 64
            fi
          done
          [[ $NEON_TEST_PROJECT_ID =~ ^[a-z0-9-]{1,60}$ ]]
          [[ $NEON_TEST_PARENT_BRANCH_ID =~ ^br-[a-z0-9-]+$ ]]

      - name: Calculate branch expiration
        id: expiration
        run: echo "expires_at=$(date -u --date '+2 hours' +'%Y-%m-%dT%H:%M:%SZ')" >> "$GITHUB_OUTPUT"

      - name: Create disposable Neon branch
        id: create_branch
        uses: neondatabase/create-branch-action@72ed4f69a12b6be9c16aebfad893f6a21e9aba8b # v6.4.0
        with:
          api_key: ${{ secrets.NEON_TEST_API_KEY }}
          project_id: ${{ vars.NEON_TEST_PROJECT_ID }}
          parent_branch: ${{ vars.NEON_TEST_PARENT_BRANCH_ID }}
          database: ${{ vars.NEON_TEST_DATABASE }}
          role: ${{ vars.NEON_TEST_ROLE }}
          branch_name: schoolorbit-test-${{ github.run_id }}-${{ github.run_attempt }}
          branch_type: schema-only
          ssl: require
          suspend_timeout: 60
          expires_at: ${{ steps.expiration.outputs.expires_at }}

      - name: Verify this run created a fresh branch
        env:
          BRANCH_CREATED: ${{ steps.create_branch.outputs.created }}
          BRANCH_ID: ${{ steps.create_branch.outputs.branch_id }}
        run: |
          set -euo pipefail
          [[ $BRANCH_CREATED == true ]]
          [[ $BRANCH_ID =~ ^br-[a-z0-9-]+$ ]]

      - name: Run direct-endpoint compatibility tests
        working-directory: backend-school
        env:
          TEST_DATABASE_URL: ${{ steps.create_branch.outputs.db_url }}
        run: |
          set -euo pipefail
          echo "::add-mask::$TEST_DATABASE_URL"
          cargo test modules::auth::session_schema_tests --bin backend-school -- --nocapture
          cargo test modules::files::schema_tests --bin backend-school -- --nocapture

      - name: Delete disposable Neon branch
        if: ${{ always() && steps.create_branch.outputs.created == 'true' && steps.create_branch.outputs.branch_id != '' }}
        env:
          NEON_TEST_API_KEY: ${{ secrets.NEON_TEST_API_KEY }}
          NEON_TEST_PROJECT_ID: ${{ vars.NEON_TEST_PROJECT_ID }}
          NEON_BRANCH_ID: ${{ steps.create_branch.outputs.branch_id }}
        run: |
          set -euo pipefail
          curl_config="$(mktemp)"
          response_file="$(mktemp)"
          trap 'rm -f "$curl_config" "$response_file"' EXIT
          chmod 0600 "$curl_config" "$response_file"
          printf 'header = "Authorization: Bearer %s"\n' "$NEON_TEST_API_KEY" > "$curl_config"
          http_status="$(
            curl --silent --show-error \
              --config "$curl_config" \
              --output "$response_file" \
              --write-out '%{http_code}' \
              --request DELETE \
              --url "https://console.neon.tech/api/v2/projects/${NEON_TEST_PROJECT_ID}/branches/${NEON_BRANCH_ID}"
          )"
          case "$http_status" in
            200|204) ;;
            *)
              printf 'ERROR: Neon branch cleanup failed: branch=%s status=%s\n' \
                "$NEON_BRANCH_ID" "$http_status" >&2
              exit 1
              ;;
          esac
```

The branch name is unique to the run attempt. The delete condition also requires `created=true`, so the workflow never deletes a pre-existing branch returned after a collision. Expiration is only a fallback for cancellation before finalization.

- [x] **Step 4: Make the workflow contract GREEN and run actionlint**

```bash
node --test scripts/tests/backend-school-test-database.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7 \
    .github/workflows/backend-school-neon-compatibility.yml
```

Expected: PASS.

- [x] **Step 5: Commit the Neon gate**

```bash
git add .github/workflows/backend-school-neon-compatibility.yml \
    scripts/tests/backend-school-test-database.test.mjs
git commit -m "ci(backend-school): add disposable Neon compatibility gate"
```

---

### Task 3: Canonical Test Database Policy and Commands

**Files:**

- Modify: `.rules:151-156`
- Modify: `docs/TESTING.md:43-61,170-186`
- Modify: `backend-school/README.md:25-35`
- Modify: `TODO.md` verification baseline

**Interfaces:**

- Consumes: runner and workflow from Tasks 1-2.
- Produces: one canonical routine command, focused-test recipe, explicit test-only Neon configuration inventory, and measurable unfinished backlog wording.

Human-facing prose does not receive a source-text change detector. Existing
`frontend-school/tests/static/documentation-policy.test.mjs` continues to validate the canonical
file allowlist and local links.

- [x] **Step 1: Update `.rules` with the durable boundary**

Replace the database-test bullet with this policy, reflowed to the file's line width:

```text
Database-backed tests use TEST_DATABASE_URL and isolated test state. Routine backend-school
database tests run through scripts/test_backend_school.sh, which supplies disposable local
PostgreSQL and removes it after the command. Neon migration/schema compatibility runs only
through the explicit disposable-branch gate and uses its direct endpoint, never a -pooler
transaction endpoint.
```

- [x] **Step 2: Replace routine database commands in `docs/TESTING.md`**

Document these root commands exactly:

```bash
# Complete backend-school binary suite; PostgreSQL runs in Docker Desktop on this computer.
./scripts/test_backend_school.sh

# Focused database-backed test.
./scripts/test_backend_school.sh \
  modules::auth::session_repository_tests -- --nocapture
```

Document that Docker Desktop WSL integration must be active; only a local Docker endpoint is accepted; Cargo cache remains on the computer; no persistent database volume exists; and success, failure, and ordinary signals all clean up. State that direct manual Cargo against a persistent Neon URL is no longer the routine recipe.

Add a separate manual-gate subsection with no values:

```text
Secret:    NEON_TEST_API_KEY
Variables: NEON_TEST_PROJECT_ID
           NEON_TEST_PARENT_BRANCH_ID
           NEON_TEST_DATABASE
           NEON_TEST_ROLE
```

Explain that the project/parent must be test-only, the workflow uses direct `db_url`, the branch has a two-hour fallback expiry, and the finalizer deletes it.

- [x] **Step 3: Keep the README and backlog concise**

In `backend-school/README.md`, replace the generic database-test sentence with the root runner invocation and a link to `docs/TESTING.md`; do not duplicate lifecycle details.

Keep both verification items unchecked in `TODO.md`, but make them measurable:

```text
- [ ] Backend-school formatting, check, static architecture tests, and the full test suite pass
  through disposable local PostgreSQL without timing-dependent retry failures.
- [ ] Fresh PostgreSQL migration tests pass locally, and the explicit disposable Neon
  compatibility gate passes for every active migration without editing applied migration files.
```

- [x] **Step 4: Validate canonical documentation**

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
git diff --check
```

Expected: PASS. No Svelte file is analyzed or modified.

- [x] **Step 5: Commit the durable policy and recipes**

```bash
git add .rules docs/TESTING.md backend-school/README.md TODO.md
git commit -m "docs(test): standardize disposable database testing"
```

---

### Task 4: Real Verification, Timing, and One-time Neon Cleanup

**Files:**

- Verify only; no new tracked file is expected.
- If a defect appears, return to its owning task, add a failing regression test, implement the minimum fix, and make a focused follow-up commit.

**Interfaces:**

- Consumes: Tasks 1-3 and ignored credentials for the dedicated test Neon database.
- Produces: cold/warm timings, a clean Docker lifecycle, verified checks, and zero retained `schoolorbit_test_*` schemas in the dedicated `schoolorbit_test` database.

- [ ] **Step 1: Run the complete local suite twice and record timings**

```bash
/usr/bin/time -f 'cold elapsed=%e seconds' ./scripts/test_backend_school.sh
/usr/bin/time -f 'warm elapsed=%e seconds' ./scripts/test_backend_school.sh
docker ps --all --filter 'name=schoolorbit-backend-school-test-' --format '{{.Names}}'
```

Expected: both suites PASS and the Docker query is empty. Report Cargo compilation separately from test execution; do not claim a percentage without a comparable measurement.

- [ ] **Step 2: Run the complete applicable local matrix**

```bash
bash -n scripts/test_backend_school.sh
node --test scripts/tests/backend-school-test-database.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
docker run --rm -v "$PWD:/repo:ro" -w /repo koalaman/shellcheck-alpine:v0.11.0 \
    /bin/shellcheck scripts/test_backend_school.sh
docker run --rm -v "$PWD:/repo:ro" -w /repo mvdan/shfmt:v3.11.0 \
    -d -i 4 -ci scripts/test_backend_school.sh
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7 \
    .github/workflows/backend-school-neon-compatibility.yml
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cd ..
git diff --check
git status --short
```

Expected: every available command PASS. Report an unavailable external validator separately.

- [ ] **Step 3: Run the manual Neon gate only when test-only configuration exists**

Dispatch `backend-school-neon-compatibility.yml` with `confirm_disposable_branch=true`. Verify configuration validation, `created=true`, both direct-endpoint test commands, the delete finalizer, and absence of the exact `schoolorbit-test-<run_id>-<run_attempt>` branch afterward.

If `NEON_TEST_*` is unavailable, report the gate as unrun and list only missing variable names. Never substitute a production project or persistent URL.

- [ ] **Step 4: Revalidate the existing Neon cleanup target without mutation**

Use the ignored `backend-school/.env` URL only in process memory and remove `-pooler.` from the authority before connecting. Supply libpq fields/password through the child environment, never a printed command argument. Run:

```sql
SELECT current_database(), current_setting('server_version');

SELECT n.nspname, count(c.oid) AS relation_count
FROM pg_namespace AS n
LEFT JOIN pg_class AS c ON c.relnamespace = n.oid
WHERE n.nspname LIKE 'schoolorbit_test\_%' ESCAPE '\'
GROUP BY n.nspname
ORDER BY n.nspname;

SELECT count(*) AS other_active_connections
FROM pg_stat_activity
WHERE datname = current_database()
  AND pid <> pg_backend_pid()
  AND state <> 'idle';
```

Require database `schoolorbit_test`, exactly 43 matching schemas at the approved checkpoint, and zero other active connections. Stop and re-analyze if any condition changes. Report schema names/counts, never the URL.

- [ ] **Step 5: Drop only the validated schemas in one transaction**

Tell the user immediately before this step that the exact 43 reproducible test schemas will be removed and are not application-recoverable. Then execute:

```sql
\set ON_ERROR_STOP on

SELECT current_database() = 'schoolorbit_test' AS database_is_safe \gset
\if :database_is_safe
\else
    \echo 'ERROR: refusing cleanup outside schoolorbit_test'
    \quit 3
\endif

SELECT count(*) = 43 AS schema_count_is_safe
FROM pg_namespace
WHERE nspname LIKE 'schoolorbit_test\_%' ESCAPE '\' \gset
\if :schema_count_is_safe
\else
    \echo 'ERROR: test schema count changed; revalidate before cleanup'
    \quit 4
\endif

SELECT count(*) = 0 AS no_active_test_work
FROM pg_stat_activity
WHERE datname = current_database()
  AND pid <> pg_backend_pid()
  AND state <> 'idle' \gset
\if :no_active_test_work
\else
    \echo 'ERROR: another database operation is active; cleanup stopped'
    \quit 5
\endif

BEGIN;
SELECT format('DROP SCHEMA %I CASCADE;', nspname)
FROM pg_namespace
WHERE nspname LIKE 'schoolorbit_test\_%' ESCAPE '\'
ORDER BY nspname
\gexec
COMMIT;
```

Do not terminate other sessions, drop `public`, broaden the prefix, or commit this one-time SQL as a utility.

- [ ] **Step 6: Verify cleanup and final repository state**

Run read-only counts:

```sql
SELECT count(*) AS remaining_test_schemas
FROM pg_namespace
WHERE nspname LIKE 'schoolorbit_test\_%' ESCAPE '\';

SELECT count(*) AS remaining_test_relations
FROM pg_class AS c
JOIN pg_namespace AS n ON n.oid = c.relnamespace
WHERE n.nspname LIKE 'schoolorbit_test\_%' ESCAPE '\';

SELECT pg_size_pretty(pg_database_size(current_database())) AS live_database_size;
```

Expected: both counts are zero. Note that Neon history/restore retention can delay visible storage reduction. Finish with:

```bash
git log --oneline -5
git diff --check
git status --short --branch
```

Expected: focused implementation commits exist, the worktree is clean, and the branch remains ahead of origin until push is separately authorized.
