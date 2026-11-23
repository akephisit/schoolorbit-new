import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
for (const section of ['', 'requests', 'evaluate']) {
	test(`supervision ${section || 'mine'} owns observations in the route`, async () => {
		const base = `src/routes/(app)/staff/academic/supervision${section ? `/${section}` : ''}`;
		const [loader, page] = await Promise.all([
			source(`${base}/+page.ts`),
			source(`${base}/+page.svelte`)
		]);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /listSupervisionObservations/);
		assert.match(loader, /SUPERVISION_OBSERVATION_READ_PERMISSIONS/);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(page, /routeData=\{data\}/);
	});
}

test('the result approval queue is retired and scoped Overview owns cycle progress', async () => {
	const base = 'src/routes/(app)/staff/academic/supervision';
	await assert.rejects(source(`${base}/approvals/+page.ts`), { code: 'ENOENT' });
	await assert.rejects(source(`${base}/approvals/+page.svelte`), { code: 'ENOENT' });

	const overview = await source(`${base}/overview/+page.ts`);
	assert.match(overview, /getSupervisionCycleProgress\(id, \{ requestFetch: fetch \}\)/);
	assert.match(overview, /waitForSupervisionAccess\(SUPERVISION_SCHOOL_REPORT_PERMISSIONS\)/);
});

test('supervision workspace has no component-owned startup and keeps selected workflows lazy', async () => {
	const page = await source('src/lib/components/supervision/SupervisionWorkspace.svelte');
	assert.doesNotMatch(page, /\bonMount\(/);
	assert.doesNotMatch(page, /listSupervisionTemplates/);
	assert.match(page, /observationsLoaded/);
	assert.match(page, /retryObservations/);
	assert.match(page, /bookingOpen/);
	assert.match(page, /evaluationTemplateRequest/);
	assert.match(page, /canReadSchoolReport/);
});
