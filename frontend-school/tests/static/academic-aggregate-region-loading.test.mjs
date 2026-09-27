import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const read = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');

test('term aggregate route starts roster and policy regions independently', async () => {
	const [route, page] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/aggregates/+page.ts'),
		read('src/routes/(app)/staff/academic/results/aggregates/+page.svelte')
	]);
	assert.match(route, /listAggregateStudents/);
	assert.match(route, /listAggregatePolicies/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(page, /data\.students/);
	assert.match(page, /data\.policies/);
	assert.doesNotMatch(page, /onMount\(\(\) => \{[\s\S]*context\.subscribe/);
});

test('annual route starts the year roster and selected-only detail in its loader', async () => {
	const [route, page] = await Promise.all([
		read('src/routes/(app)/staff/academic/results/annual/+page.ts'),
		read('src/routes/(app)/staff/academic/results/annual/+page.svelte')
	]);
	assert.match(route, /listAnnualResultStudents/);
	assert.match(route, /listAnnualResultRevisions/);
	assert.match(route, /previewAnnualResult/);
	assert.match(route, /requestFetch: fetch/);
	assert.match(route, /studentAcademicYearId/);
	assert.match(page, /data\.students/);
	assert.match(page, /data\.preview/);
	assert.match(page, /data\.history/);
	assert.doesNotMatch(page, /onMount\(\(\) => \{[\s\S]*context\.subscribe/);
});
