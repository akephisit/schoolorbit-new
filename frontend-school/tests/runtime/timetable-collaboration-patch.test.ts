import assert from 'node:assert/strict';
import test from 'node:test';
import type { DraftPatch } from '../../src/lib/api/timetable.js';
import {
	applyDraftPatch,
	InvalidDraftLookupPatch
} from '../../src/lib/academic/timetable/collaboration-patch.ts';
import { collaborationWorkspace, emptyLookupChanges } from './timetable-collaboration-fixture.ts';
import { makeTimetableBlock, timetableIds } from '../e2e/timetable-test-harness.ts';

test('a partial patch changes only affected blocks/demands and preserves server totals beyond visible scope', () => {
	const workspace = collaborationWorkspace();
	workspace.blocks = [
		makeTimetableBlock(timetableIds.blockA, timetableIds.period1),
		makeTimetableBlock(timetableIds.blockB, timetableIds.period2)
	];
	const first = workspace.ordinaryDemands[0];
	const unchanged = workspace.ordinaryDemands[1];
	const moved = {
		...workspace.blocks[0],
		bellSchedulePeriodId: timetableIds.period3,
		rowVersion: 2
	};
	const patch: DraftPatch = {
		timetableVersionId: workspace.version.id,
		mutationId: 'remote-change',
		upsertedBlocks: [moved],
		removedBlockIds: [timetableIds.blockB],
		ordinaryDemands: [{ ...first, scheduledPeriods: 7, remainingPeriods: 3 }],
		synchronizedDemands: [],
		summary: { ...workspace.summary, blockCount: 17 },
		sourceIssues: [],
		totalDraftBlockCount: 17,
		version: { ...workspace.version, rowVersion: 4 }
	};
	const next = applyDraftPatch(workspace, patch);
	assert.deepEqual(next.blocks, [moved]);
	assert.equal(next.ordinaryDemands[0].scheduledPeriods, 7);
	assert.equal(next.ordinaryDemands[0].remainingPeriods, 3);
	assert.equal(next.ordinaryDemands[1], unchanged);
	assert.equal(next.summary.blockCount, 17);
	assert.equal(next.totalDraftBlockCount, 17);
	assert.equal(next.version.rowVersion, 4);
	assert.equal(workspace.blocks.length, 2);
	assert.equal(
		applyDraftPatch(workspace, { ...patch, timetableVersionId: 'another-draft' }),
		workspace
	);
});

test('lookup deltas preserve complete DTOs, remove lost scope and use exact producer ordering for every collection', () => {
	const workspace = collaborationWorkspace();
	const oldId = '91000000-0000-4000-8000-000000000999';
	const staffA = {
		id: timetableIds.teacherA,
		displayName: 'ครู เอ',
		status: 'active',
		subjectGroups: []
	};
	const staffB = {
		...staffA,
		id: timetableIds.teacherB,
		displayName: 'ครู บี',
		subjectGroups: [{ id: timetableIds.groupB, name: 'กลุ่มวิชาใหม่', displayOrder: null }]
	};
	const roomA = { id: timetableIds.room, code: 'Z', name: 'ห้องเดิม', status: 'ACTIVE' },
		roomB = { ...roomA, id: timetableIds.roomB, code: 'A', name: 'ห้องใหม่' };
	const periodA = {
		id: timetableIds.period1,
		bellScheduleId: timetableIds.schedule,
		name: 'คาบเดิม',
		orderIndex: 2,
		startTime: '09:00',
		endTime: '10:00',
		isActive: true
	};
	const periodB = { ...periodA, id: timetableIds.period2, name: 'คาบก่อนหน้า', orderIndex: 1 };
	const homeroomA = workspace.homerooms[0],
		homeroomB = {
			...homeroomA,
			id: '81000000-0000-4000-8000-000000000211',
			name: 'ป.2/1',
			gradeLevelType: 'primary',
			gradeLevelYear: 2
		};
	const subjectA = {
			learningOfferingId: timetableIds.offeringA,
			subjectGroup: { id: timetableIds.groupA, name: 'เดิม', displayOrder: null }
		},
		subjectB = {
			learningOfferingId: timetableIds.offeringB,
			subjectGroup: { id: timetableIds.groupB, name: 'ใหม่', displayOrder: null }
		};
	workspace.staff = [staffA, { ...staffA, id: oldId }];
	workspace.rooms = [roomA, { ...roomA, id: oldId }];
	workspace.bellPeriods = [periodA, { ...periodA, id: oldId }];
	workspace.homerooms = [homeroomA, { ...homeroomA, id: oldId }];
	workspace.offeringSubjectGroups = [subjectA, { ...subjectA, learningOfferingId: oldId }];
	const before = structuredClone(workspace);
	const patch: DraftPatch = {
		timetableVersionId: workspace.version.id,
		mutationId: timetableIds.blockA,
		upsertedBlocks: [],
		removedBlockIds: [],
		ordinaryDemands: [],
		synchronizedDemands: [],
		summary: { ...workspace.summary, blockCount: 99 },
		sourceIssues: [],
		lookupChanges: {
			upsertedStaff: [staffB],
			removedStaffIds: [oldId],
			staffOrder: [staffB.id, staffA.id],
			upsertedRooms: [roomB],
			removedRoomIds: [oldId],
			roomOrder: [roomB.id, roomA.id],
			upsertedBellPeriods: [periodB],
			removedBellPeriodIds: [oldId],
			bellPeriodOrder: [periodB.id, periodA.id],
			upsertedHomerooms: [homeroomB],
			removedHomeroomIds: [oldId],
			homeroomOrder: [homeroomB.id, homeroomA.id],
			upsertedOfferingSubjectGroups: [subjectB],
			removedSubjectGroupOfferingIds: [oldId],
			subjectGroupOfferingOrder: [subjectB.learningOfferingId, subjectA.learningOfferingId]
		}
	};
	const next = applyDraftPatch(workspace, patch);
	assert.deepEqual(next.staff, [staffB, staffA]);
	assert.deepEqual(next.homerooms, [homeroomB, homeroomA]);
	assert.deepEqual(next.rooms, [roomB, roomA]);
	assert.deepEqual(next.bellPeriods, [periodB, periodA]);
	assert.deepEqual(next.offeringSubjectGroups, [subjectB, subjectA]);
	assert.equal(next.summary.blockCount, 99);
	assert.deepEqual(workspace, before);
	const cleared = applyDraftPatch(next, {
		...patch,
		lookupChanges: {
			...emptyLookupChanges(),
			removedStaffIds: next.staff.map((row) => row.id),
			staffOrder: []
		}
	});
	assert.deepEqual(cleared.staff, []);
	assert.deepEqual(cleared.rooms, next.rooms);
});

test('incomplete, duplicate or unknown producer orders refuse the complete patch without fabricating rows', () => {
	const workspace = collaborationWorkspace();
	workspace.staff = [
		{ id: timetableIds.teacherA, displayName: 'ครูเดิม', status: 'active', subjectGroups: [] }
	];
	const base: DraftPatch = {
		timetableVersionId: workspace.version.id,
		mutationId: timetableIds.blockA,
		upsertedBlocks: [],
		removedBlockIds: [],
		ordinaryDemands: [],
		synchronizedDemands: [],
		summary: workspace.summary,
		sourceIssues: []
	};
	for (const order of [
		[],
		[timetableIds.teacherB],
		[timetableIds.teacherA, timetableIds.teacherA]
	]) {
		assert.throws(
			() =>
				applyDraftPatch(workspace, {
					...base,
					lookupChanges: { ...emptyLookupChanges(), staffOrder: order }
				}),
			InvalidDraftLookupPatch
		);
	}
	assert.throws(
		() =>
			applyDraftPatch(workspace, {
				...base,
				lookupChanges: {
					...emptyLookupChanges(),
					upsertedStaff: [{ ...workspace.staff[0], id: timetableIds.teacherB }]
				}
			}),
		InvalidDraftLookupPatch
	);
	assert.equal(workspace.staff.length, 1);
});
