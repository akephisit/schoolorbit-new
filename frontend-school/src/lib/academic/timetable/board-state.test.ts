import assert from 'node:assert/strict';
import test from 'node:test';

import type {
	TimetableBlock,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace
} from '../../api/timetable';
import {
	blocksForTimetableCell,
	createTimetableBoardState,
	localPlacementPreview,
	remainingDemandForGroup,
	replaceTimetableBlocks,
	rowsForTimetableView,
	teacherPeriodCount
} from './board-state.ts';

function block(
	id: string,
	groupId: string,
	dayOfWeek: string,
	periodId: string,
	teacherIds: string[],
	homeroomIds: string[],
	roomId: string | null = null
): TimetableBlock {
	return {
		id,
		blockKind: 'course',
		dayOfWeek,
		bellSchedulePeriodId: periodId,
		groups: [
			{
				id: `${id}-group`,
				learningGroupId: groupId,
				learningOfferingId: `offering-${groupId}`,
				code: groupId,
				name: groupId,
				homeroomIds,
				instructors: teacherIds.map((teacherId, orderIndex) => ({
					teacherId,
					displayName: teacherId,
					role: orderIndex === 0 ? 'primary' : 'secondary',
					orderIndex
				})),
				roomId,
				roomCode: null,
				rowVersion: 1,
				isActive: true,
				syncStatus: null
			}
		],
		homerooms: [],
		teachers: [],
		rowVersion: 1
	} as unknown as TimetableBlock;
}

function workspace(status: 'draft' | 'published' = 'draft'): TimetableBlockWorkspace {
	const blocks = [
		block(
			'block-1',
			'group-1',
			'MON',
			'period-1',
			['teacher-1'],
			['homeroom-1', 'homeroom-2'],
			'room-1'
		),
		block('block-2', 'group-2', 'TUE', 'period-1', ['teacher-2'], ['homeroom-2'], 'room-2'),
		block('block-3', 'group-2', 'WED', 'period-2', ['teacher-1', 'teacher-2'], ['homeroom-2'])
	];
	return {
		version: { id: 'version-1', status } as TimetableBlockWorkspace['version'],
		bellPeriods: [
			{ id: 'period-1', orderIndex: 1 },
			{ id: 'period-2', orderIndex: 2 }
		] as TimetableBlockWorkspace['bellPeriods'],
		learningGroups: [
			{
				id: 'group-1',
				learningOfferingId: 'offering-1',
				code: 'ค21101-รวม',
				name: 'คณิตศาสตร์ ม.1 เรียนร่วม',
				status: 'published',
				rosterStatus: 'published',
				offeringCode: 'ค21101',
				offeringKind: 'course',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				homeroomIds: ['homeroom-1', 'homeroom-2'],
				eligibleInstructors: []
			},
			{
				id: 'group-2',
				learningOfferingId: 'offering-2',
				code: 'อ21101-ม.1/2',
				name: 'ภาษาอังกฤษ ม.1/2',
				status: 'published',
				rosterStatus: 'published',
				offeringCode: 'อ21101',
				offeringKind: 'course',
				offeringName: 'ภาษาอังกฤษ',
				homeroomIds: ['homeroom-2'],
				eligibleInstructors: []
			}
		],
		homerooms: [
			{ id: 'homeroom-1', code: 'M1-1', name: 'ม.1/1' },
			{ id: 'homeroom-2', code: 'M1-2', name: 'ม.1/2' }
		] as TimetableBlockWorkspace['homerooms'],
		rooms: [],
		staff: [
			{ id: 'teacher-1', displayName: 'ครูหนึ่ง', status: 'active' },
			{ id: 'teacher-2', displayName: 'ครูสอง', status: 'active' }
		],
		blocks,
		ordinaryDemands: [
			{
				learningGroupId: 'group-1',
				learningOfferingId: 'offering-1',
				offeringCode: 'ค21101',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				requiredPeriods: 3,
				scheduledPeriods: 1,
				remainingPeriods: 2,
				homeroomIds: ['homeroom-1', 'homeroom-2'],
				eligibleInstructors: []
			}
		],
		synchronizedDemands: [],
		summary: {} as TimetableBlockWorkspace['summary']
	};
}

test('one block projects into all covered homerooms and exact teacher rows', () => {
	const state = createTimetableBoardState(workspace());
	assert.deepEqual(
		rowsForTimetableView(state, 'homeroom').map((row) => row.id),
		['homeroom-1', 'homeroom-2']
	);
	assert.deepEqual(
		blocksForTimetableCell(state, {
			view: 'homeroom',
			rowId: 'homeroom-2',
			dayOfWeek: 'MON',
			bellSchedulePeriodId: 'period-1'
		}).map((item) => item.id),
		['block-1']
	);
	assert.equal(teacherPeriodCount(state, 'teacher-1'), 2);
	assert.equal(teacherPeriodCount(state, 'teacher-2'), 2);
	assert.equal(state.blocksById.size, 3);
});

test('local placement detects row, teacher, and room conflicts', () => {
	const state = createTimetableBoardState(workspace());
	const source: TimetableBlockPlacementSource = {
		kind: 'existing_block',
		blockId: 'block-2',
		rowVersion: 1
	};
	const candidate: TimetableBlockPlacementCandidate = {
		blockKind: 'course',
		learningGroupId: 'group-2',
		learningOfferingId: 'offering-2',
		instructorIds: ['teacher-1'],
		homeroomIds: ['homeroom-2'],
		teacherIds: [],
		roomId: 'room-1'
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
	assert.equal(preview.targetBlockId, 'block-1');
	assert.ok(preview.conflicts.includes('homeroom'));
	assert.ok(preview.conflicts.includes('teacher'));
	assert.ok(preview.conflicts.includes('room'));
});

test('demand accounting follows canonical block membership and published versions are read-only', () => {
	const state = createTimetableBoardState(workspace());
	assert.equal(remainingDemandForGroup(state, 'group-1'), 2);
	const removed = replaceTimetableBlocks(state, []);
	assert.equal(remainingDemandForGroup(removed, 'group-1'), 3);
	assert.equal(createTimetableBoardState(workspace('published')).canEdit, false);
});
