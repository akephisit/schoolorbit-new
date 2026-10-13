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
  for (const directory of ['backend-admin/src', 'frontend-admin/src', 'backend-school/crates/school-tenancy/src']) {
    for (const file of sources(directory)) {
      assert.doesNotMatch(read(file), /NEON_(?:API_KEY|PROJECT_ID|BRANCH_ID)|NeonClient|api\.neon\.tech|neon\.tech\//, file);
    }
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
