import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { backendBuildCache, SCHOOL_BUILD_CACHE_PACKAGE } from '../lib/pipeline-build-cache.mjs';

const repo = path.resolve(import.meta.dirname, '../..');
test('School main reads and writes its dedicated OCI registry cache', () => {
  assert.deepEqual(backendBuildCache({ component: 'backend-school', owner: 'Akephisit', ref: 'refs/heads/main' }), {
    from: `type=registry,ref=ghcr.io/akephisit/${SCHOOL_BUILD_CACHE_PACKAGE}:buildcache`,
    to: `type=registry,ref=ghcr.io/akephisit/${SCHOOL_BUILD_CACHE_PACKAGE}:buildcache,mode=max,image-manifest=true,oci-mediatypes=true`
  });
});
for (const ref of ['refs/heads/feature', 'refs/tags/old-release', 'refs/pull/1/merge']) {
  test(`School ${ref} cannot overwrite the main registry cache`, () => {
    const cache = backendBuildCache({ component: 'backend-school', owner: 'akephisit', ref });
    assert.match(cache.from, /^type=registry,ref=ghcr\.io\/akephisit\//);
    assert.equal(cache.to, '');
  });
}
test('Admin keeps its existing GHA cache policy on main and other refs', () => {
  for (const ref of ['refs/heads/main', 'refs/heads/feature']) assert.deepEqual(backendBuildCache({ component: 'backend-admin', owner: 'akephisit', ref }), { from: 'type=gha,scope=backend-admin', to: 'type=gha,scope=backend-admin,mode=max' });
});
test('cache owner and ref validation rejects malformed selector inputs', () => {
  const valid = { component: 'backend-school', owner: 'akephisit', ref: 'refs/heads/main' };
  for (const component of ['frontend-school', '', undefined]) assert.throws(() => backendBuildCache({ ...valid, component }), /Unknown backend/);
  for (const owner of ['owner/other', 'owner,mode=max', '', undefined]) assert.throws(() => backendBuildCache({ ...valid, owner }), /Invalid registry/);
  for (const ref of ['', 'refs/heads/main\nfrom=injected', undefined]) assert.throws(() => backendBuildCache({ ...valid, ref }), /Invalid build ref/);
});
test('CLI exports only cache configuration through the GitHub output file', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'build-cache-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const output = path.join(root, 'github-output');
  const result = spawnSync(process.execPath, [path.join(repo, 'scripts/lib/pipeline-build-cache.mjs')], {
    encoding: 'utf8', env: { ...process.env, COMPONENT: 'backend-school', GITHUB_REPOSITORY_OWNER: 'akephisit', GITHUB_REF: 'refs/heads/main', GITHUB_OUTPUT: output, GITHUB_TOKEN: 'private-cache-token-sentinel' }
  });
  assert.equal(result.status, 0, result.stderr);
  const cache = backendBuildCache({ component: 'backend-school', owner: 'akephisit', ref: 'refs/heads/main' });
  assert.equal(readFileSync(output, 'utf8'), `from=${cache.from}\nto=${cache.to}\n`);
  assert.doesNotMatch(result.stdout + result.stderr + readFileSync(output, 'utf8'), /private-cache-token-sentinel/);
});
test('preparation sends the selected policy to the image build and evidence export', () => {
  const prepare = readFileSync(path.join(repo, '.github/workflows/prepare.yml'), 'utf8');
  const action = readFileSync(path.join(repo, '.github/actions/build-backend-image/action.yml'), 'utf8');
  const maintenance = readFileSync(path.join(repo, '.github/workflows/maintenance.yml'), 'utf8');
  assert.match(prepare, /id: backend-cache[\s\S]*run: node scripts\/lib\/pipeline-build-cache\.mjs/);
  assert.match(prepare, /cache_from: \$\{\{ steps\.backend-cache\.outputs\.from \}\}/);
  assert.match(prepare, /cache_to: \$\{\{ steps\.backend-cache\.outputs\.to \}\}/);
  assert.match(prepare, /cache-from: \$\{\{ steps\.backend-cache\.outputs\.from \}\}/);
  assert.match(action, /cache-from: \$\{\{ inputs\.cache_from \}\}/);
  assert.match(action, /cache-to: \$\{\{ inputs\.cache_to \}\}/);
  assert.match(action, /SCHOOL_COMPILER_CACHE=gha/);
  assert.ok(maintenance.includes(`package: ${SCHOOL_BUILD_CACHE_PACKAGE}`));
});
