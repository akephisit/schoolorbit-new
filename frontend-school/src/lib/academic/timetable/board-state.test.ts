import assert from 'node:assert/strict';
import test from 'node:test';

import type {
	TimetablePlacementCandidate,
	TimetablePlacementSource,
	TimetableWorkspace
} from '../../api/timetable';
import {
	createTimetableBoardState,
	entriesForTimetableCell,
	localPlacementPreview,
	remainingDemandForGroup,
	replaceTimetableEntries,
	rowsForTimetableView
} from './board-state.ts';

const entry = (
	id: string,
	learningGroupId: string,
	dayOfWeek: string,
	bellSchedulePeriodId: string,
	instructorIds: string[],
	roomId: string | null = null
) =>
	({
		id,
		learningGroupId,
		dayOfWeek,
		bellSchedulePeriodId,
		roomId,
		instructors: instructorIds.map((userId, index) => ({
			userId,
			displayName: userId,
			role: index === 0 ? 'primary' : 'secondary'
		})),
		entryType: 'COURSE',
		rowVersion: 1
	}) as unknown as TimetableWorkspace['entries'][number];

function workspace(status: 'draft' | 'published' = 'draft'): TimetableWorkspace {
	return {
		version: { id: 'version-1', status } as TimetableWorkspace['version'],
		bellPeriods: [
			{ id: 'period-1', orderIndex: 1 },
			{ id: 'period-2', orderIndex: 2 }
		] as TimetableWorkspace['bellPeriods'],
		learningGroups: [
			{
				id: 'group-1',
				learningOfferingId: 'offering-1',
				code: 'คม101-ม.1/รวม',
				name: 'คณิตศาสตร์ ม.1 เรียนร่วม',
				status: 'published',
				rosterStatus: 'published',
				offeringCode: 'ค21101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				homeroomIds: ['homeroom-1', 'homeroom-2'],
				eligibleInstructorIds: ['teacher-1']
			},
			{
				id: 'group-2',
				learningOfferingId: 'offering-2',
				code: 'อ21101-ม.1/2',
				name: 'ภาษาอังกฤษ ม.1/2',
				status: 'published',
				rosterStatus: 'published',
				offeringCode: 'อ21101',
				offeringName: 'ภาษาอังกฤษ',
				homeroomIds: ['homeroom-2'],
				eligibleInstructorIds: ['teacher-1', 'teacher-2']
			}
		],
		homerooms: [
			{
				id: 'homeroom-1',
				code: 'M1-1',
				name: 'ม.1/1',
				gradeLevelId: 'grade-1',
				gradeLevelType: 'secondary',
				gradeLevelYear: 1,
				roomNumber: '1',
				isActive: true
			},
			{
				id: 'homeroom-2',
				code: 'M1-2',
				name: 'ม.1/2',
				gradeLevelId: 'grade-1',
				gradeLevelType: 'secondary',
				gradeLevelYear: 1,
				roomNumber: '2',
				isActive: true
			}
		],
		rooms: [],
		staff: [],
		entries: [
			entry('entry-1', 'group-1', 'MON', 'period-1', ['teacher-1'], 'room-1'),
			entry('entry-2', 'group-2', 'TUE', 'period-1', ['teacher-2'], 'room-2')
		],
		unscheduledDemands: [
			{
				learningGroupId: 'group-1',
				learningOfferingId: 'offering-1',
				offeringCode: 'ค21101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				requiredPeriods: 3,
				scheduledPeriods: 1,
				remainingPeriods: 2,
				homeroomIds: ['homeroom-1', 'homeroom-2'],
				eligibleInstructorIds: ['teacher-1']
			}
		]
	};
}

test('shared learning-group entries project into every covered homeroom without duplication', () => {
	const state = createTimetableBoardState(workspace());
	assert.deepEqual(
		rowsForTimetableView(state, 'homeroom').map((row) => row.id),
		['homeroom-1', 'homeroom-2']
	);
	assert.deepEqual(
		entriesForTimetableCell(state, {
			view: 'homeroom',
			rowId: 'homeroom-1',
			dayOfWeek: 'MON',
			bellSchedulePeriodId: 'period-1'
		}).map((item) => item.id),
		['entry-1']
	);
	assert.deepEqual(
		entriesForTimetableCell(state, {
			view: 'homeroom',
			rowId: 'homeroom-2',
			dayOfWeek: 'MON',
			bellSchedulePeriodId: 'period-1'
		}).map((item) => item.id),
		['entry-1']
	);
	assert.equal(state.entriesById.size, 2);
});

test('local placement uses exact group, homeroom, teacher, and room occupancy', () => {
	const state = createTimetableBoardState(workspace());
	const source: TimetablePlacementSource = {
		kind: 'existing_entry',
		entryId: 'entry-2',
		rowVersion: 1
	};
	const candidate: TimetablePlacementCandidate = {
		entryType: 'COURSE',
		learningGroupId: 'group-2',
		learningOfferingId: 'offering-2',
		instructorIds: ['teacher-1'],
		roomId: 'room-2'
	};
	const preview = localPlacementPreview(state, {
		source,
		candidate,
		view: 'homeroom',
		rowId: 'homeroom-2',
		dayOfWeek: 'MON',
		bellSchedulePeriodId: 'period-1'
	});
	assert.equal(preview.state, 'swap');
	assert.equal(preview.targetEntryId, 'entry-1');
	assert.ok(preview.conflicts.includes('homeroom'));
	assert.ok(preview.conflicts.includes('instructor'));

	const trayPreview = localPlacementPreview(state, {
		source: {
			kind: 'unscheduled_demand',
			learningGroupId: 'group-2',
			learningOfferingId: 'offering-2'
		},
		candidate,
		view: 'homeroom',
		rowId: 'homeroom-2',
		dayOfWeek: 'MON',
		bellSchedulePeriodId: 'period-1'
	});
	assert.equal(trayPreview.state, 'blocked');
});

test('remaining demand follows the normalized entries and published boards are read-only', () => {
	const state = createTimetableBoardState(workspace());
	assert.equal(state.canEdit, true);
	assert.equal(remainingDemandForGroup(state, 'group-1'), 2);

	const added = replaceTimetableEntries(state, [
		...state.entries,
		entry('entry-3', 'group-1', 'WED', 'period-2', ['teacher-1'])
	]);
	assert.equal(remainingDemandForGroup(added, 'group-1'), 1);

	const removed = replaceTimetableEntries(state, []);
	assert.equal(remainingDemandForGroup(removed, 'group-1'), 3);
	assert.equal(createTimetableBoardState(workspace('published')).canEdit, false);
});
