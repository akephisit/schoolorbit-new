import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
for (const route of ['staff/roles', 'staff/roles/[id]']) {
	test(`${route} owns primary role reads through route fetch`, async () => {
		const loader = await read(`src/routes/(app)/${route}/+page.ts`);
		const page = await read(`src/routes/(app)/${route}/+page.svelte`);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /ROLES_READ_ALL/);
		assert.doesNotMatch(page, /onMount\(/);
		assert.match(page, /\$effect.pre/);
		assert.match(page, /LatestRequest/);
	});
}
test('selected role and catalog have separate state and retries', async () => {
	const page = await read('src/routes/(app)/staff/roles/[id]/+page.svelte');
	assert.match(page, /data-testid="role-detail"/);
	assert.match(page, /data-testid="role-permissions"/);
	assert.match(page, /catalogError/);
	assert.match(page, /roleEpoch/);
});

test('identity-sensitive migrated reads participate in the shared identity refresh', async () => {
	for (const route of [
		'staff',
		'staff/profile',
		'staff/view/[id]',
		'staff/roles',
		'staff/roles/[id]'
	]) {
		const loader = await read(`src/routes/(app)/${route}/+page.ts`);
		assert.match(loader, /depends\('school:app-identity'\)/, route);
	}
});
