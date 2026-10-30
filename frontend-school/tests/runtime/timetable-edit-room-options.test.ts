import assert from 'node:assert/strict';
import test from 'node:test';
import { timetableEditRoomOptions } from '../../src/lib/academic/timetable/collaboration-presentation.ts';
import type {
	DraftAvailability,
	TimetableBlockPlacementSource
} from '../../src/lib/api/timetable.ts';
import type { OwnedDraftLease } from '../../src/lib/academic/timetable/collaboration-session.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

const source: TimetableBlockPlacementSource = {
	kind: 'existing_block',
	blockId: timetableIds.blockA,
	rowVersion: 1
};
const authorizedRooms = [
	{ id: timetableIds.room, code: 'MATH-1', name: 'ห้องคณิตศาสตร์', status: 'ACTIVE' },
	{ id: timetableIds.roomB, code: 'LAB-2', name: 'ห้องปฏิบัติการ 2', status: 'ACTIVE' }
];
const lease: OwnedDraftLease = {
	proof: { connectionId: timetableIds.user, leaseToken: timetableIds.teacherA },
	reservationId: timetableIds.teacherB,
	source,
	purpose: 'edit',
	target: null,
	expiresAt: new Date(2100000000000).toISOString(),
	valid: true
};
const availability: DraftAvailability = {
	source,
	blockedCells: [],
	roomOptions: {
		reservationId: lease.reservationId,
		availableRoomIds: [timetableIds.roomB, timetableIds.room]
	}
};

test('private edit-room choices intersect active authorized rows and preserve producer workspace order', () => {
	const rooms = authorizedRooms;
	const result = timetableEditRoomOptions(rooms, source, lease, availability);
	assert.equal(result.ready, true);
	assert.deepEqual(result.rooms, rooms);
	assert.equal(result.rooms[0], rooms[0]);
	assert.deepEqual(
		timetableEditRoomOptions(
			rooms.map((room) => ({ ...room, status: 'INACTIVE' })),
			source,
			lease,
			availability
		),
		{ ready: true, rooms: [], retainedRoomId: null }
	);
	assert.deepEqual(
		timetableEditRoomOptions(rooms, source, lease, {
			...availability,
			roomOptions: { reservationId: lease.reservationId, availableRoomIds: [] }
		}),
		{ ready: true, rooms: [], retainedRoomId: null }
	);
});

test('only the unchanged authorized original inactive room can be retained when the server allows it', () => {
	const rooms = authorizedRooms.map((room) => ({ ...room, status: 'INACTIVE' }));
	assert.equal(
		timetableEditRoomOptions(rooms, source, lease, availability, timetableIds.room).retainedRoomId,
		timetableIds.room
	);
	assert.equal(
		timetableEditRoomOptions(
			rooms,
			source,
			lease,
			{
				...availability,
				roomOptions: { reservationId: lease.reservationId, availableRoomIds: [] }
			},
			timetableIds.room
		).retainedRoomId,
		null
	);
	assert.equal(
		timetableEditRoomOptions([], source, lease, availability, timetableIds.room).retainedRoomId,
		null
	);
});

test('missing, stale reservation, row version, block, purpose and retired-lease lists fail closed', () => {
	const rooms = authorizedRooms;
	const empty = { ready: false, rooms: [], retainedRoomId: null };
	for (const payload of [
		null,
		{ ...availability, roomOptions: null },
		{
			...availability,
			roomOptions: { ...availability.roomOptions!, reservationId: timetableIds.user }
		},
		{ ...availability, source: { ...source, rowVersion: 2 } },
		{ ...availability, source: { ...source, blockId: timetableIds.blockB } }
	])
		assert.deepEqual(timetableEditRoomOptions(rooms, source, lease, payload), empty);
	assert.deepEqual(
		timetableEditRoomOptions(rooms, source, { ...lease, valid: false }, availability),
		empty
	);
	assert.deepEqual(
		timetableEditRoomOptions(rooms, source, { ...lease, purpose: 'drag' }, availability),
		empty
	);
	assert.deepEqual(
		timetableEditRoomOptions(rooms, { ...source, rowVersion: 2 }, lease, availability),
		empty
	);
	assert.deepEqual(timetableEditRoomOptions(rooms, source, null, availability), empty);
});
