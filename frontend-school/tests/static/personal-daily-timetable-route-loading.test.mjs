import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');

function source(relativePath) {
	return readFile(path.join(root, relativePath), 'utf8');
}

test('daily and personal timetable primary reads begin in their route loaders', async () => {
	for (const [route, api] of [
		['staff/academic/timetable/today', 'getDailyTeachingOverview'],
		['staff/timetable', 'getMyTimetable']
	]) {
		const directory = `src/routes/(app)/${route}`;
		const [loader, page] = await Promise.all([
			source(`${directory}/+page.ts`),
			source(`${directory}/+page.svelte`)
		]);
		assert.match(loader, new RegExp(api), route);
		assert.match(loader, /requestFetch:\s*fetch/, route);
		assert.match(page, /data\.(?:overview|blocks)/, route);
		assert.doesNotMatch(page, /\bonMount\s*\(/, route);
	}
});

test('daily overview wrapper accepts the route fetch and personal PDF stays action-lazy', async () => {
	const [api, personal] = await Promise.all([
		source('src/lib/api/timetable.ts'),
		source('src/routes/(app)/staff/timetable/+page.svelte')
	]);
	assert.match(api, /getDailyTeachingOverview[\s\S]{0,350}ApiRequestOptions/);
	assert.match(personal, /await import\('\$lib\/utils\/pdf'\)/);
	assert.doesNotMatch(personal, /import \{ generateTimetablePDF \} from/);
});

test('the volatile school-wide daily overview preloads on tap in both sidebar modes', async () => {
	const sidebar = await source('src/lib/components/layout/Sidebar.svelte');
	assert.match(sidebar, /path === '\/staff\/academic\/timetable\/today'/);
	assert.match(sidebar, /onpointerdown=\{\(\) => preloadMenuItem\(item, 'tap'\)\}/);
	assert.match(sidebar, /data-sveltekit-preload-data=\{menuPreloadPolicy\(item\)\}/);
});
