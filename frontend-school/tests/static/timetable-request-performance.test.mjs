import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('timetable workspace uses one cancellable term collection load', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /loadTimetableCollections/);
	assert.match(page, /listLearningGroupsForTerm/);
	assert.match(page, /LatestRequest/);
	assert.match(page, /isAbortError/);
	assert.match(page, /request\.begin\(\)/);
	assert.match(page, /request\.isCurrent\(revision\)/);
	assert.match(page, /request\.abort\(\)/);
	assert.doesNotMatch(page, /listLearningGroups\(offering\.id\)/);
	assert.doesNotMatch(page, /getActivitySlotTimetableContext/);
	assert.doesNotMatch(page, /listSlotInstructors|listSlotClassroomAssignments/);
});

test('academic delivery workspace uses one bounded overview request', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/delivery/+page.svelte'),
		'utf8'
	);

	assert.match(page, /getLearningDeliveryOverview/);
	assert.doesNotMatch(page, /listLearningGroupsForTerm/);
	assert.doesNotMatch(page, /listLearningOfferings/);
	assert.doesNotMatch(page, /listLearningGroups\(offering\.id\)/);
});

test('timetable derives separate setup notices from its existing term collections', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /AcademicPrerequisiteNotice/);
	assert.match(page, /missingGroupsPrerequisite/);
	assert.match(page, /missingTeachersPrerequisite/);
	assert.match(page, /missingPeriodsPrerequisite/);
	assert.match(page, /missingRoomsPrerequisite/);
	assert.match(page, /\/staff\/academic\/core#bell-schedules/);
	assert.match(page, /\/staff\/facility\/buildings/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions|getCurriculumProgramWorkspace/);
});

test('retired timetable activity context utility is removed after the hard cutover', async () => {
	await assert.rejects(
		readFile(path.join(projectRoot, 'src/lib/utils/timetable-activity-context.ts'), 'utf8'),
		(error) => error?.code === 'ENOENT'
	);
});
