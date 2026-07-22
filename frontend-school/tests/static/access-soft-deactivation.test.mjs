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

test('generated access contracts expose reversible deactivation metadata', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');

	assert.equal(contract.paths?.['/api/roles/{id}']?.delete?.operationId, 'deleteRole');
	assert.equal(
		contract.paths?.['/api/organization/units/{id}']?.delete?.operationId,
		'deactivateOrganizationUnit'
	);
	for (const schemaName of ['Role', 'OrganizationUnit']) {
		const schema = contract.components.schemas[schemaName];
		assert.ok(schema.required.includes('is_system'), `${schemaName}.is_system must be required`);
		assert.equal(schema.properties.is_system.type, 'boolean');
	}

	assert.match(generated, /is_system:\s*boolean/);
	assert.match(generated, /deleteRole:/);
	assert.match(generated, /deactivateOrganizationUnit:/);
});

test('management API wrappers type inactive lists and deactivation envelopes', async () => {
	const rolesApi = await readRepoFile('frontend-school/src/lib/api/roles.ts');
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');

	assert.match(rolesApi, /type ManagedListOptions = \{ include_inactive\?: boolean \}/);
	assert.match(rolesApi, /listRoles\(options\?: ManagedListOptions\)/);
	assert.match(
		rolesApi,
		/if \(options\?\.include_inactive\) params\.set\('include_inactive', 'true'\)/
	);
	assert.match(rolesApi, /deleteRole[\s\S]*Promise<ApiResponse<EmptyData>>/);
	assert.match(rolesApi, /apiClient\.delete<EmptyData>/);

	assert.match(staffApi, /type ManagedListOptions = \{ include_inactive\?: boolean \}/);
	assert.match(staffApi, /listOrganizationUnits\(\s*options\?: ManagedListOptions\s*\)/);
	assert.match(
		staffApi,
		/if \(options\?\.include_inactive\) params\.set\('include_inactive', 'true'\)/
	);
	assert.match(staffApi, /deleteOrganizationUnit[\s\S]*Promise<ApiResponse<EmptyData>>/);
	assert.match(staffApi, /apiClient\.delete<EmptyData>/);

	for (const source of [rolesApi, staffApi]) {
		assert.match(source, /const qs = params\.toString\(\) \? `\?\$\{params\}` : ''/);
	}
});

test('staff assignment screens keep active-only list defaults', async () => {
	for (const relativePath of [
		'frontend-school/src/routes/(app)/staff/manage/new/+page.svelte',
		'frontend-school/src/routes/(app)/staff/manage/[id]/edit/+page.svelte',
		'frontend-school/src/lib/components/UserRoleManager.svelte'
	]) {
		const source = await readRepoFile(relativePath);
		assert.doesNotMatch(source, /include_inactive\s*:\s*true/);
	}
});

test('role management uses reversible permission-aware status controls', async () => {
	const listPage = await readRepoFile('frontend-school/src/routes/(app)/staff/roles/+page.svelte');
	const detailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/roles/[id]/+page.svelte'
	);

	assert.match(listPage, /roleAPI\.listRoles\(\{ include_inactive: true \}\)/);
	assert.match(listPage, /role\.is_system/);
	assert.match(listPage, /ระบบ/);

	assert.match(detailPage, /canDeleteRoles/);
	assert.match(detailPage, /role\.is_system/);
	assert.match(detailPage, /handleDeactivate/);
	assert.match(detailPage, /handleReactivate/);
	assert.match(detailPage, /roleAPI\.deleteRole\(roleId\)/);
	assert.match(detailPage, /roleAPI\.updateRole\(roleId, \{ is_active: true \}\)/);
	assert.match(detailPage, /ปิดใช้งาน/);
	assert.match(detailPage, /เปิดใช้งาน/);
	assert.match(detailPage, /สิทธิ์[\s\S]{0,100}ทันที/);
	assert.match(detailPage, /เปิดใช้งานกลับ/);
	assert.doesNotMatch(detailPage, /การกระทำนี้ไม่สามารถย้อนกลับได้/);
});
