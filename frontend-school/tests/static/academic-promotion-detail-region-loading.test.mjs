import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

test('promotion detail starts workspace and display references in its route', async () => {
	const [route, page, detail] = await Promise.all([
		read('src/routes/(app)/staff/academic/promotion/[id]/+page.ts'),
		read('src/routes/(app)/staff/academic/promotion/[id]/+page.svelte'),
		read('src/lib/components/academic/lifecycle/PromotionRunDetail.svelte')
	]);
	assert.match(route, /getPromotionRun/);
	assert.match(route, /getPromotionPolicyOptions/);
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /lookupHomerooms|calculatePromotionRun|executePromotionRun/);
	assert.match(page, /data\.workspace/);
	assert.match(page, /data\.options/);
	assert.doesNotMatch(detail, /can\.subscribe\(/);
});
