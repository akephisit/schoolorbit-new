import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { classify, components, git, inputHash, makePlan, suites } from '../lib/pipeline-policy.mjs';
import { trustedRun, validateState } from '../lib/pipeline-state.mjs';

test('UI, service, docs, tests and control changes have separate verification/deployment scopes', () => {
  assert.deepEqual(classify(['frontend-school/src/routes/+page.svelte']).verify, ['documentation', 'frontend-school']);
  assert.deepEqual(classify(['backend-school/crates/school-navigation/src/http/handlers/admin.rs']).deploy, ['backend-school']);
  assert.deepEqual(classify(['backend-admin/src/main.rs']).deploy, ['backend-admin']);
  assert.ok(classify(['backend-school/crates/school-auth-http/src/origin.rs']).verify.includes('frontend-school'));
  assert.deepEqual(classify(['backend-school/crates/school-auth-http/src/origin.rs']).deploy, ['backend-school']);
  assert.deepEqual(classify(['frontend-admin/tests/login.test.ts']).deploy, []);
  assert.deepEqual(classify(['README.md']).deploy, []);
  assert.ok(classify(['scripts/test_backend_admin.sh']).verify.includes('backend-admin'));
  assert.ok(classify(['scripts/test_school_database_suite.sh']).verify.includes('backend-school'));
  assert.deepEqual(classify(['.github/workflows/pipeline.yml']).deploy, []);
  assert.deepEqual(classify(['.github/workflows/pipeline.yml']).verify, suites);
  assert.deepEqual(classify(['contracts/openapi/school.json']).deploy, ['backend-school', 'frontend-school']);
  assert.deepEqual(classify(['new-unmapped-owner/input.json']).verify, suites);
  assert.throws(() => classify(['../outside']), /Invalid/);
});

function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'pipeline-policy-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync('git', ['init', '-q', root]);
  execFileSync('git', ['-C', root, 'config', 'user.name', 'Fixture']);
  execFileSync('git', ['-C', root, 'config', 'user.email', 'fixture@example.invalid']);
  for (const part of components) { mkdirSync(path.join(root, part)); writeFileSync(path.join(root, part, 'input.txt'), part); }
  const commit = () => { git(root, 'add', '.'); git(root, 'commit', '-qm', 'Fixture input'); return git(root, 'rev-parse', 'HEAD'); };
  const base = commit();
  const state = { components: Object.fromEntries(components.map((part) => [part, { sha: base, inputHash: inputHash(root, base, part) }])) };
  return { root, base, state, commit };
}

test('deployment tooling changes reconcile both frontend owners while retaining existing compiled artifacts', (t) => {
  for (const file of ['scripts/deployment_tooling.mjs', '.github/actions/setup-deployment-tooling/action.yml']) {
    const f = fixture(t);
    mkdirSync(path.dirname(path.join(f.root, file)), { recursive: true });
    writeFileSync(path.join(f.root, file), '// changed deployment tooling');
    const head = f.commit();
    const plan = makePlan({ ...f, head });
    assert.deepEqual(plan.deploy, ['frontend-admin', 'frontend-school']);
    assert.deepEqual(plan.verify, ['documentation', 'deployment', 'frontend-admin', 'frontend-school']);
    for (const component of components) assert.equal(inputHash(f.root, head, component), inputHash(f.root, f.base, component));
    f.state.components['frontend-admin'] = { sha: head, inputHash: inputHash(f.root, head, 'frontend-admin') };
    assert.deepEqual(makePlan({ ...f, head }).deploy, ['frontend-school']);
  }
});

test('accepted per-component baselines retain queued changes across merges and retries', (t) => {
  const f = fixture(t);
  writeFileSync(path.join(f.root, 'frontend-school/input.txt'), 'first frontend edit');
  const first = f.commit();
  let plan = makePlan({ ...f, head: first });
  assert.deepEqual(plan.deploy, ['frontend-school']);
  assert.deepEqual(plan.reasons['frontend-school'], ['unaccepted-runtime-paths', 'changed-artifact-inputs']);
  assert.deepEqual(plan.reasons['backend-school'], []);
  assert.ok(!plan.verify.includes('backend-school'));
  writeFileSync(path.join(f.root, 'backend-admin/input.txt'), 'next backend edit');
  const second = f.commit();
  plan = makePlan({ ...f, base: first, head: second });
  assert.deepEqual(plan.deploy, ['backend-admin', 'frontend-school']);
  f.state.components['frontend-school'] = { sha: first, inputHash: inputHash(f.root, first, 'frontend-school') };
  plan = makePlan({ ...f, base: first, head: second });
  assert.deepEqual(plan.deploy, ['backend-admin']);
  assert.deepEqual(makePlan({ ...f, head: second, scope: 'frontend-admin' }).deploy, ['backend-admin', 'frontend-admin']);
});

test('reporting and orchestration retain artifact hashes; real build recipes invalidate only their owners', t => {
  for (const [file, owners] of [
    ['.github/workflows/prepare.yml', []],
    ['scripts/lib/pipeline-build-cache.mjs', []],
    ['.github/workflows/pipeline.yml', []],
    ['.github/actions/build-backend-image/action.yml', ['backend-admin', 'backend-school']],
    ['.github/actions/build-frontend-bundle/action.yml', ['frontend-admin', 'frontend-school']]
  ]) {
    const f = fixture(t);
    mkdirSync(path.dirname(path.join(f.root, file)), { recursive: true });
    writeFileSync(path.join(f.root, file), 'changed recipe or report');
    const head = f.commit();
    const plan = makePlan({ ...f, head });
    assert.deepEqual(plan.deploy, owners, file);
    for (const part of components) {
      assert.equal(inputHash(f.root, head, part) !== f.state.components[part].inputHash, owners.includes(part), `${file}: ${part}`);
    }
  }
});

test('component release changes keep pending siblings and never invalidate their compiled artifacts', t => {
  for (const [file, owners] of [
    ['.github/workflows/deploy-backend-admin.yml', ['backend-admin']],
    ['.github/workflows/deploy-frontend-admin.yml', ['frontend-admin']],
    ['scripts/lib/pipeline-admin-worker.mjs', ['frontend-admin']],
    ['.github/workflows/deploy-school-release.yml', ['backend-school', 'frontend-school']],
    ['scripts/lib/pipeline-remote/school-deploy-backend.sh', ['backend-school']],
    ['scripts/lib/pipeline-remote/school-verify-release.sh', ['backend-school', 'frontend-school']],
    ['scripts/lib/pipeline-remote/admin-deploy.sh', ['backend-admin']]
  ]) {
    const f = fixture(t);
    mkdirSync(path.dirname(path.join(f.root, file)), { recursive: true });
    writeFileSync(path.join(f.root, file), 'changed deployment');
    const head = f.commit();
    const plan = makePlan({ ...f, head });
    assert.deepEqual(plan.deploy, owners, file);
    assert.deepEqual(plan.verify, suites, 'control changes retain broad local verification');
    for (const part of components) assert.equal(inputHash(f.root, head, part), f.state.components[part].inputHash);
    f.state.components[owners[0]].sha = head;
    assert.deepEqual(makePlan({ ...f, head }).deploy, owners.slice(1), 'only acceptance clears queued runtime work');
  }
  assert.deepEqual(classify(['scripts/lib/pipeline-remote/maintenance.sh']).deploy, components);
  assert.deepEqual(classify(['.github/workflows/release.yml']).deploy, components);
});

test("Worker bundling recipe changes rebuild both frontend artifacts without invalidating backend images", (t) => {
  const f = fixture(t);
  mkdirSync(path.join(f.root, "scripts"));
  writeFileSync(
    path.join(f.root, "scripts/prepare_frontend_worker.mjs"),
    "// changed bundling recipe",
  );
  const head = f.commit();
  for (const component of components) {
    const changed =
      inputHash(f.root, head, component) !==
      inputHash(f.root, f.base, component);
    assert.equal(changed, component.startsWith("frontend-"), component);
  }
  const plan = makePlan({ ...f, head });
  assert.deepEqual(plan.deploy, ["frontend-admin", "frontend-school"]);
  assert.deepEqual(plan.verify, [
    "documentation",
    "deployment",
    "frontend-admin",
    "frontend-school",
  ]);
});

test('an older unrelated component baseline cannot recompile an already accepted backend for a frontend edit', t => {
  const f = fixture(t);
  writeFileSync(path.join(f.root, 'backend-school/input.txt'), 'accepted backend change');
  const backend = f.commit();
  f.state.components['backend-school'] = {sha:backend,inputHash:inputHash(f.root,backend,'backend-school')};
  writeFileSync(path.join(f.root, 'frontend-admin/input.txt'), 'next frontend edit');
  const head = f.commit();
  const plan = makePlan({...f,base:backend,head});
  assert.deepEqual(plan.deploy, ['frontend-admin']);
  assert.deepEqual(plan.verify, ['documentation','frontend-admin']);
});

test('cold or divergent state reconciles every affected component; tests do not invalidate runtime artifacts', (t) => {
  const f = fixture(t);
  assert.deepEqual(makePlan({ ...f, head: f.base, state: null }).deploy, components);
  mkdirSync(path.join(f.root, 'backend-school/tests'));
  writeFileSync(path.join(f.root, 'backend-school/tests/check.rs'), '// fixture');
  const head = f.commit();
  assert.equal(inputHash(f.root, head, 'backend-school'), inputHash(f.root, f.base, 'backend-school'));
  assert.deepEqual(makePlan({ ...f, head }).deploy, []);
  const accepted = {components:Object.fromEntries(components.map(part=>[part,{sha:head,inputHash:inputHash(f.root,head,part)}]))};
  const manual = makePlan({ ...f, base: head, head, state: accepted, scope: 'frontend-admin' });
  assert.deepEqual(manual.deploy, ['frontend-admin']);
  assert.deepEqual(manual.verify, ['documentation', 'frontend-admin']);
  assert.notEqual(inputHash(f.root, head, 'frontend-school', { backend: 'https://different.invalid' }), inputHash(f.root, head, 'frontend-school'));
  assert.throws(() => makePlan({ ...f, head, scope: 'unsafe' }), /Invalid requested scope/);
});

test('accepted artifact provenance rejects forks, other workflows, failed runs and corrupt baselines', () => {
  const run = { id: 42, repository: { full_name: 'team/school' }, head_repository: { full_name: 'team/school' }, head_branch: 'main', event: 'push', path: '.github/workflows/pipeline.yml', conclusion: 'success', head_sha: 'a'.repeat(40) };
  assert.ok(trustedRun(run, 'team/school', 'pipeline.yml'));
  for (const changed of [{ event: 'pull_request' }, { conclusion: 'failure' }, { path: '.github/workflows/operations.yml' }, { head_repository: { full_name: 'outside/school' } }, { head_branch: 'feature' }]) assert.ok(!trustedRun({ ...run, ...changed }, 'team/school', 'pipeline.yml'));
  const state = { schemaVersion: 2, runId: '42', sha: run.head_sha, components: Object.fromEntries(components.map((part) => [part, { sha: run.head_sha, inputHash: 'b'.repeat(64), digest: 'sha256:' + 'c'.repeat(64), artifactRunId: 42, bundleDigest: 'c'.repeat(64), versionId: '12345678-abcd-1234-abcd-123456789abc', workers: {sandbox:'12345678-abcd-1234-abcd-123456789abc'} }])) };
  assert.equal(validateState(state, run), state);
  assert.throws(() => validateState({ ...state, runId: '43' }, run), /Invalid accepted/);
  state.components['backend-school'].digest = 'latest';
  assert.throws(() => validateState(state, run), /OCI digest/);
});
