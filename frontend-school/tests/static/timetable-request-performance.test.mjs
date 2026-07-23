import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

import {
	activityInstructorEntries,
	independentItemsForInstructor,
	synchronizedSlotsForInstructor
} from '../../src/lib/utils/timetable-activity-context.ts';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

test('activity context helpers derive synchronized and independent instructor items', () => {
	const slots = [
		{ id: 'sync-a', scheduling_mode: 'synchronized' },
		{ id: 'sync-b', scheduling_mode: 'synchronized' },
		{ id: 'independent-a', scheduling_mode: 'independent' },
		{ id: 'independent-b', scheduling_mode: 'independent' }
	];
	const instructorsBySlot = {
		'sync-a': [{ id: 'membership-a', user_id: 'teacher-a' }],
		'sync-b': [{ id: 'membership-b', user_id: 'teacher-b' }],
		'independent-a': [],
		'independent-b': []
	};
	const assignmentsBySlot = {
		'sync-a': [],
		'sync-b': [],
		'independent-a': [
			{
				id: 'assignment-a',
				slot_id: 'independent-a',
				classroom_id: 'classroom-a',
				classroom_name: 'ป.1/1',
				instructor_id: 'teacher-a',
				instructor_name: 'ครูเอ'
			}
		],
		'independent-b': [
			{
				id: 'assignment-b',
				slot_id: 'independent-b',
				classroom_id: 'classroom-b',
				classroom_name: 'ป.1/2',
				instructor_id: 'teacher-b',
				instructor_name: 'ครูบี'
			}
		]
	};

	assert.deepEqual(
		synchronizedSlotsForInstructor(slots, instructorsBySlot, 'teacher-a').map((slot) => slot.id),
		['sync-a']
	);
	assert.deepEqual(
		independentItemsForInstructor(slots, assignmentsBySlot, 'teacher-a').map((item) => [
			item.slot.id,
			item.classroom_id,
			item.classroom_name
		]),
		[['independent-a', 'classroom-a', 'ป.1/1']]
	);
	assert.deepEqual(activityInstructorEntries(assignmentsBySlot), [
		['independent-a|classroom-a', { id: 'teacher-a', name: 'ครูเอ' }],
		['independent-b|classroom-b', { id: 'teacher-b', name: 'ครูบี' }]
	]);
});

test('timetable page batches activity context and cancels requests on destroy', async () => {
	const page = await readFile(
		path.join(projectRoot, 'src/routes/(app)/staff/academic/timetable/+page.svelte'),
		'utf8'
	);

	for (const required of [
		'getActivitySlotTimetableContext',
		'createRequestCoordinator',
		'isAbortError',
		'requestCoordinator.abortAll()'
	]) {
		assert.match(page, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
	}
	assert.doesNotMatch(page, /\blistSlotInstructors\b/);
	assert.doesNotMatch(page, /\blistSlotClassroomAssignments\b/);
});
