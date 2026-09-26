import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const source = (relativePath) => readFile(path.join(root, relativePath), 'utf8');

test('exam round and staff schedule lists start their primary read in a route loader', async () => {
	for (const [route, operation] of [
		['staff/academic/exam-schedules', 'listExamRounds'],
		['staff/exams', 'listStaffExamSchedules']
	]) {
		const directory = `src/routes/(app)/${route}`;
		const [loader, page] = await Promise.all([
			source(`${directory}/+page.ts`),
			source(`${directory}/+page.svelte`)
		]);
		assert.match(loader, new RegExp(operation), route);
		assert.match(loader, /requestFetch:\s*fetch/, route);
		assert.match(page, /data\.rounds/, route);
		assert.doesNotMatch(page, /\bonMount\s*\(/, route);
	}
});

test('exam list wrappers use route fetch and large or unbounded lists do not hover preload', async () => {
	const [api, sidebar] = await Promise.all([
		source('src/lib/api/examSchedule.ts'),
		source('src/lib/components/layout/Sidebar.svelte')
	]);
	assert.match(api, /listExamRounds[\s\S]{0,100}ApiRequestOptions/);
	assert.match(api, /listStaffExamSchedules[\s\S]{0,100}ApiRequestOptions/);
	assert.match(
		sidebar,
		/if \(path === '\/staff\/academic\/exam-schedules' \|\| path === '\/staff\/exams'\) return 'off'/
	);
	assert.match(sidebar, /data-sveltekit-preload-data=\{menuPreloadPolicy\(item\)\}/);
});
