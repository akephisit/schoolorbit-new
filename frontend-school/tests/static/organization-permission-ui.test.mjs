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
	assert.match(source, /if\s*\(\s*!loading\s*&&\s*canManageDelegations\s*&&\s*currentDeptId\s*\)/);
	assert.match(source, /loadDelegations\(currentDeptId\)/);
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

test('organization detail uses focused unit workspace layout', async () => {
	const source = await readProjectFile('src/routes/(app)/staff/organization/[id]/+page.svelte');

	assert.match(source, /detailStats/);
	assert.match(source, /primaryMembers/);
	assert.match(source, /contactItems/);
	assert.match(source, /contextPanel/);
	assert.match(source, /canUpdateOrganizationUnit/);
	assert.match(source, /PERMISSIONS\.ROLES_UPDATE_ALL/);
	assert.match(source, /แก้ไขหน่วยงาน/);
	assert.match(source, /งานหลักของหน่วยงาน/);
	assert.match(source, /activeTab = \$state<DetailTab>\('members'\)/);
	assert.doesNotMatch(source, /activeTab = \$state<DetailTab>\('overview'\)/);
});

test('organization detail reloads when the route id changes without remounting', async () => {
	const source = await readProjectFile('src/routes/(app)/staff/organization/[id]/+page.svelte');

	assert.doesNotMatch(source, /import\s+\{\s*onMount\s*\}\s+from\s+['"]svelte['"]/);
	assert.match(source, /\$effect\(\(\)\s*=>\s*\{\s*const currentDeptId = deptId;/);
	assert.match(source, /loadData\(currentDeptId\)/);
	assert.match(source, /async function loadData\(currentDeptId: string\)/);
	assert.match(source, /if\s*\(\s*currentDeptId !== deptId\s*\)\s*return/);
	assert.match(source, /listOrganizationMembers\(currentDeptId\)/);
	assert.match(source, /parent_unit_id === currentDeptId/);
});

test('organization member and delegation dialogs use shadcn-svelte primitives', async () => {
	const members = await readProjectFile(
		'src/lib/components/staff/OrganizationMembersSection.svelte'
	);
	const detail = await readProjectFile('src/routes/(app)/staff/organization/[id]/+page.svelte');

	for (const source of [members, detail]) {
		assert.doesNotMatch(source, /fixed inset-0/);
		assert.doesNotMatch(source, /<select\b/);
		assert.doesNotMatch(source, /<input\b/);
		assert.match(source, /from '\$lib\/components\/ui\/dialog'/);
		assert.match(source, /from '\$lib\/components\/ui\/input'/);
		assert.match(source, /from '\$lib\/components\/ui\/label'/);
		assert.match(source, /from '\$lib\/components\/ui\/select'/);
		assert.match(source, /<Dialog\.Root/);
		assert.match(source, /<Dialog\.Content/);
		assert.match(source, /<Dialog\.Footer/);
		assert.match(source, /<Select\.Root/);
		assert.match(source, /<Select\.Trigger/);
		assert.match(source, /<Select\.Item/);
	}

	assert.match(members, /from '\$lib\/components\/ui\/checkbox'/);
	assert.match(members, /<Checkbox/);
});
