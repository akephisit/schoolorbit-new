import { randomUUID, createHash } from 'node:crypto';
import { createServer } from 'node:http';
import type { Duplex } from 'node:stream';
import type { Page, WebSocketRoute } from '@playwright/test';
import type {
	DraftClientMessage,
	DraftEventEnvelope,
	DraftParticipant,
	DraftReservation,
	TimetableBlockWorkspace,
	DraftPatch,
	DraftLookupChanges,
	TimetableBlockPlacementCandidate
} from '../../src/lib/api/timetable.js';

type ServerMessage<T = DraftEventEnvelope> = T extends DraftEventEnvelope
	? Omit<T, 'epoch' | 'seq'>
	: never;
type FixturePeer = {
	id: string;
	user: DraftParticipant;
	send(value: DraftEventEnvelope): void;
	active: string | null;
};
type FixtureLease = {
	token: string;
	public: DraftReservation;
	candidate: TimetableBlockPlacementCandidate | null;
};

function lookupDelta<T>(before: readonly T[], after: readonly T[], key: (value: T) => string) {
	const previous = new Map(before.map((value) => [key(value), value]));
	const nextIds = new Set(after.map(key));
	const upserted = after.filter(
		(value) => JSON.stringify(previous.get(key(value))) !== JSON.stringify(value)
	);
	const removed = before.filter((value) => !nextIds.has(key(value))).map(key);
	const changed =
		JSON.stringify(before.map(key)) !== JSON.stringify(after.map(key)) || upserted.length > 0;
	return { upserted, removed, order: changed ? after.map(key) : undefined, changed };
}
function lookupChanges(
	before: TimetableBlockWorkspace,
	after: TimetableBlockWorkspace
): DraftLookupChanges | undefined {
	const staff = lookupDelta(before.staff, after.staff, (value) => value.id),
		homerooms = lookupDelta(before.homerooms, after.homerooms, (value) => value.id),
		rooms = lookupDelta(before.rooms, after.rooms, (value) => value.id),
		periods = lookupDelta(before.bellPeriods, after.bellPeriods, (value) => value.id),
		subjects = lookupDelta(
			before.offeringSubjectGroups,
			after.offeringSubjectGroups,
			(value) => value.learningOfferingId
		);
	if (![staff, homerooms, rooms, periods, subjects].some((value) => value.changed))
		return undefined;
	return {
		upsertedStaff: staff.upserted,
		removedStaffIds: staff.removed,
		staffOrder: staff.order,
		upsertedHomerooms: homerooms.upserted,
		removedHomeroomIds: homerooms.removed,
		homeroomOrder: homerooms.order,
		upsertedRooms: rooms.upserted,
		removedRoomIds: rooms.removed,
		roomOrder: rooms.order,
		upsertedBellPeriods: periods.upserted,
		removedBellPeriodIds: periods.removed,
		bellPeriodOrder: periods.order,
		upsertedOfferingSubjectGroups: subjects.upserted,
		removedSubjectGroupOfferingIds: subjects.removed,
		subjectGroupOfferingOrder: subjects.order
	};
}

/** Protocol fixture, not a substitute for the real server's resource/authorization tests. */
export function createDraftRelay() {
	const epoch = randomUUID();
	let seq = 0;
	let canonical: TimetableBlockWorkspace | null = null;
	const peers = new Map<string, FixturePeer>();
	const leases = new Map<string, FixtureLease>();
	const journal: DraftEventEnvelope[] = [];
	const received: { connectionId: string; message: DraftClientMessage }[] = [];
	let denyNextTarget = false;
	let denyNextClaim = false;
	let holdNextClaim = false;
	let heldClaim: (() => void) | null = null;
	let holdNextStateSync = false;
	let heldStateSync: (() => void) | null = null;
	let roomOptionsPaused = false;
	function editRoomOptions(active: FixtureLease) {
		const source = active.public.source;
		if (roomOptionsPaused || active.public.purpose !== 'edit' || source.kind !== 'existing_block')
			return undefined;
		const block = canonical?.blocks.find((item) => item.id === source.blockId);
		if (!block || !canonical) return undefined;
		const overlaps = (day: string, start: string, end: string) =>
			day === block.dayOfWeek && start < block.endTime && end > block.startTime;
		const busy = new Set<string>();
		for (const other of canonical.blocks) {
			if (
				other.id === block.id ||
				!other.isActive ||
				!overlaps(other.dayOfWeek, other.startTime, other.endTime)
			)
				continue;
			for (const room of [...other.groups, ...other.homerooms])
				if (room.roomId) busy.add(room.roomId);
		}
		for (const other of leases.values()) {
			if (other === active || !other.candidate?.roomId) continue;
			const target = other.public.target;
			const source = other.public.source;
			const original =
				source.kind === 'existing_block'
					? canonical.blocks.find((item) => item.id === source.blockId)
					: null;
			const period = target
				? canonical.bellPeriods.find((item) => item.id === target.bellSchedulePeriodId)
				: original;
			const day = target?.dayOfWeek ?? original?.dayOfWeek;
			if (period && day && overlaps(day, period.startTime, period.endTime))
				busy.add(other.candidate.roomId);
		}
		return {
			reservationId: active.public.reservationId,
			availableRoomIds: canonical.rooms
				.filter(
					(room) =>
						(room.status.toUpperCase() === 'ACTIVE' ||
							room.id === (block.groups[0]?.roomId ?? block.homerooms[0]?.roomId)) &&
						!busy.has(room.id)
				)
				.map((room) => room.id)
		};
	}
	function send(peer: FixturePeer, message: ServerMessage) {
		if (holdNextStateSync && message.type === 'DraftStateSync') {
			holdNextStateSync = false;
			heldStateSync = () => send(peer, message);
			return;
		}
		if (
			holdNextClaim &&
			message.type === 'DraftCommandResult' &&
			message.payload.command === 'claim' &&
			message.payload.accepted
		) {
			holdNextClaim = false;
			heldClaim = () => send(peer, message);
			return;
		}
		peer.send({ ...message, epoch, seq });
	}
	function broadcast(message: ServerMessage) {
		for (const peer of peers.values()) send(peer, message);
	}
	function users() {
		const result = new Map<string, DraftParticipant>();
		for (const peer of peers.values()) {
			const previous = result.get(peer.user.userId);
			if (previous) previous.connections.push(...peer.user.connections);
			else result.set(peer.user.userId, structuredClone(peer.user));
		}
		return [...result.values()];
	}
	function presence() {
		for (const user of users()) broadcast({ type: 'DraftPresenceChanged', payload: user });
	}
	function reservations() {
		broadcast({
			type: 'DraftReservationsChanged',
			payload: { reservations: [...leases.values()].map((item) => item.public) }
		});
		presence();
		for (const peer of peers.values()) {
			const active = peer.active && leases.get(peer.active);
			if (!active) continue;
			send(peer, {
				type: 'DraftAvailability',
				payload: {
					source: active.public.source,
					context: peer.user.connections[0].context,
					roomOptions: editRoomOptions(active),
					blockedCells: [...leases.values()]
						.filter((item) => item.token !== peer.active && item.public.target)
						.map((item) => ({
							dayOfWeek: item.public.target!.dayOfWeek,
							bellSchedulePeriodId: item.public.target!.bellSchedulePeriodId,
							reservationId: item.public.reservationId,
							ownerConnectionId: item.public.ownerConnectionId,
							purpose: item.public.purpose
						}))
				}
			});
		}
	}
	function activePresence(peer: FixturePeer) {
		peer.user.connections[0].activeReservationId = peer.active
			? (leases.get(peer.active)?.public.reservationId ?? null)
			: null;
	}
	function join(
		workspace: TimetableBlockWorkspace,
		output: FixturePeer['send'],
		identity: { userId: string; name: string }
	) {
		canonical ??= structuredClone(workspace);
		const id = randomUUID();
		const peer: FixturePeer = {
			id,
			send: output,
			active: null,
			user: {
				userId: identity.userId,
				name: identity.name,
				color: '#2563eb',
				profileImageFileId: null,
				connections: [{ connectionId: id, context: null, anchor: null, activeReservationId: null }]
			}
		};
		peers.set(id, peer);
		send(peer, {
			type: 'DraftStateSync',
			payload: {
				connectionId: id,
				workspace: canonical,
				users: users(),
				reservations: [...leases.values()].map((item) => item.public)
			}
		});
		presence();
		return {
			id,
			receive(message: DraftClientMessage) {
				received.push({ connectionId: id, message: structuredClone(message) });
				if (message.type === 'ClientPresenceUpdate') {
					Object.assign(peer.user.connections[0], message.payload);
					presence();
					return;
				}
				if (message.type === 'ClientReplay') {
					for (const event of journal) if (event.seq > message.payload.afterSeq) peer.send(event);
					return;
				}
				const command = (
					{
						ClientClaim: 'claim',
						ClientTarget: 'target',
						ClientRenew: 'renew',
						ClientRelease: 'release'
					} as const
				)[message.type];
				if (message.type === 'ClientClaim') {
					const collision = [...leases.values()].find(
						(item) => JSON.stringify(item.public.source) === JSON.stringify(message.payload.source)
					);
					if (collision || denyNextClaim) {
						denyNextClaim = false;
						send(peer, {
							type: 'DraftCommandResult',
							payload: {
								requestId: message.payload.requestId,
								command,
								accepted: false,
								code: 'reserved'
							}
						});
						return;
					}
					const lease: FixtureLease = {
						token: randomUUID(),
						candidate: message.payload.candidate ?? null,
						public: {
							reservationId: randomUUID(),
							ownerConnectionId: id,
							purpose: message.payload.purpose,
							source: message.payload.source,
							target: null,
							expiresAt: new Date(Date.now() + 10000).toISOString()
						}
					};
					leases.set(lease.token, lease);
					peer.active = lease.token;
					activePresence(peer);
					send(peer, {
						type: 'DraftCommandResult',
						payload: {
							requestId: message.payload.requestId,
							command,
							accepted: true,
							source: lease.public.source,
							leaseToken: lease.token,
							reservationId: lease.public.reservationId,
							expiresAt: lease.public.expiresAt,
							target: null
						}
					});
					reservations();
					return;
				}
				const lease = leases.get(message.payload.leaseToken);
				if (!lease || lease.public.ownerConnectionId !== id) {
					send(peer, {
						type: 'DraftCommandResult',
						payload: {
							requestId: message.payload.requestId,
							command,
							accepted: false,
							code: 'stale_reservation'
						}
					});
					return;
				}
				if (message.type === 'ClientTarget' && denyNextTarget) {
					denyNextTarget = false;
					send(peer, {
						type: 'DraftCommandResult',
						payload: {
							requestId: message.payload.requestId,
							command,
							accepted: false,
							code: 'reserved',
							leaseToken: lease.token,
							source: lease.public.source,
							target: message.payload.target
						}
					});
					return;
				}
				if (message.type === 'ClientTarget') {
					lease.public.target = message.payload.target;
					lease.candidate = message.payload.candidate ?? null;
				}
				lease.public.expiresAt = new Date(Date.now() + 10000).toISOString();
				send(peer, {
					type: 'DraftCommandResult',
					payload: {
						requestId: message.payload.requestId,
						command,
						accepted: true,
						source: lease.public.source,
						target: lease.public.target,
						leaseToken: lease.token,
						reservationId: lease.public.reservationId,
						expiresAt: lease.public.expiresAt
					}
				});
				if (message.type === 'ClientRelease') {
					leases.delete(lease.token);
					if (peer.active === lease.token) peer.active = null;
					activePresence(peer);
				}
				reservations();
			},
			close() {
				peers.delete(id);
				for (const [token, lease] of leases)
					if (lease.public.ownerConnectionId === id) leases.delete(token);
				broadcast({ type: 'DraftPresenceLeft', payload: { connectionId: id } });
				reservations();
			}
		};
	}
	function commit(
		workspace: TimetableBlockWorkspace,
		identity: { connectionId?: string; leaseToken?: string; mutationId?: string } = {},
		publish = true
	) {
		const previous = canonical;
		canonical = structuredClone(workspace);
		const patch: DraftPatch = {
			timetableVersionId: workspace.version.id,
			mutationId: identity.mutationId ?? randomUUID(),
			originConnectionId: identity.connectionId,
			upsertedBlocks: workspace.blocks.filter(
				(block) =>
					JSON.stringify(previous?.blocks.find((item) => item.id === block.id)) !==
					JSON.stringify(block)
			),
			removedBlockIds:
				previous?.blocks
					.filter((block) => !workspace.blocks.some((item) => item.id === block.id))
					.map((block) => block.id) ?? [],
			ordinaryDemands: workspace.ordinaryDemands,
			synchronizedDemands: workspace.synchronizedDemands,
			summary: workspace.summary,
			sourceIssues: workspace.sourceIssues,
			totalDraftBlockCount: workspace.totalDraftBlockCount,
			version: workspace.version,
			lookupChanges: previous ? lookupChanges(previous, workspace) : undefined
		};
		const event: DraftEventEnvelope = { type: 'DraftPatched', payload: patch, epoch, seq: ++seq };
		journal.push(event);
		if (publish) for (const peer of peers.values()) peer.send(event);
		const consumed = identity.leaseToken ? leases.get(identity.leaseToken) : null;
		if (consumed && consumed.public.ownerConnectionId === identity.connectionId) {
			leases.delete(consumed.token);
			const peer = peers.get(consumed.public.ownerConnectionId);
			if (peer?.active === consumed.token) {
				peer.active = null;
				activePresence(peer);
			}
		}
		if (publish) reservations();
		return event;
	}
	return {
		join,
		commit,
		received,
		peers,
		setRoomOptionsPaused(value: boolean) {
			roomOptionsPaused = value;
			reservations();
		},
		denyTarget() {
			denyNextTarget = true;
		},
		denyClaim() {
			denyNextClaim = true;
		},
		holdClaim() {
			holdNextClaim = true;
		},
		holdStateSync() {
			holdNextStateSync = true;
		},
		releaseStateSync() {
			const sync = heldStateSync;
			heldStateSync = null;
			sync?.();
		},
		releaseClaim() {
			const grant = heldClaim;
			heldClaim = null;
			grant?.();
		},
		get workspace() {
			return canonical;
		},
		send(connectionId: string, event: DraftEventEnvelope) {
			peers.get(connectionId)?.send(event);
		},
		reconcile() {
			broadcast({ type: 'DraftReconcileRequired', payload: { reason: 'journal_gap' } });
		}
	};
}
export type DraftRelay = ReturnType<typeof createDraftRelay>;

export async function installRoutedDraftSocket(
	page: Page,
	relay: DraftRelay,
	workspace: (versionId?: string) => TimetableBlockWorkspace,
	identity: { userId: string; name: string }
) {
	const rooms = new Map<string, DraftRelay>([[workspace().version.id, relay]]);
	await page.routeWebSocket('**/ws/timetable*', (socket: WebSocketRoute) => {
		if (!new URL(socket.url()).searchParams.has('timetableVersionId')) {
			socket.send(JSON.stringify({ type: 'StateSync', payload: { users: [], current_seq: 0 } }));
			return;
		}
		const versionId = new URL(socket.url()).searchParams.get('timetableVersionId') ?? undefined;
		const selectedWorkspace = workspace(versionId);
		let scopedRelay = rooms.get(selectedWorkspace.version.id);
		if (!scopedRelay) {
			scopedRelay = createDraftRelay();
			rooms.set(selectedWorkspace.version.id, scopedRelay);
		}
		const peer = scopedRelay.join(
			selectedWorkspace,
			(event) => socket.send(JSON.stringify(event)),
			identity
		);
		socket.onMessage((data) => peer.receive(JSON.parse(String(data)) as DraftClientMessage));
		socket.onClose(() => peer.close());
	});
}

/** Real RFC6455 transport used by two independent browser contexts. */
export async function startNativeDraftRelay(
	relay: DraftRelay,
	workspace: () => TimetableBlockWorkspace
) {
	const sockets = new Set<Duplex>();
	const policyClosers = new Map<string, () => void>();
	const server = createServer();
	server.on('upgrade', (request, socket, head) => {
		const key = request.headers['sec-websocket-key'];
		if (typeof key !== 'string') {
			socket.destroy();
			return;
		}
		socket.write(
			'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ' +
				createHash('sha1')
					.update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11')
					.digest('base64') +
				'\r\n\r\n'
		);
		sockets.add(socket);
		function frame(opcode: number, payload: Buffer) {
			const header =
				payload.length < 126
					? Buffer.from([0x80 | opcode, payload.length])
					: payload.length < 65536
						? Buffer.from([0x80 | opcode, 126, payload.length >> 8, payload.length & 255])
						: Buffer.from([
								0x80 | opcode,
								127,
								0,
								0,
								0,
								0,
								(payload.length >>> 24) & 255,
								(payload.length >>> 16) & 255,
								(payload.length >>> 8) & 255,
								payload.length & 255
							]);
			if (!socket.destroyed && !socket.writableEnded)
				socket.write(Buffer.concat([header, payload]));
		}
		const url = new URL(request.url ?? '/', 'http://localhost');
		const peer = relay.join(workspace(), (event) => frame(1, Buffer.from(JSON.stringify(event))), {
			userId: url.searchParams.get('fixtureUser') ?? '51000000-0000-4000-8000-000000000211',
			name: url.searchParams.get('fixtureName') ?? 'ผู้ร่วมจัดตาราง'
		});
		policyClosers.set(peer.id, () => {
			const payload = Buffer.alloc(2);
			payload.writeUInt16BE(1008);
			frame(8, payload);
			socket.end();
		});
		let buffered = head;
		function read(data: Buffer) {
			buffered = Buffer.concat([buffered, data]);
			while (buffered.length >= 2) {
				const opcode = buffered[0] & 15;
				let length = buffered[1] & 127;
				let offset = 2;
				if (length === 126) {
					if (buffered.length < 4) return;
					length = buffered.readUInt16BE(2);
					offset = 4;
				} else if (length === 127) {
					if (buffered.length < 10) return;
					const large = buffered.readBigUInt64BE(2);
					if (large > 1048576n) {
						socket.destroy();
						return;
					}
					length = Number(large);
					offset = 10;
				}
				const masked = Boolean(buffered[1] & 128);
				if (!masked) {
					socket.destroy();
					return;
				}
				if (buffered.length < offset + 4 + length) return;
				const mask = buffered.subarray(offset, offset + 4);
				offset += 4;
				const payload = Buffer.from(buffered.subarray(offset, offset + length));
				for (let index = 0; index < payload.length; index++) payload[index] ^= mask[index % 4];
				buffered = buffered.subarray(offset + length);
				if (opcode === 8) {
					frame(8, payload);
					socket.end();
					return;
				}
				if (opcode === 9) {
					frame(10, payload);
					continue;
				}
				if (opcode === 1) peer.receive(JSON.parse(payload.toString()) as DraftClientMessage);
			}
		}
		socket.on('error', () => socket.destroy());
		socket.on('data', read);
		socket.on('close', () => {
			sockets.delete(socket);
			policyClosers.delete(peer.id);
			peer.close();
		});
		if (head.length) read(Buffer.alloc(0));
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = server.address();
	if (!address || typeof address === 'string') throw new Error('Missing relay address');
	return {
		url: `ws://127.0.0.1:${address.port}`,
		policyClose(connectionId: string) {
			const close = policyClosers.get(connectionId);
			if (!close) throw new Error('Missing controlled policy connection');
			close();
		},
		async close() {
			for (const socket of sockets) socket.destroy();
			await new Promise<void>((resolve, reject) =>
				server.close((error) => (error ? reject(error) : resolve()))
			);
		}
	};
}
