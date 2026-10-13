import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const read = file => readFileSync(path.join(root, file), 'utf8');
const sources = directory => readdirSync(path.join(root, directory), { withFileTypes: true })
  .flatMap(entry => entry.isDirectory()
    ? sources(`${directory}/${entry.name}`)
    : [`${directory}/${entry.name}`]);

test('application runtime has one PostgreSQL provider and no Neon fallback', () => {
  for (const directory of ['backend-admin/src', 'backend-school/src', 'backend-school/crates', 'frontend-admin/src', 'frontend-school/src']) {
    for (const file of sources(directory)) {
      assert.doesNotMatch(read(file), /NEON_[A-Z_]+|NeonClient|neon\.tech|@neondatabase\//, file);
    }
  }
  for (const directory of ['.github/workflows', 'scripts/lib']) {
    for (const file of sources(directory)) {
      assert.doesNotMatch(read(file), /NEON_[A-Z_]+|NeonClient|neon\.tech|postgres_data_transfer|postgres_cutover_gate/, file);
    }
  }
  for (const file of ['backend-admin/Cargo.toml', 'backend-admin/Cargo.lock', 'backend-school/Cargo.toml', 'backend-school/Cargo.lock', 'frontend-admin/package.json', 'frontend-admin/package-lock.json', 'frontend-school/package.json', 'frontend-school/package-lock.json']) {
    assert.doesNotMatch(read(file), /@neondatabase\/|neon\.tech/, file);
  }
  for (const file of ['.env.example', 'backend-admin/.env.example', 'backend-school/.env.example', 'podman-compose.yml', 'compose.local.yml']) {
    assert.doesNotMatch(read(file), /NEON_|neon\.tech|SCHOOL_ROLLBACK_JWT_SECRET/, file);
  }
  assert.doesNotMatch(read('backend-school/crates/school-test-db/src/lib.rs'), /direct_test_database_url|neon\.tech|-pooler\./);
  assert.doesNotMatch(read('backend-admin/src/main.rs'), /--migrate-admin-expand|--provision-database/);
  assert.doesNotMatch(read('backend-admin/src/db/pool.rs'), /expand_only/);
  assert.doesNotMatch(read('backend-school/crates/school-tenancy/src/pool_manager.rs'), /get_pool_with_permission_change/);
  assert.doesNotMatch(read('.github/workflows/operations.yml'), /database-cutover/);
  for (const file of [
    'scripts/postgres_data_transfer.py', 'scripts/postgres_cutover_gate.mjs',
    'scripts/lib/pipeline-remote/postgres-cutover.sh',
    'scripts/lib/schoolorbit-installer/remote/provision_school_session_runtime.sh',
  ]) assert.equal(existsSync(path.join(root, file)), false, file);
});
