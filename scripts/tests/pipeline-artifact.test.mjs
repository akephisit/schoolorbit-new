import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const script = path.resolve(import.meta.dirname, '../frontend_artifact.mjs');
test('frontend artifact checks configuration/source identity and real bundle bytes on cache reuse', (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'pipeline-bundle-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'frontend-school/build'), { recursive: true });
  writeFileSync(path.join(root, 'frontend-school/build/index.js'), 'export default {};');
  const env = { ...process.env, COMPONENT: 'frontend-school', INPUT_HASH: 'b'.repeat(64), GITHUB_SHA: 'a'.repeat(40), RUNNER_TEMP: root };
  const run = (command, extra = {}) => spawnSync('node', [script, command], { cwd: root, env: { ...env, ...extra }, encoding: 'utf8' });
  assert.equal(run('pack').status, 0);
  assert.equal(run('unpack').status, 0);
  assert.notEqual(run('unpack', { INPUT_HASH: 'c'.repeat(64) }).status, 0);
  assert.notEqual(run('unpack', { GITHUB_SHA: 'd'.repeat(40) }).status, 0);
  assert.equal(run('unpack', { GITHUB_SHA: 'd'.repeat(40), ARTIFACT_SHA: 'a'.repeat(40) }).status, 0);
  writeFileSync(path.join(root, 'frontend-school/build/index.js'), 'modified output');
  assert.notEqual(run('pack').status, 0);
  writeFileSync(path.join(root, 'frontend-school.tar'), 'corrupt archive');
  assert.notEqual(run('unpack').status, 0);
});

test('frontend unpack refuses symlinks even when the artifact digest is valid', (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'pipeline-unsafe-bundle-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'frontend-school/build'), { recursive: true });
  symlinkSync('/tmp/outside', path.join(root, 'frontend-school/build/outside'));
  const env = { ...process.env, COMPONENT: 'frontend-school', INPUT_HASH: 'b'.repeat(64), GITHUB_SHA: 'a'.repeat(40), RUNNER_TEMP: root };
  const run = command => spawnSync('node', [script, command], { cwd: root, env, encoding: 'utf8' });
  assert.equal(run('pack').status, 0);
  assert.notEqual(run('unpack').status, 0);
});
