import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (file) => readFile(new URL(`../../${file}`, import.meta.url), 'utf8');
for (const route of ['staff/organization', 'staff/organization/[id]']) {
	test(`${route} owns primary reads in its loader`, async () => {
		const loader = await read(`src/routes/(app)/${route}/+page.ts`);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /captureRouteLoad/);
		const page = await read(`src/routes/(app)/${route}/+page.svelte`);
		assert.doesNotMatch(page, /onMount/);
	});
}
test('members consume the route region without another startup read', async () => {
	const component = await read('src/lib/components/staff/OrganizationMembersSection.svelte');
	assert.doesNotMatch(component, /listOrganizationMembers/);
	assert.match(component, /onChanged/);
});
test('delegation choices and permission labels stay opened and scoped', async () => {
	const detail = await read('src/routes/(app)/staff/organization/[id]/+page.svelte');
	assert.match(detail, /activeTab === 'delegations'/);
	assert.match(detail, /showDelegateDialog/);
	const dialog = await read('src/lib/components/staff/OrganizationPermissionDialog.svelte');
	assert.match(dialog, /SETTINGS_READ_ALL/);
	assert.match(dialog, /LatestRequest/);
});
