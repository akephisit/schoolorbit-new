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

test('organization permission matrix keeps header and row checkboxes aligned', async () => {
	const source = await readProjectFile(
		'src/lib/components/staff/OrganizationPermissionDialog.svelte'
	);

	assert.match(source, /table-fixed/);
	assert.match(source, /<colgroup>/);
	assert.match(source, /<col class="w-\[360px\]"/);
	assert.match(source, /<col class="w-\[96px\]"/);
	assert.match(source, /flex justify-center/);
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

test('organization delegation tab follows backend school-wide authorization policy', async () => {
	const source = await readProjectFile('src/routes/(app)/staff/organization/[id]/+page.svelte');
	const registry = await readProjectFile('src/lib/permissions/registry.ts');

	assert.match(registry, /ROLES_UPDATE_ALL:\s*['"]roles\.update\.all['"]/);
	assert.match(source, /canManageDelegations/);
	assert.match(source, /PERMISSIONS\.ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT/);
	assert.match(source, /PERMISSIONS\.ROLES_ASSIGN_ALL/);
	assert.match(source, /PERMISSIONS\.ROLES_UPDATE_ALL/);
	assert.match(source, /if\s*\(\s*canManageDelegations\s*\)/);
	assert.match(source, /if\s*\(\s*!loading\s*&&\s*canManageDelegations\s*&&\s*deptId\s*\)/);
});

test('organization overview uses focused navigation with selected-unit details', async () => {
	const source = await readProjectFile('src/routes/(app)/staff/organization/+page.svelte');

	assert.match(source, /organizationStats/);
	assert.match(source, /unitTypeFilters/);
	assert.match(source, /activeUnitTypeFilter/);
	assert.match(source, /selectedUnit/);
	assert.match(source, /selectedMembers/);
	assert.match(source, /visibleTreeRoots/);
	assert.match(source, /handleSelectUnit/);
	assert.match(source, /เปิดรายละเอียด/);
	assert.doesNotMatch(source, /grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6/);
});
