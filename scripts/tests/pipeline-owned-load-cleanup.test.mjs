import test from 'node:test';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync, spawnSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, symlinkSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import path from 'node:path';

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
  const common = {inputSha256, actualBindingSha256: binding};
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
