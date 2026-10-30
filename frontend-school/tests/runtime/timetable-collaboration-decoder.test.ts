import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import { applyDraftPatch } from '../../src/lib/academic/timetable/collaboration-patch.ts';
import { collaborationWorkspace, emptyLookupChanges } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';
import { makeTimetableBlock } from '../e2e/timetable-test-harness.ts';

const epoch = 'f1000000-0000-4000-8000-000000000999';

const nullableRoom = { id: timetableIds.room, code: null, name: 'ห้องเรียน', status: 'ACTIVE' };
const metadataCases = [
	{
		name: 'version.createdBy',
		valid: timetableIds.user,
		workspace(value: unknown) {
			const workspace = collaborationWorkspace();
			return { ...workspace, version: { ...workspace.version, createdBy: value } };
		}
	},
	{
		name: 'homeroom.roomNumber',
		valid: '1',
		workspace(value: unknown) {
			const workspace = collaborationWorkspace();
			return { ...workspace, homerooms: [{ ...workspace.homerooms[0], roomNumber: value }] };
		}
	},
	{
		name: 'room.code',
		valid: 'ROOM-1',
		workspace(value: unknown) {
			return { ...collaborationWorkspace(), rooms: [{ ...nullableRoom, code: value }] };
		}
	}
];
for (const field of metadataCases) {
	test(`required nullable ${field.name} accepts explicit null and its declared non-null type`, () => {
		for (const value of [null, field.valid]) {
			const event = {
				epoch,
				seq: 0,
				type: 'DraftStateSync',
				payload: {
					connectionId: timetableIds.teacherA,
					workspace: field.workspace(value),
					users: [],
					reservations: []
				}
			};
			assert.deepEqual(decodeDraftEvent(JSON.stringify(event)), event);
		}
	});
	test(`required nullable ${field.name} still rejects missing and malformed non-null metadata`, () => {
		for (const value of [
			undefined,
			1,
			false,
			{},
			[],
			...(field.name === 'version.createdBy' ? ['not-a-uuid'] : [])
		]) {
			assert.equal(
				decodeDraftEvent(
					JSON.stringify({
						epoch,
						seq: 0,
						type: 'DraftStateSync',
						payload: {
							connectionId: timetableIds.teacherA,
							workspace: field.workspace(value),
							users: [],
							reservations: []
						}
					})
				),
				null
			);
		}
	});
}

test('nullable lookup metadata and patch version decode and retain explicit nulls through the shared patch consumer', () => {
	const workspace = collaborationWorkspace();
	const homeroom = { ...workspace.homerooms[0], roomNumber: null };
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
			version: { ...workspace.version, createdBy: null },
			lookupChanges: {
				...emptyLookupChanges(),
				upsertedRooms: [nullableRoom],
				roomOrder: [nullableRoom.id],
				upsertedHomerooms: [homeroom],
				homeroomOrder: [homeroom.id]
			}
		}
	};
	const decoded = decodeDraftEvent(JSON.stringify(patch));
	assert.equal(decoded?.type, 'DraftPatched');
	if (decoded?.type !== 'DraftPatched') return;
	const updated = applyDraftPatch(workspace, decoded.payload);
	assert.equal(updated.version.createdBy, null);
	assert.equal(updated.homerooms[0].roomNumber, null);
	assert.equal(updated.rooms[0].code, null);
	for (const lookupChanges of [
		{
			...patch.payload.lookupChanges,
			upsertedRooms: [{ id: nullableRoom.id, name: 'ห้องเรียน', status: 'ACTIVE' }]
		},
		{ ...patch.payload.lookupChanges, upsertedHomerooms: [{ ...homeroom, roomNumber: 1 }] }
	])
		assert.equal(
			decodeDraftEvent(JSON.stringify({ ...patch, payload: { ...patch.payload, lookupChanges } })),
			null
		);
});

test('decoder validates optional private room options and rejects invalid or duplicated room identities', () => {
	const payload = {
		source: { kind: 'existing_block', blockId: timetableIds.blockA, rowVersion: 1 },
		blockedCells: [],
		roomOptions: { reservationId: timetableIds.teacherA, availableRoomIds: [timetableIds.room] }
	};
	const decode = (roomOptions: unknown) =>
		decodeDraftEvent(
			JSON.stringify({
				epoch,
				seq: 0,
				type: 'DraftAvailability',
				payload: { ...payload, roomOptions }
			})
		);
	assert.ok(decode(payload.roomOptions));
	assert.ok(decode(undefined));
	assert.ok(decode(null));
	assert.ok(decode({ ...payload.roomOptions, availableRoomIds: [] }));
	for (const invalid of [
		{ ...payload.roomOptions, reservationId: 'invalid' },
		{ ...payload.roomOptions, availableRoomIds: [timetableIds.room, timetableIds.room] },
		{ ...payload.roomOptions, availableRoomIds: ['invalid'] },
		{ reservationId: timetableIds.teacherA },
		{ ...payload.roomOptions, hiddenConflict: timetableIds.blockB }
	])
		assert.equal(decode(invalid), null);
});
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
