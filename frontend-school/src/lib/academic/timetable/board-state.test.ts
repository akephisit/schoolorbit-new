import assert from 'node:assert/strict';
import test from 'node:test';

import type {
	TimetableBlock,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace
} from '../../api/timetable';
import { timetablePlacementReasons } from './placement-reasons.ts';
import {
	blockInstructorIds,
	blockTargetTeacherIds,
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
				homeroomNames: homeroomIds.map((id) => (id === 'homeroom-1' ? 'ม.1/1' : 'ม.1/2')),
				instructors: teacherIds.map((teacherId, orderIndex) => ({
					teacherId,
					displayName: teacherId,
					role: orderIndex === 0 ? 'primary' : 'secondary',
					orderIndex
				})),
				roomId,
				roomCode: null,
				roomName: null,
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
	const deliveryPublication: TimetableBlockWorkspace['deliveryPublication'] = {
		id: 'delivery-1',
		publicationNo: 1,
		effectiveFrom: '2026-05-01',
		effectiveUntil: null,
		publishedAt: '2026-05-01T00:00:00Z',
		isSuperseded: false
	};
	return {
		deliveryPublication,
		latestDeliveryPublication: deliveryPublication,
		publicationDateRules: { today: '2026-11-01', minimumEffectiveFrom: '2026-11-01' },
		latestDeliveryVersionId: 'delivery-1',
		sourceIssues: [],
		totalDraftBlockCount: null,
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
				subjectType: 'BASIC',
				offeringName: 'คณิตศาสตร์พื้นฐาน',
				homeroomIds: ['homeroom-1', 'homeroom-2'],
				preferredRoomIds: [],
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
				subjectType: 'BASIC',
				offeringName: 'ภาษาอังกฤษ',
				homeroomIds: ['homeroom-2'],
				preferredRoomIds: [],
				eligibleInstructors: []
			}
		],
		homerooms: [
			{ id: 'homeroom-1', code: 'M1-1', name: 'ม.1/1' },
			{ id: 'homeroom-2', code: 'M1-2', name: 'ม.1/2' }
		] as TimetableBlockWorkspace['homerooms'],
		rooms: [],
		staff: [
			{ id: 'teacher-1', displayName: 'ครูหนึ่ง', subjectGroups: [], status: 'active' },
			{ id: 'teacher-2', displayName: 'ครูสอง', subjectGroups: [], status: 'active' }
		],
		offeringSubjectGroups: [],
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

test('local swap removes source and target reservations and uses persisted source resources', () => {
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
	assert.deepEqual(preview.conflicts, []);
});

test('demand accounting follows canonical block membership and published versions are read-only', () => {
	const state = createTimetableBoardState(workspace());
	assert.equal(remainingDemandForGroup(state, 'group-1'), 2);
	const removed = replaceTimetableBlocks(state, []);
	assert.equal(remainingDemandForGroup(removed, 'group-1'), 3);
	assert.equal(createTimetableBoardState(workspace('published')).canEdit, false);
});

test('keeps confirmed group instructors separate from provisional teacher targets', () => {
	const synchronized = block(
		'block-sync',
		'group-sync',
		'WED',
		'period-1',
		['teacher-1'],
		['homeroom-1']
	);
	synchronized.blockKind = 'activity';
	synchronized.schedulingMode = 'synchronized';
	synchronized.teachers = [
		{ teacherId: 'teacher-1' },
		{ teacherId: 'teacher-2' }
	] as TimetableBlock['teachers'];

	assert.deepEqual(blockInstructorIds(synchronized), ['teacher-1']);
	assert.deepEqual(blockTargetTeacherIds(synchronized), ['teacher-1', 'teacher-2']);
});

function requestForGroup(): {
	source: TimetableBlockPlacementSource;
	candidate: TimetableBlockPlacementCandidate;
	view: 'homeroom';
	rowId: string;
	dayOfWeek: string;
	bellSchedulePeriodId: string;
} {
	return {
		source: {
			kind: 'ordinary_demand',
			learningGroupId: 'group-1',
			learningOfferingId: 'offering-1'
		},
		candidate: {
			blockKind: 'course',
			learningGroupId: 'group-1',
			learningOfferingId: 'offering-1',
			instructorIds: ['teacher-1', 'teacher-2', 'teacher-2'],
			teacherIds: [],
			roomId: 'room-1'
		},
		view: 'homeroom',
		rowId: 'homeroom-1',
		dayOfWeek: 'WED',
		bellSchedulePeriodId: 'period-2'
	};
}

test('local reasons retain actual block and every exact conflicting resource, including co-teachers', () => {
	const state = createTimetableBoardState(workspace());
	const preview = localPlacementPreview(state, requestForGroup());
	assert.equal(preview.state, 'blocked');
	assert.deepEqual(
		preview.conflicts.map((conflict) => [
			conflict.conflictType,
			conflict.targetId,
			conflict.existingBlockId
		]),
		[
			['homeroom', 'homeroom-2', 'block-3'],
			['teacher', 'teacher-1', 'block-3'],
			['teacher', 'teacher-2', 'block-3']
		]
	);
	const homeroom = timetablePlacementReasons(state, 'homeroom', preview.conflicts);
	assert.match(homeroom.primary, /ครูหนึ่ง ไม่ว่าง/);
	assert.match(homeroom.lesson ?? '', /group-2/);
	assert.match(homeroom.context ?? '', /ม.1\/2/);
	assert.equal(homeroom.details.length, 1);
	assert.match(homeroom.details[0], /ครูสอง/);
	assert.match(homeroom.details[0], /ม.1\/2 ไม่ว่าง/);
	assert.match(
		timetablePlacementReasons(state, 'teacher', preview.conflicts).primary,
		/ม.1\/2 ไม่ว่าง/
	);
	assert.match(
		timetablePlacementReasons(state, 'learning_group', preview.conflicts).primary,
		/ครูหนึ่ง ไม่ว่าง/
	);
	assert.deepEqual(
		timetablePlacementReasons(state, 'homeroom', [...preview.conflicts, ...preview.conflicts]),
		homeroom
	);
});

test('swap checks third-party blockers in both directions and never reports the dragged source', () => {
	const data = workspace();
	data.blocks.push(block('third-party', 'group-3', 'TUE', 'period-1', ['teacher-1'], [], 'room-9'));
	const state = createTimetableBoardState(data);
	const request = {
		...requestForGroup(),
		source: { kind: 'existing_block', blockId: 'block-2', rowVersion: 1 } as const,
		dayOfWeek: 'MON',
		bellSchedulePeriodId: 'period-1',
		rowId: 'homeroom-2'
	};
	const preview = localPlacementPreview(state, request);
	assert.equal(preview.state, 'blocked');
	assert.deepEqual(
		preview.conflicts.map((conflict) => conflict.existingBlockId),
		['third-party']
	);
	assert.equal(preview.conflicts[0].code, 'TIMETABLE_SWAP_RETURN_TEACHER_CONFLICT');
	assert.match(
		timetablePlacementReasons(state, 'homeroom', preview.conflicts).primary,
		/^คาบที่สลับกลับ: /
	);
	const sameSlot = localPlacementPreview(state, { ...request, dayOfWeek: 'TUE' });
	assert.equal(sameSlot.state, 'source');
	assert.deepEqual(sameSlot.conflicts, []);

	data.blocks.push(block('forward-blocker', 'group-3', 'MON', 'period-1', ['teacher-2'], [], null));
	const forward = localPlacementPreview(createTimetableBoardState(data), request);
	assert.equal(forward.state, 'blocked');
	assert.ok(forward.conflicts.some((conflict) => conflict.existingBlockId === 'forward-blocker'));
	assert.ok(
		forward.conflicts.every(
			(conflict) => !['block-1', 'block-2'].includes(conflict.existingBlockId ?? '')
		)
	);
});

test('detailed labels include activity, structural and room identities; hidden server references stay generic', () => {
	const data = workspace();
	data.blocks[0].blockKind = 'structural';
	data.blocks[0].title = 'ประชุมครู';
	data.blocks[0].offeringName = null;
	data.blocks[0].groups[0].roomName = 'ห้องประชุม';
	const state = createTimetableBoardState(data);
	const preview = localPlacementPreview(state, {
		...requestForGroup(),
		dayOfWeek: 'MON',
		bellSchedulePeriodId: 'period-1'
	});
	const roomConflict = preview.conflicts.filter((conflict) => conflict.conflictType === 'room');
	assert.match(
		timetablePlacementReasons(state, 'teacher', roomConflict).details[0],
		/ห้องสอน ห้องประชุม ไม่ว่าง · กิจกรรมรวม: ประชุมครู/
	);
	data.blocks[0].blockKind = 'activity';
	assert.match(
		timetablePlacementReasons(state, 'teacher', roomConflict).lesson ?? '',
		/กิจกรรม: ประชุมครู/
	);
	const hidden = {
		...roomConflict[0],
		existingBlockId: 'outside-scope',
		message: 'ห้องสอนถูกใช้อยู่แล้ว'
	};
	assert.deepEqual(timetablePlacementReasons(state, 'teacher', [hidden]).details, [
		'ห้องสอนถูกใช้อยู่แล้ว'
	]);
});
