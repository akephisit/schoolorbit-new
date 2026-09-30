import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (file) => readFile(new URL(`../../src/${file}`, import.meta.url), 'utf8');
for (const route of ['student', 'student/profile']) {
	test(`${route}: scoped context and primary profile are route owned`, async () => {
		const loader = await read(`routes/(app)/${route}/+page.ts`);
		const page = await read(`routes/(app)/${route}/+page.svelte`);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /listMyAcademicContextOptions/);
		assert.match(loader, /getOwnProfile/);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /ownerKey !==/);
		assert.match(page, /\$effect\.pre/);
		assert.match(page, /LatestRequest/);
		assert.match(page, /replaceState/);
		assert.doesNotMatch(page, /onMount|invalidateAll/);
	});
}

test('self profile navigation preloads only on tap', async () => {
	const { menuPreloadPolicy } = await import('../../src/lib/navigation/menu-preload.ts');
	assert.equal(menuPreloadPolicy({ path: '/student' }), 'tap');
	assert.equal(menuPreloadPolicy({ path: '/student/profile' }), 'tap');
});
