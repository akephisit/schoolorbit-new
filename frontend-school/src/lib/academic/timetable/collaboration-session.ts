import type {
	TimetableAvailability,
	TimetableClientMessage,
	TimetableCommandResult,
	TimetableContext,
	TimetableEventEnvelope,
	TimetableMutationIdentity,
	TimetableParticipant,
	TimetableReservation,
	TimetableReservationPurpose,
	TimetableTarget,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource
} from '../../api/timetable.js';

export interface DraftSessionState {
	timetableId: string | null;
	connectionId: string | null;
	epoch: string | null;
	seq: number;
	ready: boolean;
	users: TimetableParticipant[];
	reservations: TimetableReservation[];
	availability: TimetableAvailability | null;
	leaseRevision: number;
}

/** A local owned interaction. Its proof is never part of a public reservation. */
export interface OwnedDraftLease {
	proof: Omit<TimetableMutationIdentity, 'mutationId'>;
	reservationId: string;
	source: TimetableBlockPlacementSource;
	purpose: TimetableReservationPurpose;
	target: TimetableTarget | null;
	expiresAt: string;
	valid: boolean;
}

type CommandMessage = Extract<
	TimetableClientMessage,
	{ type: 'ClientClaim' | 'ClientTarget' | 'ClientRenew' | 'ClientRelease' }
>;

export function sameDraftSource(
	left: TimetableBlockPlacementSource,
	right: TimetableBlockPlacementSource
): boolean {
	if (left.kind !== right.kind) return false;
	if (left.kind === 'existing_block' && right.kind === 'existing_block')
		return left.blockId === right.blockId && left.rowVersion === right.rowVersion;
	if (left.kind === 'ordinary_demand' && right.kind === 'ordinary_demand')
		return (
			left.learningGroupId === right.learningGroupId &&
			left.learningOfferingId === right.learningOfferingId
		);
	return (
		left.kind === 'synchronized_offering' &&
		right.kind === 'synchronized_offering' &&
		left.learningOfferingId === right.learningOfferingId
	);
}

export function sameTimetableTarget(
	left: TimetableTarget | null | undefined,
	right: TimetableTarget | null | undefined
): boolean {
	return !left || !right
		? !left && !right
		: left.dayOfWeek === right.dayOfWeek &&
				left.bellSchedulePeriodId === right.bellSchedulePeriodId &&
				(left.blockId ?? null) === (right.blockId ?? null);
}

export class DraftLeaseError extends Error {
	readonly code: TimetableCommandResult['code'];
	constructor(code?: TimetableCommandResult['code']) {
		super(
			code === 'reserved'
				? 'มีผู้อื่นกำลังใช้คาบหรือช่องเวลานี้ กรุณาเลือกตำแหน่งอื่น'
				: 'สิทธิ์จัดคาบหมดอายุหรือการเชื่อมต่อเปลี่ยนไป กรุณาลองใหม่'
		);
		this.name = 'DraftLeaseError';
		this.code = code;
	}
}
function leaseError(code?: TimetableCommandResult['code']): DraftLeaseError {
	return new DraftLeaseError(code);
}

export function createDraftCollaborationSession<Timer>(dependencies: {
	send(message: TimetableClientMessage): boolean;
	uuid(): string;
	setTimer(callback: () => void, delay: number): Timer;
	clearTimer(timer: Timer): void;
	onState(state: DraftSessionState): void;
	onEvent(event: TimetableEventEnvelope): void;
	onReconcile(reason: string): void;
	onLeaseLost(lease: OwnedDraftLease, error: Error): void;
}) {
	let state: DraftSessionState = initialState(null);
	let generation = 0;
	let replayTimer: Timer | null = null;
	const buffered = new Map<number, TimetableEventEnvelope>();
	const leases = new Map<string, { lease: OwnedDraftLease; timer: Timer | null }>();
	const pending = new Map<
		string,
		{
			message: CommandMessage;
			resolve(result: TimetableCommandResult): void;
			reject(error: Error): void;
			timer: Timer;
			dispose(): void;
		}
	>();

	function initialState(timetableId: string | null): DraftSessionState {
		return {
			timetableId,
			connectionId: null,
			epoch: null,
			seq: 0,
			ready: false,
			users: [],
			reservations: [],
			availability: null,
			leaseRevision: 0
		};
	}
	function publish(changes: Partial<DraftSessionState> = {}) {
		state = { ...state, ...changes };
		dependencies.onState(state);
	}
	function clearReplay() {
		if (replayTimer !== null) dependencies.clearTimer(replayTimer);
		replayTimer = null;
	}
	function retireLease(lease: OwnedDraftLease) {
		const owned = leases.get(lease.proof.leaseToken);
		if (owned?.timer !== null && owned?.timer !== undefined) dependencies.clearTimer(owned.timer);
		leases.delete(lease.proof.leaseToken);
		lease.valid = false;
		publish({ leaseRevision: state.leaseRevision + 1 });
	}
	function reset(timetableId: string | null = null) {
		generation++;
		clearReplay();
		buffered.clear();
		for (const command of [...pending.values()]) {
			dependencies.clearTimer(command.timer);
			command.dispose();
			command.reject(leaseError());
		}
		pending.clear();
		for (const { lease, timer } of leases.values()) {
			lease.valid = false;
			if (timer !== null) dependencies.clearTimer(timer);
		}
		leases.clear();
		state = initialState(timetableId);
		dependencies.onState(state);
	}
	function replay() {
		if (!state.epoch || replayTimer !== null) return;
		dependencies.send({
			type: 'ClientReplay',
			payload: { epoch: state.epoch, afterSeq: state.seq }
		});
		const current = generation;
		replayTimer = dependencies.setTimer(() => {
			replayTimer = null;
			if (current === generation) dependencies.onReconcile('journal_gap');
		}, 3000);
	}
	function emitPatch(event: TimetableEventEnvelope) {
		publish({ seq: event.seq });
		dependencies.onEvent(event);
	}
	function apply(event: TimetableEventEnvelope) {
		if (!state.timetableId) return;
		if (event.type === 'TimetableStateSync') {
			if (event.payload.workspace.timetable.id !== state.timetableId) return;
			if (state.connectionId && event.payload.connectionId !== state.connectionId) return;
			if (state.ready && event.epoch === state.epoch && event.seq < state.seq) return;
			clearReplay();
			buffered.clear();
			publish({
				connectionId: event.payload.connectionId,
				epoch: event.epoch,
				seq: event.seq,
				ready: true,
				users: event.payload.users,
				reservations: event.payload.reservations,
				availability: null
			});
			dependencies.onEvent(event);
			return;
		}
		if (!state.ready) return;
		if (event.epoch !== state.epoch) {
			dependencies.onReconcile('epoch_changed');
			return;
		}
		if (event.type === 'TimetablePatched') {
			if (event.payload.timetableId !== state.timetableId || event.seq <= state.seq) return;
			if (event.seq > state.seq + 1) {
				if (buffered.size >= 128) dependencies.onReconcile('journal_gap');
				else {
					buffered.set(event.seq, event);
					replay();
				}
				return;
			}
			emitPatch(event);
			while (buffered.has(state.seq + 1)) {
				const next = buffered.get(state.seq + 1)!;
				buffered.delete(state.seq + 1);
				emitPatch(next);
			}
			if (buffered.size === 0) clearReplay();
			return;
		}
		switch (event.type) {
			case 'TimetablePresenceChanged':
				publish({
					users: state.users.some((user) => user.userId === event.payload.userId)
						? state.users.map((user) =>
								user.userId === event.payload.userId ? event.payload : user
							)
						: [...state.users, event.payload]
				});
				break;
			case 'TimetablePresenceLeft':
				publish({
					users: state.users
						.map((user) => ({
							...user,
							connections: user.connections.filter(
								(connection) => connection.connectionId !== event.payload.connectionId
							)
						}))
						.filter((user) => user.connections.length > 0)
				});
				break;
			case 'TimetableReservationsChanged':
				publish({ reservations: event.payload.reservations });
				break;
			case 'TimetableAvailability':
				publish({ availability: event.payload });
				break;
			case 'TimetableCommandResult': {
				const command = pending.get(event.payload.requestId);
				if (!command) {
					if (
						event.payload.command === 'claim' &&
						event.payload.accepted &&
						event.payload.leaseToken
					)
						dependencies.send({
							type: 'ClientRelease',
							payload: { requestId: dependencies.uuid(), leaseToken: event.payload.leaseToken }
						});
					break;
				}
				pending.delete(event.payload.requestId);
				dependencies.clearTimer(command.timer);
				command.dispose();
				const expected = {
					ClientClaim: 'claim',
					ClientTarget: 'target',
					ClientRenew: 'renew',
					ClientRelease: 'release'
				}[command.message.type];
				if (event.payload.command !== expected) {
					if (
						event.payload.accepted &&
						event.payload.command === 'claim' &&
						event.payload.leaseToken
					)
						dependencies.send({
							type: 'ClientRelease',
							payload: { requestId: dependencies.uuid(), leaseToken: event.payload.leaseToken }
						});
					command.reject(leaseError());
					break;
				}
				command.resolve(event.payload);
				break;
			}
			case 'TimetableReconcileRequired':
				dependencies.onReconcile(event.payload.reason);
				break;
		}
		if (event.seq > state.seq && event.type !== 'TimetableReconcileRequired') replay();
	}

	function command(message: CommandMessage, signal?: AbortSignal): Promise<TimetableCommandResult> {
		if (!state.ready || !state.connectionId || signal?.aborted) return Promise.reject(leaseError());
		return new Promise((resolve, reject) => {
			const id = message.payload.requestId;
			function fail() {
				const operation = pending.get(id);
				if (!operation) return;
				pending.delete(id);
				dependencies.clearTimer(operation.timer);
				operation.dispose();
				reject(leaseError());
			}
			pending.set(id, {
				message,
				resolve,
				reject,
				timer: dependencies.setTimer(fail, 4000),
				dispose: () => signal?.removeEventListener('abort', fail)
			});
			signal?.addEventListener('abort', fail, { once: true });
			if (!dependencies.send(message)) fail();
		});
	}
	function owns(lease: OwnedDraftLease): boolean {
		return (
			lease.valid &&
			state.ready &&
			state.connectionId === lease.proof.connectionId &&
			leases.get(lease.proof.leaseToken)?.lease === lease
		);
	}
	function renewLater(lease: OwnedDraftLease) {
		const owned = leases.get(lease.proof.leaseToken);
		if (!owned || !owns(lease)) return;
		owned.timer = dependencies.setTimer(() => {
			owned.timer = null;
			void command({
				type: 'ClientRenew',
				payload: { requestId: dependencies.uuid(), leaseToken: lease.proof.leaseToken }
			})
				.then((result) => {
					if (!owns(lease)) return;
					if (!result.accepted || result.leaseToken !== lease.proof.leaseToken || !result.expiresAt)
						throw leaseError(result.code);
					lease.expiresAt = result.expiresAt;
					renewLater(lease);
				})
				.catch((error: unknown) => {
					if (!owns(lease)) return;
					retireLease(lease);
					dependencies.onLeaseLost(lease, error instanceof Error ? error : leaseError());
				});
		}, 3000);
	}
	async function claim(
		source: TimetableBlockPlacementSource,
		candidate: TimetableBlockPlacementCandidate,
		purpose: TimetableReservationPurpose,
		signal?: AbortSignal
	): Promise<OwnedDraftLease> {
		const current = generation;
		const result = await command(
			{
				type: 'ClientClaim',
				payload: { requestId: dependencies.uuid(), source, candidate, purpose }
			},
			signal
		);
		if (!result.accepted) throw leaseError(result.code);
		if (
			current !== generation ||
			!state.connectionId ||
			result.command !== 'claim' ||
			!result.source ||
			!sameDraftSource(result.source, source) ||
			!result.leaseToken ||
			!result.reservationId ||
			!result.expiresAt
		) {
			if (result.leaseToken)
				dependencies.send({
					type: 'ClientRelease',
					payload: { requestId: dependencies.uuid(), leaseToken: result.leaseToken }
				});
			throw leaseError();
		}
		const lease: OwnedDraftLease = {
			proof: { connectionId: state.connectionId, leaseToken: result.leaseToken },
			reservationId: result.reservationId,
			source,
			purpose,
			target: null,
			expiresAt: result.expiresAt,
			valid: true
		};
		leases.set(lease.proof.leaseToken, { lease, timer: null });
		publish({ leaseRevision: state.leaseRevision + 1 });
		renewLater(lease);
		return lease;
	}
	async function target(
		lease: OwnedDraftLease,
		selected: TimetableTarget | null,
		candidate: TimetableBlockPlacementCandidate,
		signal?: AbortSignal
	): Promise<void> {
		if (!owns(lease)) throw leaseError();
		const result = await command(
			{
				type: 'ClientTarget',
				payload: {
					requestId: dependencies.uuid(),
					leaseToken: lease.proof.leaseToken,
					target: selected,
					candidate
				}
			},
			signal
		);
		if (
			!owns(lease) ||
			!result.accepted ||
			result.command !== 'target' ||
			result.leaseToken !== lease.proof.leaseToken ||
			!result.source ||
			!sameDraftSource(result.source, lease.source) ||
			!sameTimetableTarget(result.target, selected)
		)
			throw leaseError(result.code);
		lease.target = selected;
		if (result.expiresAt) lease.expiresAt = result.expiresAt;
	}
	async function release(lease: OwnedDraftLease): Promise<void> {
		if (!owns(lease)) return;
		retireLease(lease);
		const result = await command({
			type: 'ClientRelease',
			payload: { requestId: dependencies.uuid(), leaseToken: lease.proof.leaseToken }
		});
		if (!result.accepted && result.code !== 'stale_reservation') throw leaseError(result.code);
	}
	function presence(
		context: TimetableContext,
		anchor: import('../../api/timetable.js').TimetableAnchor | null
	) {
		return (
			state.ready &&
			dependencies.send({ type: 'ClientPresenceUpdate', payload: { context, anchor } })
		);
	}
	function mutation(lease: OwnedDraftLease): TimetableMutationIdentity {
		if (!owns(lease)) throw leaseError();
		return { ...lease.proof, mutationId: dependencies.uuid() };
	}
	return {
		reset,
		apply,
		claim,
		target,
		release,
		owns,
		presence,
		mutation,
		get state() {
			return state;
		}
	};
}
