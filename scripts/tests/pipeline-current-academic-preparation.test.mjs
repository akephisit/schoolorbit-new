import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { releaseBundleFiles } from '../lib/pipeline-release-bundle.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const read = (name) => readFileSync(new URL(`../../${name}`, import.meta.url), 'utf8');

test('fixed preparation controller has portable pure failure coverage without provider or crypto tooling', () => {
  const result = spawnSync('python3', ['scripts/tests/current-academic-preparation.test.py', 'PreparationTests'], {
    cwd: root,
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
    encoding: 'utf8', timeout: 60_000, maxBuffer: 500_000,
  });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
});

test('release image contains the exact existing canonical copy CLI and checksum-pinned age tools', () => {
  const docker = read('backend-school/Dockerfile');
  const build = read('backend-school/scripts/build_release.sh');
  assert.match(docker, /ADD --checksum=sha256:7df45a6cc87d4da11cc03a539a7470c15b1041ab2b396af088fe9990f7c79d50 https:\/\/github\.com\/FiloSottile\/age\/releases\/download\/v1\.2\.1\/age-v1\.2\.1-linux-amd64\.tar\.gz/);
  assert.match(docker, /COPY .*target\/release\/migrate_tenant_schema \/app\/migrate_tenant_schema/);
  assert.match(docker, /COPY --from=academic-age-tool .*age-keygen/);
  assert.match(build, /cargo build --release --locked --bin migrate_tenant_schema/);
  assert.match(build, /cargo rustc --release --locked --bin backend-school --timings --message-format=json-render-diagnostics/);
});

test('immutable bundle and existing headless cutover own preparation after writer removal and before live migration', () => {
  assert.ok(releaseBundleFiles(root).includes('scripts/lib/pipeline-remote/current-academic-preparation.py'));
  const rollout = read('scripts/lib/pipeline-remote/school-storage-rollout.sh');
  const stop = rollout.indexOf('podman rm "$backend"');
  const prepare = rollout.indexOf('current-academic-preparation.py" prepare');
  const live = rollout.indexOf('configured "$scratch/migration" schoolorbit-web migrate-tenants');
  const complete = rollout.indexOf('current-academic-preparation.py" complete');
  assert.ok(stop >= 0 && stop < prepare && prepare < live && live < complete);
  for (const code of ['ACADEMIC_CURRENT_PREPARATION_REQUIRED', 'ACADEMIC_CURRENT_PREPARATION_MISMATCH']) assert.ok(rollout.includes(code));
});

test('release only forwards exact bounded controller and canonical refusal schemas', () => {
  const source = read('scripts/lib/pipeline-remote/school-storage-rollout.sh');
  const helper = source.slice(source.indexOf('report_preparation_refusal() {'), source.indexOf('private_file() {'));
  const execute = (body) => {
    const directory = mkdtempSync(path.join(os.tmpdir(), 'academic-refusal-'));
    const input = path.join(directory, 'input.json');
    try {
      writeFileSync(input, typeof body === 'string' ? body : JSON.stringify(body), { mode: 0o600 });
      return spawnSync('bash', ['-c', `${helper}\nreport_preparation_refusal "$1"`, 'fixture', input], { encoding: 'utf8', timeout: 10_000 });
    } finally { rmSync(directory, { recursive: true, force: true }); }
  };
  const diagnostic = { schemaVersion: 1, operation: 'migrate_tenant_schema', status: 'refused', version: 157, sqlstate: '23514', code: 'ACADEMIC_CURRENT_PREPARATION_MISMATCH' };
  assert.deepEqual(JSON.parse(execute({ schemaVersion: 1, error: 'cutover_date_changed' }).stdout), { schemaVersion: 1, error: 'cutover_date_changed' });
  assert.deepEqual(JSON.parse(execute({ schemaVersion: 1, error: 'copy_migration_refused', migrationRefusal: diagnostic }).stdout), { schemaVersion: 1, error: 'copy_migration_refused', migrationRefusal: { version: 157, sqlstate: '23514', code: diagnostic.code } });
  for (const value of [
    'raw PostgreSQL error with private row sentinel',
    { schemaVersion: 1, error: 'postgresql://private-sentinel' },
    { schemaVersion: 1, error: 'child_failed', rows: 'private-sentinel' },
    { schemaVersion: 1, error: 'copy_migration_refused', migrationRefusal: { ...diagnostic, version: 151 } },
    { schemaVersion: 1, error: 'copy_migration_refused', migrationRefusal: { ...diagnostic, payload: 'private-sentinel' } },
    { schemaVersion: 1, error: 'copy_migration_refused', migrationRefusal: { ...diagnostic, code: 'unknown-private-sentinel' } },
  ]) {
    const result = execute(value);
    assert.equal(result.stdout, '');
    assert.equal(result.stderr, '');
  }
});
