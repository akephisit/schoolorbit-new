import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
for (const route of [
	'staff/manage',
	'staff/manage/[id]',
	'staff/manage/[id]/edit',
	'staff/manage/[id]/roles'
]) {
	test(`${route} primary reads have a route owner and identity dependency`, async () => {
		const loader = await read(`src/routes/(app)/${route}/+page.ts`);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /captureRouteLoad/);
	});
}
test('staff wizard reads only the opened workflow, and navigation never broadly invalidates', async () => {
	for (const route of ['staff/manage/new', 'staff/manage/[id]/edit']) {
		const page = await read(`src/routes/(app)/${route}/+page.svelte`);
		assert.doesNotMatch(page, /invalidateAll\s*:/);
		assert.doesNotMatch(page, /onMount\([\s\S]*?loadOptions\(/);
		assert.match(page, /LatestRequest/);
	}
});
