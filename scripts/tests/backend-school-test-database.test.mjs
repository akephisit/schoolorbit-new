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

async function fixture(t, { withPodman = true } = {}) {
    const root = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-test-db-'));
    const bin = path.join(root, 'bin');
    const podmanLog = path.join(root, 'podman.log');
    const cargoLog = path.join(root, 'cargo.log');
    const containerState = path.join(root, 'container.exists');
    await mkdir(bin);
    t.after(() => rm(root, { recursive: true, force: true }));

    await writeExecutable(path.join(bin, 'bash'), '#!/bin/sh\nexec /bin/bash "$@"\n');
    await writeExecutable(path.join(bin, 'dirname'), '#!/bin/sh\nexec /usr/bin/dirname "$@"\n');
    for (const command of ['grep', 'mktemp', 'rm', 'tee']) {
        await writeExecutable(
            path.join(bin, command),
            `#!/bin/sh\nexec /usr/bin/${command} "$@"\n`
        );
    }

    if (withPodman) {
        await writeExecutable(
            path.join(bin, 'podman'),
            `#!/usr/bin/env bash
set -u
command_name=\${1-}
if ((\$# > 0)); then shift; fi
{
    printf 'command=%s\\n' "\$command_name"
    for argument in "\$@"; do printf 'arg=%s\\n' "\$argument"; done
} >> "\$FAKE_PODMAN_LOG"
case "\$command_name" in
    info)
        if [[ " \$* " == *' --format '* ]]; then
            printf '%s\\n' "\${FAKE_PODMAN_ROOTLESS:-true}"
        fi
        exit "\${FAKE_PODMAN_INFO_STATUS:-0}"
        ;;
    run)
        previous=''
        container_name=''
        for argument in "\$@"; do
            if [[ \$previous == --name ]]; then container_name="\$argument"; fi
            previous="\$argument"
        done
        printf '%s\\n' "\$container_name" > "\$FAKE_CONTAINER_STATE"
        if [[ "\${FAKE_PODMAN_RUN_STATUS:-0}" != 0 ]]; then
            exit "\$FAKE_PODMAN_RUN_STATUS"
        fi
        printf '%s\\n' fake-container-id
        ;;
    exec)
        case " \$* " in
            *' pg_isready '*)
                if [[ \${FAKE_PODMAN_REQUIRE_TCP_PROBE:-false} == true ]]; then
                    case " \$* " in
                        *' --host 127.0.0.1 '*) ;;
                        *) exit 1 ;;
                    esac
                fi
                exit "\${FAKE_PODMAN_READY_STATUS:-0}"
                ;;
            *' psql '*) exit "\${FAKE_PODMAN_BOOTSTRAP_STATUS:-0}" ;;
            *) exit 64 ;;
        esac
        ;;
    port) printf '%s\\n' "\${FAKE_PODMAN_BINDING:-127.0.0.1:55432}" ;;
    container)
        case "\${1-}" in
            exists) [[ -f "\$FAKE_CONTAINER_STATE" ]] ;;
            inspect) printf '%s\\n' "\${FAKE_CONTAINER_RUNNING:-true}" ;;
            *) exit 64 ;;
        esac
        ;;
    rm)
        if [[ "\${FAKE_PODMAN_REMOVE_STATUS:-0}" != 0 ]]; then
            exit "\$FAKE_PODMAN_REMOVE_STATUS"
        fi
        /usr/bin/rm -f "\$FAKE_CONTAINER_STATE"
        ;;
    logs) printf '%s\\n' 'fake postgres startup log' ;;
    *) exit 64 ;;
esac
`
        );
    }
    await writeExecutable(
        path.join(bin, 'cargo'),
        `#!/usr/bin/env bash
{
    printf 'url=%s\\n' "\${TEST_DATABASE_URL-}"
    for argument in "\$@"; do printf 'arg=%s\\n' "\$argument"; done
} > "\$FAKE_CARGO_LOG"
if [[ -n \${FAKE_CARGO_BLOCK_FILE-} ]]; then
    : > "\$FAKE_CARGO_BLOCK_FILE"
    trap 'exit 143' TERM
    while :; do /bin/sleep 1; done
fi
case "\${FAKE_CARGO_REPORT:-passing}" in
    passing) printf '%s\n' 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' ;;
    zero) printf '%s\n' 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out' ;;
    none) ;;
    *) exit 64 ;;
esac
exit "\${FAKE_CARGO_STATUS:-0}"
`
    );
    await writeExecutable(path.join(bin, 'sleep'), '#!/usr/bin/env bash\nexit 0\n');

    return {
        root,
        podmanLog,
        cargoLog,
        containerState,
        env: {
            ...process.env,
            PATH: bin,
            FAKE_PODMAN_LOG: podmanLog,
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
    const result = runRunner(f, ['modules::auth::session_repository_tests', '--', '--nocapture']);

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
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
    assert.doesNotMatch(
        `${result.stdout}${result.stderr}${await read(f.podmanLog)}`,
        /must-not-survive/
    );
});

test('no arguments select the backend-school binary target', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f);

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    assert.match(await read(f.cargoLog), /arg=test\narg=--bin\narg=backend-school\n$/);
});

test('validated package mode selects the extracted crate and forwards its filter', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [
        '--package',
        'school-auth',
        'session_repository_tests',
        '--',
        '--nocapture'
    ]);

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    assert.equal(
        await read(f.cargoLog),
        [
            'url=postgresql://schoolorbit_test:schoolorbit_test@127.0.0.1:55432/schoolorbit_test?sslmode=disable',
            'arg=test',
            'arg=-p',
            'arg=school-auth',
            'arg=session_repository_tests',
            'arg=--',
            'arg=--nocapture',
            ''
        ].join('\n')
    );
});

for (const packageName of ['../backend-school', 'backend-school', 'missing-owner']) {
    test(`invalid package mode target ${packageName} fails before Podman`, async (t) => {
        const f = await fixture(t);
        const result = runRunner(f, ['--package', packageName]);

        assert.equal(result.status, 64);
        assert.match(result.stderr, /workspace package/);
        await assert.rejects(read(f.podmanLog));
        await assert.rejects(read(f.cargoLog));
    });
}

test('a focused command that matches zero tests fails instead of reporting success', async (t) => {
    const f = await fixture(t);
    const result = runRunner(
        f,
        ['--package', 'school-auth', 'removed_test_module'],
        { FAKE_CARGO_REPORT: 'zero' }
    );

    assert.equal(result.status, 65);
    assert.match(result.stderr, /matched zero tests/);
    await assert.rejects(read(f.containerState));
});

test('explicit local binary target runs seed sandbox tests in the same database boundary', async (t) => {
    const f = await fixture(t);
    const result = runRunner(
        f,
        ['canonical_seed_is_idempotent_across_student_year_and_placement', '--', '--exact'],
        { BACKEND_SCHOOL_TEST_BIN: 'seed_sandbox' }
    );

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    assert.equal(
        await read(f.cargoLog),
        [
            'url=postgresql://schoolorbit_test:schoolorbit_test@127.0.0.1:55432/schoolorbit_test?sslmode=disable',
            'arg=test',
            'arg=--bin',
            'arg=seed_sandbox',
            'arg=canonical_seed_is_idempotent_across_student_year_and_placement',
            'arg=--',
            'arg=--exact',
            ''
        ].join('\n')
    );
});

test('cargo failure status survives successful cleanup', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_CARGO_STATUS: '23' });

    assert.equal(result.status, 23);
    await assert.rejects(read(f.containerState));
});

test('startup failure skips cargo and removes a partially created container', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_RUN_STATUS: '17' });

    assert.equal(result.status, 70);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /failed to start disposable PostgreSQL/);
});

test('readiness failure skips cargo and removes the started container', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_READY_STATUS: '1' });

    assert.equal(result.status, 70);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /PostgreSQL did not become ready/);
});

test('container exit during readiness fails immediately and prints local logs', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], {
        FAKE_PODMAN_READY_STATUS: '1',
        FAKE_CONTAINER_RUNNING: 'false'
    });

    assert.equal(result.status, 70);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /fake postgres startup log/);
    assert.match(result.stderr, /PostgreSQL exited before becoming ready/);
    assert.equal((await read(f.podmanLog)).match(/^command=exec$/gm)?.length, 1);
});

test('readiness waits for the final local TCP server instead of the init socket', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_REQUIRE_TCP_PROBE: 'true' });

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    assert.match(await read(f.cargoLog), /arg=backend-school/);
});

test('runner provisions baseline extensions in public before cargo starts', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f);

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    const podman = await read(f.podmanLog);
    const readyAt = podman.indexOf('arg=pg_isready');
    const psqlAt = podman.indexOf('arg=psql');
    assert.ok(readyAt >= 0 && psqlAt > readyAt);
    assert.match(podman, /CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public/);
    assert.match(podman, /CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public/);
    assert.match(await read(f.cargoLog), /arg=backend-school/);
});

test('extension bootstrap failure skips cargo and cleans up', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_BOOTSTRAP_STATUS: '33' });

    assert.equal(result.status, 70);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /failed to provision PostgreSQL test extensions/);
});

test('missing Podman fails before Cargo runs', async (t) => {
    const f = await fixture(t, { withPodman: false });
    const result = runRunner(f);

    assert.equal(result.status, 127);
    assert.match(result.stderr, /Podman is required/);
    await assert.rejects(read(f.cargoLog));
});

for (const [variable, value] of [
    ['CONTAINER_HOST', 'ssh://server.example/run/user/1000/podman/podman.sock'],
    ['CONTAINER_CONNECTION', 'production']
]) {
    test(`${variable} remote selection is rejected before contacting Podman`, async (t) => {
        const f = await fixture(t);
        const result = runRunner(f, [], { [variable]: value });

        assert.equal(result.status, 64);
        await assert.rejects(read(f.cargoLog));
        assert.match(result.stderr, /local Podman engine/);
        await assert.rejects(read(f.podmanLog));
    });
}

test('a non-rootless Podman engine is rejected before creating a container', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_ROOTLESS: 'false' });

    assert.equal(result.status, 64);
    assert.match(result.stderr, /rootless Podman/);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
});

test('cleanup failure makes success fail but does not hide cargo failure', async (t) => {
    const successfulCargo = await fixture(t);
    const cleanupOnly = runRunner(successfulCargo, [], {
        FAKE_PODMAN_REMOVE_STATUS: '19'
    });
    assert.equal(cleanupOnly.status, 1);
    assert.match(cleanupOnly.stderr, /failed to remove disposable PostgreSQL container/);

    const failedCargo = await fixture(t);
    const both = runRunner(failedCargo, [], {
        FAKE_CARGO_STATUS: '23',
        FAKE_PODMAN_REMOVE_STATUS: '19'
    });
    assert.equal(both.status, 23);
    assert.match(both.stderr, /failed to remove disposable PostgreSQL container/);
});

test('container uses loopback and disposable disk storage instead of a capped data tmpfs', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f);

    assert.equal(result.status, 0, result.error?.message ?? result.stderr);
    const podman = await read(f.podmanLog);
    assert.match(podman, /arg=127\.0\.0\.1::5432/);
    assert.match(podman, /arg=--mount\narg=type=volume,destination=\/var\/lib\/postgresql\n/);
    assert.doesNotMatch(podman, /arg=--tmpfs/);
    assert.match(podman, /arg=--shm-size\narg=1g/);
    assert.match(podman, /arg=fsync=off/);
    assert.match(podman, /arg=synchronous_commit=off/);
    assert.match(podman, /arg=full_page_writes=off/);
    assert.doesNotMatch(podman, /arg=--volume\n|arg=-v\n|source=|src=/);
});

for (const cargoStatus of ['0', '23']) {
    test(`runner removes only its container and anonymous volumes after cargo status ${cargoStatus}`, async (t) => {
        const f = await fixture(t);
        const result = runRunner(f, [], { FAKE_CARGO_STATUS: cargoStatus });
        assert.equal(result.status, Number(cargoStatus));
        const podman = await read(f.podmanLog);
        const name = podman.match(/arg=--name\narg=([^\n]+)/)?.[1];
        assert.ok(name);
        assert.ok(podman.endsWith(`command=rm\narg=--force\narg=--volumes\narg=${name}\n`));
        assert.doesNotMatch(podman, /command=volume|command=system|arg=prune/);
        await assert.rejects(read(f.containerState));
    });
}

test('unexpected published address fails closed and cleans up', async (t) => {
    const f = await fixture(t);
    const result = runRunner(f, [], { FAKE_PODMAN_BINDING: '0.0.0.0:55432' });

    assert.equal(result.status, 70);
    await assert.rejects(read(f.cargoLog));
    await assert.rejects(read(f.containerState));
    assert.match(result.stderr, /unexpected PostgreSQL port binding/);
});

test('TERM preserves signal status and removes the owned container', async (t) => {
    const f = await fixture(t);
    const cargoStarted = path.join(f.root, 'cargo.started');
    const child = spawn(runner, [], {
        cwd: f.root,
        env: { ...f.env, FAKE_CARGO_BLOCK_FILE: cargoStarted },
        detached: true,
        stdio: 'ignore'
    });
    let childExited = false;
    const exitPromise = new Promise((resolve) => {
        child.once('exit', (code, signal) => {
            childExited = true;
            resolve({ code, signal });
        });
    });
    t.after(() => {
        if (childExited) return;
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
    const exit = await exitPromise;

    assert.deepEqual(exit, { code: 143, signal: null });
    await assert.rejects(read(f.containerState));
});

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
    assert.doesNotMatch(workflow, /neondatabase\/create-branch-action/);
    assert.match(workflow, /node scripts\/neon-create-test-branch\.mjs/);
    assert.doesNotMatch(workflow, /^\s*branch_type:/m);
    assert.doesNotMatch(workflow, /suspend_timeout/);
    assert.match(workflow, /NEON_BRANCH_EXPIRES_AT:/);
    assert.match(
        workflow,
        /TEST_DATABASE_URL:\s*\$\{\{ steps\.create_branch\.outputs\.db_url \}\}/
    );
    assert.doesNotMatch(workflow, /db_url_pooled/);

    const createAt = workflow.indexOf('id: create_branch');
    const provisionAt = workflow.indexOf('name: Provision migration prerequisite extensions');
    const testAt = workflow.indexOf('name: Run direct-endpoint compatibility tests');
    const deleteAt = workflow.indexOf('name: Delete disposable Neon branch');
    assert.ok(createAt >= 0 && provisionAt > createAt && testAt > provisionAt && deleteAt > testAt);
    const creation = workflow.slice(createAt, provisionAt);
    assert.match(
        creation,
        /NEON_BRANCH_NAME:\s*schoolorbit-test-\$\{\{ github\.run_id \}\}-\$\{\{ github\.run_attempt \}\}/
    );
    assert.match(
        creation,
        /NEON_BRANCH_EXPIRES_AT:\s*\$\{\{ steps\.expiration\.outputs\.expires_at \}\}/
    );
    const provision = workflow.slice(provisionAt, testAt);
    assert.match(
        provision,
        /TEST_DATABASE_URL:\s*\$\{\{ steps\.create_branch\.outputs\.db_url \}\}/
    );
    assert.match(provision, /command -v psql/);
    assert.match(provision, /--set=ON_ERROR_STOP=1/);
    assert.match(provision, /CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;/);
    assert.match(provision, /CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;/);
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
