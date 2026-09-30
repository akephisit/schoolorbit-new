import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
for (const [route, operation] of [
	['work', 'getMyWorkItems'],
	['work/manage', 'listManageableWorkflowWindows']
]) {
	test(`${route} primary data is owned directly by the route loader`, async () => {
		const loader = await source(`routes/(app)/staff/${route}/+page.ts`);
		const page = await source(`routes/(app)/staff/${route}/+page.svelte`);
		assert.match(loader, new RegExp(operation));
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /captureRouteLoad/);
		assert.doesNotMatch(page, /onMount/);
		assert.match(page, /\$effect\.pre/);
		assert.match(page, /LatestRequest/);
	});
}
test('global work state owns counts and change signals, never hidden item reads', async () => {
	const store = await source('lib/stores/work.ts');
	assert.doesNotMatch(store, /getMyWorkItems|fetchItems|loadingItems/);
	assert.match(store, /revision/);
	assert.match(store, /getMyWorkCounts/);
});
test('work assignment references are bounded and interaction-lazy', async () => {
	const page = await source('routes/(app)/staff/work/manage/+page.svelte');
	assert.match(page, /showAssignment/);
	assert.match(page, /showCreate/);
	assert.match(page, /staffRequest/);
	assert.match(page, /unitsRequest/);
	assert.doesNotMatch(page, /async function loadData/);
});
