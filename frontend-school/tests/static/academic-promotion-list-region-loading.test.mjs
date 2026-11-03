import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
for (const name of ['promotion', 'promotion/policies'])
	test(`${name} retires commands into one canonical route`, async () => {
		const route = await read(`src/routes/(app)/staff/academic/${name}/+page.ts`);
		assert.match(route, /redirect\(307/);
		assert.match(route, /year-lifecycle/);
		assert.doesNotMatch(route, /menu:|listPromotionPolicies|createPromotionRun/);
	});
