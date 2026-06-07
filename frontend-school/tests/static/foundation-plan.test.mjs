import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('school operations foundation plan uses canonical organization unit terminology', async () => {
	const source = await readRepoFile('docs/plans/SCHOOL_OPERATIONS_FOUNDATION_PLAN.md');

	assert.match(source, /\borganization_units\b/);
	assert.match(source, /\borganization_members\b/);
	assert.match(source, /\borganization_permission_grants\b/);
	assert.doesNotMatch(
		source,
		/\bdepartments\b|\bdepartment_members\b|\bdepartment_permissions\b|\bdepartments\.subject_group_id\b/
	);
});
