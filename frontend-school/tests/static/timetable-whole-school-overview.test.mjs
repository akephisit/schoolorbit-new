import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('whole-school view loads one bounded day and remains outside editable board state', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const overview = await read(
		'src/lib/components/academic/timetable/TimetableWholeSchoolOverview.svelte'
	);

	assert.match(page, /getWholeSchoolTimetableOverview/);
	assert.match(page, /overviewCache/);
	assert.match(page, /timetableVersionId.*selectedOverviewDay/s);
	assert.match(page, /activeView === 'wholeSchool'/);
	assert.match(overview, /ภาพรวมทั้งโรงเรียน · ดูอย่างเดียว/);
	assert.doesNotMatch(
		overview,
		/createTimetableEntry|updateTimetableEntry|swapTimetableEntries|deleteTimetableEntry/
	);
	assert.doesNotMatch(overview, /draggable=|onDropIntent|onDragStart/);
});

test('whole-school issue recovery links preserve the exact version and owner projection', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');
	const summary = await read('src/lib/components/academic/timetable/TimetableIssueSummary.svelte');

	assert.match(page, /openOverviewHomeroom/);
	assert.match(page, /openOverviewTeacher/);
	assert.match(page, /focusPeriodId/);
	assert.match(summary, /onOpenHomeroom/);
	assert.match(summary, /onOpenTeacher/);
	assert.match(summary, /instructorIds/);
	assert.match(summary, /homeroomIds/);
});

test('whole-school matrix has sticky workbook headers and a compact mobile slice', async () => {
	const overview = await read(
		'src/lib/components/academic/timetable/TimetableWholeSchoolOverview.svelte'
	);

	assert.match(overview, /sticky top-0/);
	assert.match(overview, /sticky left-0/);
	assert.match(overview, /sm:hidden/);
	assert.match(overview, /selectedMobilePeriodId/);
	assert.match(overview, /selectedMobileHomeroomId/);
	assert.match(overview, /Select\.Root/);
});
