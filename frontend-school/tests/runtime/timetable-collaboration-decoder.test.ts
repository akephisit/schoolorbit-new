import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import { collaborationWorkspace } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

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
