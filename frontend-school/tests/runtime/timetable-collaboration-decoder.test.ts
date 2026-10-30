import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import { collaborationWorkspace, emptyLookupChanges } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';
import { makeTimetableBlock } from '../e2e/timetable-test-harness.ts';

const epoch = 'f1000000-0000-4000-8000-000000000999';
const participant = {
	userId: timetableIds.user,
	name: 'ผู้ร่วมจัดตาราง',
	color: '#123456',
	profileImageFileId: null,
	connections: [
		{
			connectionId: timetableIds.teacherA,
			context: { viewMode: 'homeroom', viewId: timetableIds.homeroom },
			anchor: {
				kind: 'cell',
				dayOfWeek: 'MON',
				bellSchedulePeriodId: timetableIds.period1,
				u: 0.25,
				v: 0.75
			}
		}
	]
};

test('decoder accepts a typed authorized baseline and finite semantic presence', () => {
	const sync = {
		epoch,
		seq: 0,
		type: 'DraftStateSync',
		payload: {
			connectionId: timetableIds.teacherA,
			workspace: collaborationWorkspace(),
			users: [participant],
			reservations: []
		}
	};
	assert.deepEqual(decodeDraftEvent(JSON.stringify(sync)), sync);
	const presence = { epoch, seq: 0, type: 'DraftPresenceChanged', payload: participant };
	assert.deepEqual(decodeDraftEvent(JSON.stringify(presence)), presence);
});

test('a valid persisted note longer than sixteen thousand characters remains decodable', () => {
	const workspace = collaborationWorkspace();
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.note = 'หมายเหตุเดิม '.repeat(2000);
	assert.ok(block.note.length > 16_384);
	workspace.blocks = [block];
	const event = decodeDraftEvent(
		JSON.stringify({
			epoch,
			seq: 0,
			type: 'DraftStateSync',
			payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
		})
	);
	assert.equal(event?.type, 'DraftStateSync');
	if (event?.type === 'DraftStateSync')
		assert.equal(event.payload.workspace.blocks[0].note, block.note);
});

test('decoder rejects selector identities, nonfinite or outside anchors, unsafe colors and image URLs', () => {
	const event = (payload: unknown) =>
		JSON.stringify({ epoch, seq: 0, type: 'DraftPresenceChanged', payload });
	assert.equal(decodeDraftEvent(event({ ...participant, userId: '"] article' })), null);
	assert.equal(
		decodeDraftEvent(event({ ...participant, color: 'url(https://outside.invalid)' })),
		null
	);
	assert.equal(
		decodeDraftEvent(
			event({ ...participant, profileImageFileId: 'https://outside.invalid/photo' })
		),
		null
	);
	for (const u of [Infinity, -0.2, 1.2]) {
		assert.equal(
			decodeDraftEvent(
				event({
					...participant,
					connections: [
						{ ...participant.connections[0], anchor: { ...participant.connections[0].anchor, u } }
					]
				})
			),
			null
		);
	}
	assert.equal(decodeDraftEvent('{bad JSON'), null);
	assert.equal(
		decodeDraftEvent(
			JSON.stringify({
				epoch,
				seq: -1,
				type: 'DraftPresenceLeft',
				payload: { connectionId: timetableIds.user }
			})
		),
		null
	);
});

test('public reservation snapshots refuse accidentally disclosed private proof or candidate fields', () => {
	const reservation = {
		reservationId: timetableIds.blockA,
		ownerConnectionId: timetableIds.user,
		purpose: 'drag',
		source: { kind: 'existing_block', blockId: timetableIds.blockB, rowVersion: 1 },
		target: null,
		expiresAt: '2030-01-01T00:00:00Z'
	};
	const event = (value: unknown) =>
		JSON.stringify({
			epoch,
			seq: 0,
			type: 'DraftReservationsChanged',
			payload: { reservations: [value] }
		});
	assert.ok(decodeDraftEvent(event(reservation)));
	assert.equal(decodeDraftEvent(event({ ...reservation, leaseToken: timetableIds.groupA })), null);
	assert.equal(
		decodeDraftEvent(event({ ...reservation, candidate: { roomId: timetableIds.room } })),
		null
	);
});

test('the complete JSON event budget accepts exactly 64 MiB of UTF-8 and refuses one additional byte', () => {
	const limit = 64 * 1024 * 1024;
	const workspace = collaborationWorkspace();
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.note = '';
	workspace.blocks = [block];
	const sync = {
		epoch,
		seq: 0,
		type: 'DraftStateSync',
		payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
	};
	const overhead = Buffer.byteLength(JSON.stringify(sync), 'utf8');
	block.note = 'x'.repeat(limit - overhead);
	const exact = JSON.stringify(sync);
	assert.equal(Buffer.byteLength(exact, 'utf8'), limit);
	assert.equal(decodeDraftEvent(exact)?.type, 'DraftStateSync');
	assert.equal(decodeDraftEvent(exact + ' '), null);
});

test('a valid Thai snapshot above sixteen MiB uses UTF-8 bytes and keeps complete persisted notes', () => {
	const workspace = collaborationWorkspace();
	const block = makeTimetableBlock(timetableIds.blockA, timetableIds.period1);
	block.note = 'ก'.repeat(6 * 1024 * 1024);
	workspace.blocks = [block];
	const serialized = JSON.stringify({
		epoch,
		seq: 0,
		type: 'DraftStateSync',
		payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
	});
	assert.ok(serialized.length < 16 * 1024 * 1024);
	assert.ok(Buffer.byteLength(serialized, 'utf8') > 16 * 1024 * 1024);
	const event = decodeDraftEvent(serialized);
	assert.equal(event?.type, 'DraftStateSync');
	if (event?.type === 'DraftStateSync')
		assert.equal(event.payload.workspace.blocks[0].note, block.note);
	block.note = 'ก'.repeat(Math.ceil((64 * 1024 * 1024) / 3));
	assert.equal(
		decodeDraftEvent(
			JSON.stringify({
				epoch,
				seq: 0,
				type: 'DraftStateSync',
				payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
			})
		),
		null
	);
});

test('workspace and lookup-delta rows share identity/metadata validation, including unique authoritative orders', () => {
	const workspace = collaborationWorkspace();
	const staff = {
		id: timetableIds.teacherA,
		displayName: 'ครูใหม่',
		status: 'active',
		subjectGroups: [{ id: timetableIds.groupA, name: 'คณิตศาสตร์', displayOrder: null }]
	};
	const valid = { ...emptyLookupChanges(), upsertedStaff: [staff], staffOrder: [staff.id] };
	const patch = {
		epoch,
		seq: 1,
		type: 'DraftPatched',
		payload: {
			timetableVersionId: workspace.version.id,
			mutationId: timetableIds.blockA,
			upsertedBlocks: [],
			removedBlockIds: [],
			ordinaryDemands: [],
			synchronizedDemands: [],
			summary: workspace.summary,
			sourceIssues: [],
			lookupChanges: valid
		}
	};
	assert.equal(decodeDraftEvent(JSON.stringify(patch))?.type, 'DraftPatched');
	for (const changes of [
		{ ...valid, staffOrder: [staff.id, staff.id] },
		{ ...valid, removedStaffIds: ['invalid-selector'] },
		{
			...valid,
			upsertedStaff: [
				{ ...staff, subjectGroups: [{ id: 'invalid', name: 'bad', displayOrder: null }] }
			]
		},
		{ ...valid, upsertedStaff: [staff, staff] }
	])
		assert.equal(
			decodeDraftEvent(
				JSON.stringify({ ...patch, payload: { ...patch.payload, lookupChanges: changes } })
			),
			null
		);
	const invalidStaff = {
		...staff,
		subjectGroups: [{ id: 'invalid', name: 'bad', displayOrder: null }]
	};
	workspace.staff = [invalidStaff];
	assert.equal(
		decodeDraftEvent(
			JSON.stringify({
				epoch,
				seq: 0,
				type: 'DraftStateSync',
				payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
			})
		),
		null
	);
});
