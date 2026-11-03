import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
test('promotion history loads the preserved workspace without action-only references or mutation consumers', async () => {
	const route = await read('src/routes/(app)/staff/academic/promotion/[id]/+page.ts');
	assert.match(route, /getPromotionRun/);
	assert.match(route, /requestFetch: fetch/);
	assert.doesNotMatch(route, /getPromotionPolicyOptions|calculatePromotionRun|executePromotionRun/);
	const detail = await read('src/lib/components/academic/lifecycle/PromotionRunDetail.svelte');
	assert.match(detail, /workspaceLoad/);
	assert.match(detail, /student\.receipt/);
	assert.doesNotMatch(
		detail,
		/approvePromotionRun|reviewPromotionItem|resolvePromotionRunImpact|annualResultCurrent/
	);
	const api = await read('src/lib/api/academic-promotion.ts');
	assert.doesNotMatch(api, /apiClient\.(post|put|patch|delete)/);
});
