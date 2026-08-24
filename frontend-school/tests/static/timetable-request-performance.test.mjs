import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('timetable workspace loads canonical term delivery context and rejects stale responses', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /listLearningOfferings\(termId\)/);
	assert.match(page, /listLearningGroups\(offering\.id\)/);
	assert.match(page, /listTimetableEntries\(\{ academicTermId: termId \}\)/);
	assert.match(page, /const current = \+\+revision/);
	assert.match(page, /if \(current !== revision\) return/);
	assert.doesNotMatch(page, /getActivitySlotTimetableContext/);
	assert.doesNotMatch(page, /listSlotInstructors|listSlotClassroomAssignments/);
});

test('retired timetable activity context utility is removed after the hard cutover', async () => {
	await assert.rejects(
		readFile(path.join(projectRoot, 'src/lib/utils/timetable-activity-context.ts'), 'utf8'),
		(error) => error?.code === 'ENOENT'
	);
});
