import test from 'node:test';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, symlinkSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import path from 'node:path';

import {classify} from '../lib/pipeline-policy.mjs';
import {catalogue, verifyInputs, validateDispatch, relatedFiles, requireQuiescentBeforeDispatch} from '../lib/pipeline-owned-load-cleanup.mjs';

const repo = path.resolve(import.meta.dirname, '../..');
const operation = 'cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b';
const guard = path.join(repo, 'scripts/lib/pipeline-remote/owned-load-cleanup-interlock.sh');
const read = file => readFileSync(path.join(repo, file), 'utf8');

function fixture(t) {
  const stack = mkdtempSync(path.join(tmpdir(), 'owned-cleanup-guard-'));
  t.after(() => rmSync(stack, {recursive: true, force: true}));
  const state = path.join(stack, 'deployment/owned-load-cleanup');
  const journal = path.join(state, 'operations', operation);
  mkdirSync(journal, {recursive: true, mode: 0o700});
  const bin = path.join(stack, 'bin');
  mkdirSync(bin);
  writeFileSync(path.join(bin, 'podman'), '#!/bin/sh\nexit "${TEST_PODMAN_EXISTS_EXIT:-1}"\n', {mode: 0o700});
  const run = (extra = {}) => spawnSync('bash', [guard], {
    env: {...process.env, SCHOOLORBIT_STACK_ROOT: stack, PATH: `${bin}:${process.env.PATH}`, ...extra}, encoding: 'utf8'
  });
  return {stack, state, journal, run};
}

test('clear guard performs no writes and permits an unused installation', t => {
  const f = fixture(t);
  const before = execFileSync('find', [f.stack, '-type', 'f'], {encoding: 'utf8'});
  assert.equal(f.run().status, 0);
  assert.equal(execFileSync('find', [f.stack, '-type', 'f'], {encoding: 'utf8'}), before);
});

for (const kind of ['directory', 'dangling-symlink']) {
  test(`active ${kind} refuses every mutation`, t => {
    const f = fixture(t);
    const active = path.join(f.state, 'active');
    if (kind === 'directory') mkdirSync(active);
    else symlinkSync('missing', active);
    assert.equal(f.run().status, 78);
  });
}

function completedChain(f, watchOverrides = {}) {
  const hash = value => createHash('sha256').update(value).digest('hex');
  const inputSha256 = '9649a38dd08f91bcd1fbcb899a5fda28e7f0689cb0f06c55f77040b3177b33ae';
  const put = (name, fields) => {
    const raw = JSON.stringify({schemaVersion: 1, operationId: operation, ...fields});
    writeFileSync(path.join(f.journal, name), raw, {mode: 0o600});
    return hash(raw);
  };
  const binding = put('actual-binding.json', {subdomain: 'snwsb', acceptedRelease: '7'.repeat(40), tenantId: '0e297ca4-0809-4aab-a03f-1915045257b8', databaseSchemaVersion: 138,
    backend: {containerId: 'a'.repeat(64), pid: 123, restartCount: 0, startedAt: '2026-10-25T10:00:00Z', image: 'sha256:f983b8e4593e2a7b480b13643c9778e7b79a55c6fa7eb72f6987a8562fd89043'}});
  const approved = put('approved-snapshot.json', {outside: []});
  const common = {inputSha256, actualBindingSha256: binding, approvedSnapshotSha256: approved};
  const intent = put('dispatch-intent.json', {...common, status: 'dispatching'});
  const exited = put('postgres-exit.json', {...common, status: 'exited', exitCode: 0});
  const commit = put('commit-receipt.json', {...common, status: 'committed', postgresExitSha256: exited, postCommitVerified: true});
  const watch = put('watch-receipt.json', {...common, status: 'preserved', watchSeconds: 300, samples: 2, stableBackend: true, preserved: true, ...watchOverrides});
  put('completed.json', {...common, status: 'completed', watchSeconds: watchOverrides.watchSeconds ?? 300, dispatchIntentSha256: intent, commitReceiptSha256: commit, watchReceiptSha256: watch});
}

test('completion requires the actual immutable receipt and intent chain', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.journal, 'dispatch-intent.json'), '{}', {mode: 0o600});
  assert.equal(f.run().status, 78);
  writeFileSync(path.join(f.journal, 'completed.json'), JSON.stringify({schemaVersion: 1, operationId: operation,
    status: 'completed', watchSeconds: 300, commitReceiptSha256: 'a'.repeat(64), watchReceiptSha256: 'b'.repeat(64)}), {mode: 0o600});
  assert.equal(f.run().status, 78);
  completedChain(f);
  assert.equal(f.run().status, 0);
  writeFileSync(path.join(f.journal, 'commit-receipt.json'), '{}');
  assert.equal(f.run().status, 78);
});

for (const status of ['0', '125']) {
  test(`existing or uninspectable SQL client (${status}) refuses mutations`, t => {
    assert.equal(fixture(t).run({TEST_PODMAN_EXISTS_EXIT: status}).status, 78);
  });
}

for (const component of ['root', 'operations', 'journal']) {
  test(`malformed ${component} cannot appear clear`, t => {
    const f = fixture(t);
    const p = component === 'root' ? f.state : component === 'operations' ? path.dirname(f.journal) : f.journal;
    rmSync(p, {recursive: true});
    writeFileSync(p, 'wrong', {mode: 0o600});
    assert.equal(f.run().status, 78);
  });
}

for (const field of ['dispatch-intent.json', 'postgres-exit.json', 'actual-binding.json', 'watch-receipt.json']) {
  test(`missing ${field} or altered binding refuses completion`, t => {
    const f = fixture(t); completedChain(f);
    rmSync(path.join(f.journal, field));
    assert.equal(f.run().status, 78);
  });
}

for (const value of ['300', true, 300.5, -1]) {
  test(`invalid watch duration ${JSON.stringify(value)} cannot release interlock`, t => {
    const f = fixture(t); completedChain(f, {watchSeconds: value});
    assert.equal(f.run().status, 78);
  });
}

test('a symlinked ancestor refuses a misleading clear path', t => {
  const f = fixture(t);
  const alias = path.join(f.stack, 'alias'); symlinkSync(f.stack, alias);
  assert.equal(f.run({SCHOOLORBIT_STACK_ROOT: alias}).status, 78);
});

test('symlinked or public completion evidence fails closed', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.journal, 'dispatch-intent.json'), '{}', {mode: 0o600});
  writeFileSync(path.join(f.journal, 'receipt'), '{}', {mode: 0o600});
  symlinkSync('receipt', path.join(f.journal, 'completed.json'));
  assert.equal(f.run().status, 78);
});

test('every public and callable production mutator observes the guard before its first write', () => {
  const firstMutations = {
    'release.yml': 'Upload read-only Admin account preflight',
    'deploy-school-tenant.yml': 'Upload inactive accepted frontend',
    'deploy-backend-admin.yml': 'Upload canonical runtime and proxy templates',
    'deploy-frontend-admin.yml': 'Create missing Admin Worker with an isolated maintenance response',
    'deploy-school-release.yml': 'Upload canonical runtime',
    'api-tls.yml': 'Upload shared certificate and maintenance owners',
    'recreate-admin-frontend.yml': 'Upload canonical maintenance implementation',
    'maintenance.yml': 'Retain bounded backend releases'
  };
  for (const [file, mutation] of Object.entries(firstMutations)) {
    const source = read(`.github/workflows/${file}`);
    const observed = source.indexOf('uses: ./.github/actions/owned-cleanup-interlock');
    assert.ok(observed >= 0, `${file} missing guard`);
    assert.ok(source.indexOf(mutation) > observed, `${file} guard follows first write`);
  }
  const action = read('.github/actions/owned-cleanup-interlock/action.yml');
  assert.match(action, /script_path: scripts\/lib\/pipeline-remote\/owned-load-cleanup-interlock\.sh/);
  assert.doesNotMatch(action, /scp-action|upload-artifact/);
});


test('cleanup dispatch requires explicit fixed selectors and inert typed options', () => {
  const request = {request: catalogue.operationId, subdomain: catalogue.subdomain, schoolId: '', apiUrl: catalogue.apiOrigin, options: '{}'};
  assert.equal(validateDispatch(request).action, 'execute');
  assert.equal(validateDispatch({...request, options: '{"action":"reconcile"}'}).action, 'reconcile');
  for (const change of [{subdomain: 'sandbox'}, {schoolId: 'another'}, {apiUrl: 'https://outside.invalid'}, {request: ''}, {options: '{"sql":"DELETE"}'}, {options: '{"action":"retry"}'}, {options: '[]'}]) {
    assert.throws(() => validateDispatch({...request, ...change}));
  }
});

test('related application and cleanup owner changes fence DML while docs remain independent', () => {
  for (const file of ['frontend-school/src/a.ts', 'frontend-admin/src/a.ts', 'backend-school/src/a.rs', 'backend-admin/src/a.rs', 'contracts/a.json', 'scripts/lib/owned-load-cleanup-proof.py', 'scripts/lib/owned-load-cleanup/catalogue.json', '.github/workflows/operations.yml']) {
    assert.ok(relatedFiles([file]), file);
    assert.throws(() => requireQuiescentBeforeDispatch([[file]], [], '1'));
  }
  requireQuiescentBeforeDispatch([['docs/TESTING.md', 'backend-school/tests/catalog.rs']], [], '1');
  for (const status of ['queued', 'in_progress', 'waiting', 'pending', 'requested']) {
    assert.throws(() => requireQuiescentBeforeDispatch([], [{id: 2, head_branch: 'main', path: '.github/workflows/pipeline.yml', status}], '1'));
  }
});

test('only the outer Operations caller owns cleanup concurrency', () => {
  const outer = read('.github/workflows/operations.yml');
  const child = read('.github/workflows/owned-load-cleanup.yml');
  const owner = outer.slice(outer.indexOf('  owned-load-cleanup:'), outer.indexOf('  api-dns:'));
  assert.match(owner, /group: schoolorbit-production/);
  assert.match(owner, /cancel-in-progress: false/);
  assert.doesNotMatch(child, /concurrency:/);
  assert.match(child, /CLEANUP_PHASE: commit/);
  assert.ok(child.indexOf('Recheck current main and change quiescence') < child.indexOf('Dispatch once, persist actual exit'));
  assert.doesNotMatch(child, /assert hashlib/);
});

test('both cleanup jobs retain the ancestry required by the trusted Git gate', () => {
  const workflow = read('.github/workflows/owned-load-cleanup.yml');
  for (const job of ['plan', 'cleanup']) {
    const start = workflow.indexOf(`  ${job}:\n`);
    assert.ok(start >= 0, `${job} job missing`);
    const section = workflow.slice(start).split(/\n(?= {2}\S)/, 1)[0];
    assert.match(section, /uses: actions\/checkout@v6\n        with:\n          fetch-depth: 0/,
      `${job} requires complete Git history`);
  }
});

test('local deployment verification owns the pure proof and controller Python cases', () => {
  for (const file of ['owned-load-cleanup-proof.test.py', 'owned-load-cleanup-controller.test.py', 'owned-load-cleanup-transport.test.py', 'owned-load-cleanup-bootstrap.test.py']) {
    execFileSync('python3', ['-B', path.join(repo, 'scripts/tests', file)], {stdio: 'pipe'});
  }
});


test('inert staging verifies exact files without accepting executables or symlinks', t => {
  const f = fixture(t);
  const bundle = path.join(f.stack, 'bundle'); mkdirSync(bundle, {mode: 0o700});
  writeFileSync(path.join(bundle, 'scope.json'), '{}', {mode: 0o600});
  const expected = {files: {'scope.json': createHash('sha256').update('{}').digest('hex')}};
  verifyInputs(bundle, expected);
  writeFileSync(path.join(bundle, 'run.py'), 'wrong', {mode: 0o600});
  assert.throws(() => verifyInputs(bundle, expected));
  rmSync(path.join(bundle, 'run.py')); rmSync(path.join(bundle, 'scope.json'));
  symlinkSync('/etc/passwd', path.join(bundle, 'scope.json'));
  assert.throws(() => verifyInputs(bundle, expected));
});

test('a validated no-dispatch abort is distinct from successful cleanup', t => {
  const f = fixture(t); const run = '123-1';
  const staging = path.join(f.state, 'staging'); const origin = path.join(staging, run);
  mkdirSync(origin, {recursive: true, mode: 0o700});
  const aborted = path.join(f.state, 'aborted'); mkdirSync(aborted, {mode: 0o700});
  const reservation = {schemaVersion: 1, operationId: operation, inputSha256: catalogue.inputSha256,
    status: 'prepared', run, controller: {pid: 2147483647, startTicks: 1, bootId: readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim()},
    catalogueSha256: 'a'.repeat(64), controllerSha256: 'b'.repeat(64), proofSha256: 'c'.repeat(64)};
  const raw = JSON.stringify(reservation);
  writeFileSync(path.join(origin, 'aborted-reservation.json'), raw, {mode: 0o600});
  const receipt = {...reservation, status: 'aborted-before-dispatch', reservationSha256: createHash('sha256').update(raw).digest('hex'), noDispatchIntent: true};
  writeFileSync(path.join(aborted, `${run}.json`), JSON.stringify(receipt), {mode: 0o600});
  assert.equal(f.run().status, 0);
  assert.equal(readFileSync(path.join(aborted, `${run}.json`), 'utf8').includes('exitCode'), false);
  writeFileSync(path.join(origin, 'aborted-reservation.json'), '{}');
  assert.equal(f.run().status, 78);
});


test('manual Operations helpers select deployment verification without replacing application artifacts', () => {
  for (const file of ['scripts/lib/pipeline-remote/owned-load-cleanup.py', 'scripts/lib/pipeline-remote/api-dns-operation.sh', 'scripts/lib/pipeline-remote/api-dns-verification.sh']) {
    const plan = classify([file]);
    assert.deepEqual(plan.deploy, [], file);
    assert.ok(plan.verify.includes('deployment'), file);
  }
  for (const file of ['scripts/lib/pipeline-remote/school-deploy-backend.sh', 'scripts/lib/pipeline-remote/maintenance.sh']) {
    assert.ok(classify([file]).deploy.length > 0, file);
  }
});
