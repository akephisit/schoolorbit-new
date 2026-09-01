import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

test('timetable workspace uses one cancellable set-based board load', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /getTimetableBlockWorkspace/);
	assert.equal((page.match(/getTimetableBlockWorkspace\(/g) ?? []).length, 3);
	assert.match(page, /listTimetableVersions/);
	assert.match(page, /LatestRequest/);
	assert.match(page, /isAbortError/);
	assert.match(page, /request\.begin\(\)/);
	assert.match(page, /request\.abort\(\)/);
	assert.doesNotMatch(page, /loadTimetableCollections/);
	assert.doesNotMatch(page, /listLearningGroupsForTerm|listLearningOfferings|listHomerooms/);
	assert.doesNotMatch(page, /listTimetableEntries|listBellSchedulePeriods|lookupRooms/);
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

test('timetable derives setup notices from its set-based workspace', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	assert.match(page, /AcademicPrerequisiteNotice/);
	assert.match(page, /controller\.workspace\.learningGroups/);
	assert.match(page, /controller\.workspace\.bellPeriods/);
	assert.match(page, /controller\.workspace\.rooms/);
	assert.match(page, /missingGroupsPrerequisite/);
	assert.match(page, /missingTeachersPrerequisite/);
	assert.match(page, /missingPeriodsPrerequisite/);
	assert.match(page, /missingRoomsPrerequisite/);
	assert.match(page, /\/staff\/academic\/core/);
	assert.match(page, /\/staff\/facility\/buildings/);
	assert.doesNotMatch(page, /getLearningDeliveryManagementOptions|getCurriculumProgramWorkspace/);
});

test('retired timetable activity context utility is removed after the hard cutover', async () => {
	await assert.rejects(
		readFile(path.join(projectRoot, 'src/lib/utils/timetable-activity-context.ts'), 'utf8'),
		(error) => error?.code === 'ENOENT'
	);
});
