import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
test('canonical year workspace owns independent initial reads and keeps action data lazy', async () => {
	const route = await read('src/routes/(app)/staff/academic/year-lifecycle/+page.ts');
	for (const operation of [
		'getYearLifecycleWorkspace',
		'getAcademicSetupWorkspace',
		'listPromotionRuns'
	])
		assert.match(route, new RegExp(operation));
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(
		route,
		/getAcademicRetentionWorkspace|previewYearRollover|getYearRolloverOptions/
	);
	const page = await read('src/routes/(app)/staff/academic/year-lifecycle/+page.svelte');
	assert.match(page, /data\.workspace/);
	assert.match(page, /data\.setup/);
	assert.doesNotMatch(page, /academicContext\.subscribe\(/);
});
test('term entrypoint redirects to the canonical workspace without primary legacy reads', async () => {
	const route = await read('src/routes/(app)/staff/academic/term-lifecycle/+page.ts');
	assert.match(route, /redirect\(307/);
	assert.match(route, /academicYearId/);
	assert.doesNotMatch(route, /getTermLifecycleWorkspace|menu:/);
});
