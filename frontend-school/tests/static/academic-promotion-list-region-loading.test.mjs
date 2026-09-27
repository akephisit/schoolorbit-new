import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

test('promotion runs list belongs to the year route and keeps policy options lazy', async () => {
	const [route, page] = await Promise.all([
		read('src/routes/(app)/staff/academic/promotion/+page.ts'),
		read('src/routes/(app)/staff/academic/promotion/+page.svelte')
	]);
	assert.match(route, /listPromotionRuns/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(route, /academicYearId/);
	assert.doesNotMatch(route, /listPromotionPolicies/);
	assert.match(page, /data\.runs/);
	assert.match(page, /listPromotionPolicies/);
	assert.doesNotMatch(page, /context\.subscribe\(refresh\)/);
});

test('promotion policy list belongs to the route and keeps references lazy', async () => {
	const [route, page] = await Promise.all([
		read('src/routes/(app)/staff/academic/promotion/policies/+page.ts'),
		read('src/routes/(app)/staff/academic/promotion/policies/+page.svelte')
	]);
	assert.match(route, /listPromotionPolicies/);
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /getPromotionPolicyOptions/);
	assert.match(page, /data\.policies/);
	assert.match(page, /getPromotionPolicyOptions/);
	assert.doesNotMatch(page, /can\.subscribe\(/);
});
