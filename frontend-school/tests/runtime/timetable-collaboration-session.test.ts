import assert from 'node:assert/strict';
import test from 'node:test';
import { createDraftCollaborationSession } from '../../src/lib/academic/timetable/collaboration-session.ts';
import { decodeDraftEvent } from '../../src/lib/academic/timetable/collaboration-decoder.ts';
import type {
	DraftClientMessage,
	DraftCommandResult,
	DraftEventEnvelope,
	DraftPatch,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource
} from '../../src/lib/api/timetable.js';
import { collaborationWorkspace } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

const source: TimetableBlockPlacementSource = {
	kind: 'ordinary_demand',
	learningGroupId: timetableIds.groupA,
	learningOfferingId: timetableIds.offeringA
};
const candidate: TimetableBlockPlacementCandidate = {
	blockKind: 'course',
	learningGroupId: timetableIds.groupA,
	learningOfferingId: timetableIds.offeringA,
	homeroomIds: [timetableIds.homeroom],
	instructorIds: [timetableIds.teacherA],
	teacherIds: [],
	roomId: timetableIds.room
};
const target = { dayOfWeek: 'MON', bellSchedulePeriodId: timetableIds.period1, blockId: null };

function harness() {
	let serial = 0;
	const sent: DraftClientMessage[] = [],
		events: DraftEventEnvelope[] = [],
		reconciles: string[] = [];
	const timers = new Map<number, { callback(): void; delay: number }>();
	const session = createDraftCollaborationSession({
		send(message) {
			sent.push(message);
			return true;
		},
		uuid: () => `request-${++serial}`,
		setTimer(callback, delay) {
			const id = ++serial;
			timers.set(id, { callback, delay });
			return id;
		},
		clearTimer(id) {
			timers.delete(id);
		},
		onState() {},
		onEvent: (event) => events.push(event),
		onReconcile: (reason) => reconciles.push(reason),
		onLeaseLost() {}
	});
	const sync = (seq = 0) =>
		session.apply({
			epoch: 'epoch-one',
			seq,
			type: 'DraftStateSync',
			payload: {
				connectionId: 'tab-one',
				workspace: collaborationWorkspace(),
				users: [],
				reservations: []
			}
		});
	const reply = (result: Partial<DraftCommandResult> = {}) => {
		const request = sent.at(-1);
		assert.ok(request && 'requestId' in request.payload);
		const command =
			request.type === 'ClientClaim'
				? 'claim'
				: request.type === 'ClientTarget'
					? 'target'
					: request.type === 'ClientRenew'
						? 'renew'
						: 'release';
		session.apply({
			epoch: 'epoch-one',
			seq: session.state.seq,
			type: 'DraftCommandResult',
			payload: {
				requestId: request.payload.requestId,
				command,
				accepted: true,
				source,
				leaseToken: 'private-token',
				reservationId: 'public-reservation',
				expiresAt: '2030-01-01T00:00:00Z',
				target: command === 'target' ? target : null,
				...result
			}
		});
	};
	session.reset(timetableIds.draftVersion);
	sync();
	return { session, sent, events, reconciles, timers, reply, sync };
}

function patch(seq: number): DraftEventEnvelope {
	const workspace = collaborationWorkspace();
	const payload: DraftPatch = {
		timetableVersionId: timetableIds.draftVersion,
		mutationId: `mutation-${seq}`,
		originConnectionId: 'tab-two',
		upsertedBlocks: [],
		removedBlockIds: [],
		ordinaryDemands: [],
		synchronizedDemands: [],
		summary: workspace.summary,
		sourceIssues: [],
		totalDraftBlockCount: 0
	};
	return { epoch: 'epoch-one', seq, type: 'DraftPatched', payload };
}

for (const accepted of [true, false]) {
	test(`release retires local proof and resolves ${accepted ? 'an accepted release' : 'the authoritative stale_reservation after commit'}`, async () => {
		const h = harness();
		const claimed = h.session.claim(source, candidate, 'drag');
		h.reply();
		const lease = await claimed;
		const released = h.session.release(lease);
		assert.equal(h.session.owns(lease), false);
		assert.equal([...h.timers.values()].filter((timer) => timer.delay === 3000).length, 0);
		h.reply({
			accepted,
			code: accepted ? null : 'stale_reservation',
			source: null,
			leaseToken: null,
			reservationId: null,
			expiresAt: null
		});
		await released;
		assert.equal(h.timers.size, 0);
		const sent = h.sent.length;
		await h.session.release(lease);
		assert.equal(h.sent.length, sent, 'releasing the locally retired proof is idempotent');
		const reclaimed = h.session.claim(source, candidate, 'drag');
		h.reply({ leaseToken: 'new-private-token' });
		assert.equal(h.session.owns(await reclaimed), true);
		h.session.reset(null);
	});
}

for (const code of ['forbidden', 'invalid_candidate', 'reserved', 'unknown', 'stale', null]) {
	test(`release still rejects unsuccessful ${String(code)} results`, async () => {
		const h = harness();
		const claimed = h.session.claim(source, candidate, 'drag');
		h.reply();
		const lease = await claimed;
		const released = h.session.release(lease);
		h.reply({ accepted: false, code });
		await assert.rejects(released);
		assert.equal(h.session.owns(lease), false);
		assert.equal(h.timers.size, 0);
	});
}

test('release still rejects a mismatched command even when it carries the terminal stale code', async () => {
	const h = harness();
	const claimed = h.session.claim(source, candidate, 'edit');
	h.reply();
	const released = h.session.release(await claimed);
	h.reply({ command: 'target', accepted: false, code: 'stale_reservation' });
	await assert.rejects(released);
	assert.equal(h.timers.size, 0);
});

test('release preserves its bounded timeout when no correlated response arrives', async () => {
	const h = harness();
	const claimed = h.session.claim(source, candidate, 'edit');
	h.reply();
	const released = h.session.release(await claimed);
	const rejected = assert.rejects(released);
	h.reply({ requestId: 'different-request', accepted: false, code: 'stale_reservation' });
	assert.equal(h.timers.size, 1);
	const timeout = [...h.timers.values()][0];
	assert.equal(timeout.delay, 4000);
	timeout.callback();
	await rejected;
	assert.equal(h.timers.size, 0);
});

test('a decoded combined-null metadata baseline makes the session ready and admits an owned claim', async () => {
	const h = harness();
	h.session.reset(timetableIds.draftVersion);
	assert.equal(h.session.state.ready, false);
	const workspace = collaborationWorkspace();
	workspace.version.createdBy = null;
	workspace.homerooms[0].roomNumber = null;
	workspace.rooms = [{ id: timetableIds.room, code: null, name: 'ห้องเรียน', status: 'ACTIVE' }];
	const epoch = timetableIds.blockA;
	const decoded = decodeDraftEvent(
		JSON.stringify({
			epoch,
			seq: 0,
			type: 'DraftStateSync',
			payload: { connectionId: timetableIds.teacherA, workspace, users: [], reservations: [] }
		})
	);
	assert.ok(decoded);
	h.session.apply(decoded);
	assert.equal(h.session.state.ready, true);
	const claimed = h.session.claim(source, candidate, 'drag');
	const request = h.sent.at(-1);
	assert.ok(request?.type === 'ClientClaim');
	h.session.apply({
		epoch,
		seq: 0,
		type: 'DraftCommandResult',
		payload: {
			requestId: request.payload.requestId,
			command: 'claim',
			accepted: true,
			source,
			leaseToken: timetableIds.groupA,
			reservationId: timetableIds.groupB,
			expiresAt: '2030-01-01T00:00:00Z'
		}
	});
	const lease = await claimed;
	assert.equal(h.session.owns(lease), true);
	assert.equal(lease.proof.connectionId, timetableIds.teacherA);
	assert.deepEqual(h.reconciles, []);
	h.session.reset(null);
});

test('sequence gaps request one replay, apply patches in order and ignore stale snapshots/duplicates', () => {
	const h = harness();
	h.session.apply(patch(2));
	h.session.apply(patch(3));
	assert.equal(h.sent.filter((message) => message.type === 'ClientReplay').length, 1);
	assert.equal(h.events.filter((event) => event.type === 'DraftPatched').length, 0);
	h.session.apply(patch(1));
	assert.deepEqual(
		h.events.filter((event) => event.type === 'DraftPatched').map((event) => event.seq),
		[1, 2, 3]
	);
	h.session.apply(patch(2));
	h.sync(0);
	assert.equal(h.session.state.seq, 3);
	assert.equal(h.timers.size, 0);
	h.session.apply({ ...patch(4), epoch: 'another-epoch' });
	assert.deepEqual(h.reconciles, ['epoch_changed']);
});

test('late accepted claims after cancellation are released without acquiring local ownership', async () => {
	const h = harness(),
		abort = new AbortController();
	const claimed = h.session.claim(source, candidate, 'drag', abort.signal);
	const rejected = assert.rejects(claimed);
	abort.abort();
	await rejected;
	h.reply();
	assert.equal(h.sent.at(-1)?.type, 'ClientRelease');
	assert.equal(h.timers.size, 0);
});

test('a claim grant for another source is refused and its private token is released', async () => {
	const h = harness();
	const claimed = h.session.claim(source, candidate, 'drag');
	h.reply({
		source: { kind: 'synchronized_offering', learningOfferingId: timetableIds.offeringSync }
	});
	await assert.rejects(claimed);
	assert.equal(h.sent.at(-1)?.type, 'ClientRelease');
});

test('confirmed target correlation prevents a mismatched target from becoming drop proof', async () => {
	const h = harness();
	const claimed = h.session.claim(source, candidate, 'drag');
	h.reply();
	const lease = await claimed;
	const selected = h.session.target(lease, target, candidate);
	h.reply({ target: { ...target, bellSchedulePeriodId: timetableIds.period2 } });
	await assert.rejects(selected);
	assert.equal(lease.target, null);
	const confirmed = h.session.target(lease, target, candidate);
	h.reply();
	await confirmed;
	assert.deepEqual(lease.target, target);
	assert.deepEqual(h.session.mutation(lease).connectionId, 'tab-one');
	h.session.reset(null);
	assert.equal(lease.valid, false);
	assert.throws(() => h.session.mutation(lease));
	assert.equal(h.timers.size, 0);
});

test('an unrelated second lease does not retire the pending first save and both renew', async () => {
	const h = harness();
	const first = h.session.claim(source, candidate, 'drag');
	h.reply({ leaseToken: 'token-one' });
	const firstLease = await first;
	const second = h.session.claim(source, candidate, 'edit');
	h.reply({ leaseToken: 'token-two', reservationId: 'reservation-two' });
	const secondLease = await second;
	assert.equal(h.session.owns(firstLease), true);
	assert.equal(h.session.owns(secondLease), true);
	const renewals = [...h.timers.values()].filter((timer) => timer.delay === 3000);
	assert.equal(renewals.length, 2);
	for (const timer of renewals) timer.callback();
	assert.equal(h.sent.filter((message) => message.type === 'ClientRenew').length, 2);
	h.session.reset(null);
});

test('two tabs of the same person survive presence updates and closing one tab removes only it', () => {
	const h = harness();
	h.session.apply({
		epoch: 'epoch-one',
		seq: 0,
		type: 'DraftPresenceChanged',
		payload: {
			userId: 'one-user',
			name: 'Same user',
			color: '#123456',
			connections: [{ connectionId: 'tab-one' }, { connectionId: 'tab-two' }]
		}
	});
	assert.equal(h.session.state.users[0].connections.length, 2);
	h.session.apply({
		epoch: 'epoch-one',
		seq: 0,
		type: 'DraftPresenceLeft',
		payload: { connectionId: 'tab-two' }
	});
	assert.deepEqual(
		h.session.state.users[0].connections.map((connection) => connection.connectionId),
		['tab-one']
	);
});

test('presence-only updates retain existing participant order while joins append and departures remove only their member', () => {
	const h = harness();
	const users = [
		timetableIds.teacherA,
		timetableIds.teacherB,
		timetableIds.blockA,
		timetableIds.blockB
	].map((userId) => ({
		userId,
		name: userId,
		color: '#123456',
		connections: [{ connectionId: userId }]
	}));
	for (const user of users)
		h.session.apply({ epoch: 'epoch-one', seq: 0, type: 'DraftPresenceChanged', payload: user });
	for (const user of [users[0], users[3], users[1], users[2]]) {
		h.session.apply({
			epoch: 'epoch-one',
			seq: 0,
			type: 'DraftPresenceChanged',
			payload: { ...user, name: 'ชื่อที่ปรับปรุง', profileImageFileId: timetableIds.room }
		});
		assert.deepEqual(
			h.session.state.users.map((value) => value.userId),
			users.map((value) => value.userId)
		);
	}
	const joined = {
		...users[0],
		userId: timetableIds.user,
		connections: [{ connectionId: timetableIds.user }]
	};
	h.session.apply({ epoch: 'epoch-one', seq: 0, type: 'DraftPresenceChanged', payload: joined });
	assert.deepEqual(
		h.session.state.users.map((value) => value.userId),
		[...users.map((value) => value.userId), joined.userId]
	);
	h.session.apply({
		epoch: 'epoch-one',
		seq: 0,
		type: 'DraftPresenceLeft',
		payload: { connectionId: users[1].connections[0].connectionId }
	});
	assert.deepEqual(
		h.session.state.users.map((value) => value.userId),
		[users[0].userId, users[2].userId, users[3].userId, joined.userId]
	);
});
