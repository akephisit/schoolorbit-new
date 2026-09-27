import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

for (const [routeName, apiName] of [
	['term-lifecycle', 'getTermLifecycleWorkspace'],
	['year-lifecycle', 'getYearLifecycleWorkspace']
]) {
	test(`${routeName} owns its visible workspace in the route loader`, async () => {
		const [route, page] = await Promise.all([
			read(`src/routes/(app)/staff/academic/${routeName}/+page.ts`),
			read(`src/routes/(app)/staff/academic/${routeName}/+page.svelte`)
		]);
		assert.match(route, new RegExp(apiName));
		assert.match(route, /requestFetch: fetch/);
		assert.match(page, /data\.workspace/);
		assert.doesNotMatch(page, /academicContext\.subscribe\(/);
	});
}
