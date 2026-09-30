import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = (path) => readFile(new URL(`../../${path}`, import.meta.url), 'utf8');
test('selected supervision detail owns only identity and minimal dependent references', async () => {
	const loader = await source('src/routes/(app)/staff/academic/supervision/[id]/+page.ts');
	assert.match(loader, /getSupervisionObservation/);
	assert.match(loader, /getSupervisionCycle/);
	assert.match(loader, /getSupervisionTemplateSummary/);
	assert.match(loader, /SUPERVISION_OBSERVATION_READ_PERMISSIONS/);
	assert.match(loader, /requestFetch: fetch/);
	assert.doesNotMatch(
		loader,
		/getSupervisionObservationReview|listSupervisionCycles|getSupervisionTemplate\(/
	);
});
test('detail regions supersede stale reads and keep review and editors interaction-lazy', async () => {
	const page = await source('src/routes/(app)/staff/academic/supervision/[id]/+page.svelte');
	assert.doesNotMatch(page, /\bonMount\(/);
	assert.doesNotMatch(page, /listSupervisionCycles|getSupervisionTemplate\(/);
	assert.match(page, /\$effect.pre/);
	assert.match(page, /observationRequest.abort\(\)/);
	assert.match(page, /reviewOpen/);
	assert.match(page, /editTimetableError/);
	assert.match(page, /evaluatorAvailabilityError/);
	assert.match(page, /contextEpoch/);
});
