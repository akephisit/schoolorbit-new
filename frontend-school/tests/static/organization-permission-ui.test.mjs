import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('organization permission dialog preserves position-scoped grants', async () => {
	const source = await readProjectFile(
		'src/lib/components/staff/OrganizationPermissionDialog.svelte'
	);

	assert.match(source, /permissionPositionColumns/);
	assert.match(source, /selectedGrantKeys/);
	assert.match(source, /grantKey\(/);
	assert.match(source, /position_code:\s*parseGrantKey/);
	assert.doesNotMatch(source, /if\s*\(\s*!grant\.position_code\s*\)\s*selectedPermissionIds\.add/);
	assert.doesNotMatch(source, /Array\.from\(selectedPermissionIds\)/);

	for (const position of [
		'all',
		'director',
		'deputy_director',
		'head',
		'deputy_head',
		'coordinator',
		'member'
	]) {
		assert.match(source, new RegExp(`value:\\s*['"]${position}['"]`));
	}
});

test('organization members section exposes full school position set', async () => {
	const source = await readProjectFile(
		'src/lib/components/staff/OrganizationMembersSection.svelte'
	);

	for (const position of [
		'director',
		'deputy_director',
		'head',
		'deputy_head',
		'coordinator',
		'member'
	]) {
		assert.match(source, new RegExp(`value:\\s*['"]${position}['"]`));
	}

	assert.match(source, /groupedMembers/);
	assert.match(source, /activeMemberCount/);
});
