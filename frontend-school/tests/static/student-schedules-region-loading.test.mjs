import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const read = (file) => readFile(new URL(`../../src/${file}`, import.meta.url), 'utf8');
for (const route of ['timetable', 'exams', 'activities']) {
	test(`student/${route}: route owns authorized prerequisite and primary`, async () => {
		const loader = await read(`routes/(app)/student/${route}/+page.ts`),
			page = await read(`routes/(app)/student/${route}/+page.svelte`);
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /listMyAcademicContextOptions/);
		assert.match(loader, /requestFetch: fetch/);
		assert.match(loader, /ownerKey !==/);
		assert.match(page, /\$effect\.pre/);
		assert.match(page, /LatestRequest/);
		assert.doesNotMatch(page, /onMount|loadHistory|invalidateAll/);
	});
}
test('date calendar loads authorized events without an academic prerequisite', async () => {
	const loader = await read('routes/(app)/student/calendar/+page.ts');
	assert.match(loader, /waitForAuthenticatedUser/);
	assert.match(loader, /listMyCalendarEvents/);
	assert.match(loader, /requestFetch: fetch/);
	assert.doesNotMatch(loader, /AcademicContextOptions|academicYearId/);
	const viewer = await read('lib/components/calendar/CalendarViewer.svelte');
	assert.match(viewer, /LatestRequest/);
	assert.match(viewer, /ownerKey !== k/);
});
test('month history only reloads events; registration is mutation-owned', async () => {
	const calendar = await read('lib/components/calendar/CalendarViewer.svelte');
	assert.match(calendar, /shallow:\s*true/);
	assert.match(calendar, /learnerCalendarUrl/);
	const activities = await read('routes/(app)/student/activities/+page.svelte');
	assert.match(activities, /applyRegistrationResult/);
	assert.match(activities, /ownerEpoch/);
});

test('scoped term selection rejects foreign terms and optional calendar keeps whole year', async () => {
	const { resolveScopedAcademicContextUrl } =
		await import('../../src/lib/academic-context/scoped-year.ts');
	const options = {
		years: [{ id: 'year-a' }, { id: 'year-b' }],
		terms: [
			{ id: 'term-a', academicYearId: 'year-a' },
			{ id: 'term-b', academicYearId: 'year-b' }
		],
		activeAcademicYearId: 'year-a',
		activeAcademicTermId: 'term-b'
	};
	const requested = new URL(
		'https://school.test/student/exams?academicYearId=year-a&academicTermId=term-b'
	);
	const required = resolveScopedAcademicContextUrl(options, requested, true);
	assert.equal(required.academicYearId, 'year-a');
	assert.equal(required.academicTermId, 'term-a');
	assert.equal(required.replaceUrl.searchParams.get('academicTermId'), 'term-a');
	const optional = resolveScopedAcademicContextUrl(options, requested, false);
	assert.equal(optional.academicTermId, '');
	assert.equal(optional.replaceUrl.searchParams.has('academicTermId'), false);
	assert.equal(
		resolveScopedAcademicContextUrl({ ...options, years: [], terms: [] }, requested, true)
			.academicYearId,
		''
	);
});
