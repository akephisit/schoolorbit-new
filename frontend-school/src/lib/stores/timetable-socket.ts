import { writable, type Writable } from 'svelte/store';
import { PUBLIC_BACKEND_URL } from '$env/static/public';
import type { TimetableEntry } from '$lib/api/timetable';

// Types matching backend
export interface UserContext {
	view_mode: string;
	view_id?: string;
}

export interface DragInfo {
	code: string;
	title: string;
	color?: string;
}

export interface UserPresence {
	user_id: string;
	name: string;
	color: string;
	context?: UserContext;
}

export interface UserActivityState {
	activity_type: string;
	target?: unknown;
}

export type TimetableEvent =
	| {
			type: 'StateSync';
			payload: {
				users: UserPresence[];
				drags: Record<string, { course_id?: string; entry_id?: string; info?: DragInfo }>;
				activities: Record<string, UserActivityState>;
				current_seq: number;
			};
	  }
	| { type: 'TableRefresh'; payload: { user_id: string } }
	| { type: 'UserJoined'; payload: UserPresence }
	| { type: 'UserLeft'; payload: { user_id: string } }
	| {
			type: 'CursorMove';
			payload: { user_id: string; x: number; y: number; context?: UserContext };
	  }
	| {
			type: 'DragStart';
			payload: { user_id: string; course_id?: string; entry_id?: string; info?: DragInfo };
	  }
	| { type: 'DragEnd'; payload: { user_id: string } }
	| {
			type: 'DragMove';
			payload: {
				user_id: string;
				x: number;
				y: number;
				target_day?: string;
				target_period_id?: string;
			};
	  }
	| { type: 'UserActivity'; payload: { user_id: string; activity_type: string; target?: unknown } }
	| { type: 'UserActivityEnd'; payload: { user_id: string } }
	// Patch events (new — seq-tracked)
	| { type: 'EntryCreated'; payload: { user_id: string; entry: TimetableEntry } }
	| { type: 'EntryUpdated'; payload: { user_id: string; entry: TimetableEntry } }
	| { type: 'EntryDeleted'; payload: { user_id: string; entry_id: string } }
	| {
			type: 'EntriesSwapped';
			payload: { user_id: string; entry_a: TimetableEntry; entry_b: TimetableEntry };
	  }
	| {
			type: 'EntryInstructorAdded';
			payload: {
				user_id: string;
				entry_id: string;
				instructor_id: string;
				instructor_name: string;
				role: string;
			};
	  }
	| {
			type: 'EntryInstructorRemoved';
			payload: { user_id: string; entry_id: string; instructor_id: string; entry_deleted: boolean };
	  }
	| { type: 'CourseTeamChanged'; payload: { user_id: string; course_id: string } }
	| {
			type: 'DropIntent';
			payload: {
				user_id: string;
				kind: string;
				entry_id: string;
				day_of_week: string;
				period_id: string;
				room_id?: string | null;
				swap_partner_id?: string | null;
				swap_partner_day?: string | null;
				swap_partner_period_id?: string | null;
			};
	  }
	| {
			type: 'DropRejected';
			payload: {
				user_id: string;
				entry_id: string;
				original_day: string;
				original_period_id: string;
				original_room_id?: string | null;
				partner_id?: string | null;
				partner_original_day?: string | null;
				partner_original_period_id?: string | null;
				reason: string;
			};
	  };

/** Patch events ที่ page subscribe เพื่อ apply ต่อ state — ไม่ fetch DB ซ้ำ */
export type TimetablePatch =
	| { type: 'EntryCreated'; entry: TimetableEntry }
	| { type: 'EntryUpdated'; entry: TimetableEntry }
	| { type: 'EntryDeleted'; entry_id: string }
	| { type: 'EntriesSwapped'; entry_a: TimetableEntry; entry_b: TimetableEntry }
	| {
			type: 'EntryInstructorAdded';
			entry_id: string;
			instructor_id: string;
			instructor_name: string;
			role: string;
	  }
	| {
			type: 'EntryInstructorRemoved';
			entry_id: string;
			instructor_id: string;
			entry_deleted: boolean;
	  }
	| { type: 'CourseTeamChanged'; course_id: string }
	| {
			type: 'DropIntent';
			user_id: string;
			kind: string; // 'move' | 'swap'
			entry_id: string;
			day_of_week: string;
			period_id: string;
			room_id: string | null;
			swap_partner_id: string | null;
			swap_partner_day: string | null;
			swap_partner_period_id: string | null;
	  }
	| {
			type: 'DropRejected';
			user_id: string;
			entry_id: string;
			original_day: string;
			original_period_id: string;
			original_room_id: string | null;
			partner_id: string | null;
			partner_original_day: string | null;
			partner_original_period_id: string | null;
			reason: string;
	  };

// Stores
export const activeUsers: Writable<UserPresence[]> = writable([]);
export const remoteCursors: Writable<
	Record<string, { x: number; y: number; context?: UserContext }>
> = writable({});
// Key: user_id -> What they are dragging
export const userDrags: Writable<
	Record<string, { course_id?: string; entry_id?: string; info?: DragInfo }>
> = writable({});
// Key: user_id -> Current drag position & target cell
export const dragPositions: Writable<
	Record<string, { x: number; y: number; target_day?: string; target_period_id?: string }>
> = writable({});
// Key: user_id -> Current dialog activity (room picker, instructor edit, etc.)
export const remoteActivities: Writable<Record<string, UserActivityState>> = writable({});
export const refreshTrigger: Writable<number> = writable(0);
export const isConnected: Writable<boolean> = writable(false);
/** Patch events ที่ broadcast จาก backend — page subscribe เพื่อ apply ต่อ state
 *  reset เป็น null หลัง apply เพื่อ dedupe */
export const lastPatch: Writable<TimetablePatch | null> = writable(null);

// Seq tracking (ใช้ detect gap + reconcile)
let lastSeq = 0;
let reconcileInFlight = false;

export function setInitialSeq(seq: number) {
	lastSeq = seq;
}

export function getLastSeq(): number {
	return lastSeq;
}

/** Force reconcile: fetch replay หรือ full-fetch, apply, update lastSeq */
async function triggerReconcile(semesterId: string) {
	if (reconcileInFlight) return;
	reconcileInFlight = true;
	try {
		const baseUrl = PUBLIC_BACKEND_URL || 'http://localhost:8081';
		const url = `${baseUrl}/api/academic/timetable/replay?semester_id=${semesterId}&after_seq=${lastSeq}`;
		const res = await fetch(url, { credentials: 'include' }).catch(() => null);
		if (!res || !res.ok) {
			// ล้มเหลว → fallback full-fetch ผ่าน refreshTrigger
			refreshTrigger.update((n) => n + 1);
			return;
		}
		const data = await res.json();
		if (data.needs_refetch) {
			// Buffer หมด → full-fetch
			lastSeq = data.current_seq ?? 0;
			refreshTrigger.update((n) => n + 1);
		} else {
			// Apply events ตามลำดับ
			for (const seqEvent of data.events ?? []) {
				applyPatchFromSeqEvent(seqEvent);
			}
			lastSeq = data.current_seq ?? lastSeq;
		}
	} finally {
		reconcileInFlight = false;
	}
}

interface RawMessagePayload {
	// StateSync
	users?: UserPresence[];
	drags?: Record<string, { course_id?: string; entry_id?: string; info?: DragInfo }>;
	activities?: Record<string, UserActivityState>;
	current_seq?: number;
	// User events
	user_id?: string;
	name?: string;
	color?: string;
	// Activity
	activity_type?: string;
	target?: unknown;
	// Cursor
	x?: number;
	y?: number;
	context?: UserContext;
	// Drag
	course_id?: string;
	entry_id?: string;
	info?: DragInfo;
	target_day?: string;
	target_period_id?: string;
	// Patch events
	entry?: TimetableEntry;
	entry_a?: TimetableEntry;
	entry_b?: TimetableEntry;
	instructor_id?: string;
	instructor_name?: string;
	role?: string;
	entry_deleted?: boolean;
	// Phase 2 — DropIntent / DropRejected
	kind?: string;
	day_of_week?: string;
	period_id?: string;
	room_id?: string | null;
	swap_partner_id?: string | null;
	swap_partner_day?: string | null;
	swap_partner_period_id?: string | null;
	original_day?: string;
	original_period_id?: string;
	original_room_id?: string | null;
	partner_id?: string | null;
	partner_original_day?: string | null;
	partner_original_period_id?: string | null;
	reason?: string;
}

interface SeqEvent {
	type: string;
	payload: RawMessagePayload;
	seq?: number;
}

function applyPatchFromSeqEvent(seqEvent: SeqEvent) {
	const { type, payload } = seqEvent;
	if (seqEvent.seq !== undefined && seqEvent.seq !== null) {
		lastSeq = Math.max(lastSeq, seqEvent.seq);
	}
	switch (type) {
		case 'TableRefresh':
			refreshTrigger.update((n) => n + 1);
			break;
		case 'EntryCreated':
			if (payload.entry) lastPatch.set({ type: 'EntryCreated', entry: payload.entry });
			break;
		case 'EntryUpdated':
			if (payload.entry) lastPatch.set({ type: 'EntryUpdated', entry: payload.entry });
			break;
		case 'EntryDeleted':
			if (payload.entry_id) lastPatch.set({ type: 'EntryDeleted', entry_id: payload.entry_id });
			break;
		case 'EntriesSwapped':
			if (payload.entry_a && payload.entry_b)
				lastPatch.set({
					type: 'EntriesSwapped',
					entry_a: payload.entry_a,
					entry_b: payload.entry_b
				});
			break;
		case 'EntryInstructorAdded':
			if (payload.entry_id && payload.instructor_id && payload.instructor_name && payload.role) {
				lastPatch.set({
					type: 'EntryInstructorAdded',
					entry_id: payload.entry_id,
					instructor_id: payload.instructor_id,
					instructor_name: payload.instructor_name,
					role: payload.role
				});
			}
			break;
		case 'EntryInstructorRemoved':
			if (payload.entry_id && payload.instructor_id && payload.entry_deleted !== undefined) {
				lastPatch.set({
					type: 'EntryInstructorRemoved',
					entry_id: payload.entry_id,
					instructor_id: payload.instructor_id,
					entry_deleted: payload.entry_deleted
				});
			}
			break;
		case 'CourseTeamChanged':
			if (payload.course_id)
				lastPatch.set({ type: 'CourseTeamChanged', course_id: payload.course_id });
			break;
		case 'DropIntent':
			if (payload.user_id && payload.entry_id && payload.day_of_week && payload.period_id) {
				lastPatch.set({
					type: 'DropIntent',
					user_id: payload.user_id,
					kind: payload.kind ?? 'move',
					entry_id: payload.entry_id,
					day_of_week: payload.day_of_week,
					period_id: payload.period_id,
					room_id: payload.room_id ?? null,
					swap_partner_id: payload.swap_partner_id ?? null,
					swap_partner_day: payload.swap_partner_day ?? null,
					swap_partner_period_id: payload.swap_partner_period_id ?? null
				});
			}
			break;
		case 'DropRejected':
			if (
				payload.user_id &&
				payload.entry_id &&
				payload.original_day &&
				payload.original_period_id
			) {
				lastPatch.set({
					type: 'DropRejected',
					user_id: payload.user_id,
					entry_id: payload.entry_id,
					original_day: payload.original_day,
					original_period_id: payload.original_period_id,
					original_room_id: payload.original_room_id ?? null,
					partner_id: payload.partner_id ?? null,
					partner_original_day: payload.partner_original_day ?? null,
					partner_original_period_id: payload.partner_original_period_id ?? null,
					reason: payload.reason ?? ''
				});
			}
			break;
	}
}

let socket: WebSocket | null = null;
let currentUserId: string | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let connectionDebounceTimer: ReturnType<typeof setTimeout> | null = null; // New timer for debouncing initial connection
let shouldReconnect = false;
let lastParams: { semester_id: string; user_id: string; name: string } | null = null;

export function connectTimetableSocket(params: {
	semester_id: string;
	user_id: string;
	name: string;
}) {
	// Check duplicate connection (Immediate check)
	if (
		socket &&
		(socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)
	) {
		if (
			lastParams &&
			String(lastParams.semester_id) === String(params.semester_id) &&
			String(lastParams.user_id) === String(params.user_id)
		) {
			// Same params, already connected/connecting
			// If there is a pending debounce for a NEW connection, we should probably clear it because we are happy with current?
			// But if we are here, socket exists.
			if (connectionDebounceTimer) clearTimeout(connectionDebounceTimer);
			return;
		}
	}

	// Clear any pending reconnect or debounce
	if (reconnectTimer) clearTimeout(reconnectTimer);
	if (connectionDebounceTimer) clearTimeout(connectionDebounceTimer);

	shouldReconnect = true;
	lastParams = params;

	// Debounce the actual connection creation by 50ms
	// This allows rapid destroy/create cycles (e.g. quick navigation) to cancel out
	// before the socket is actually created.
	connectionDebounceTimer = setTimeout(() => {
		if (!shouldReconnect) return; // If disconnected in the meantime

		if (socket) {
			// Prevent old socket from triggering reconnect
			socket.onclose = null;
			socket.onerror = null;
			socket.close();
		}
		currentUserId = params.user_id;

		const baseUrl = PUBLIC_BACKEND_URL || 'http://localhost:8081';
		const wsUrl = baseUrl.replace(/^http/, 'ws');

		// Auto-detect school_key from hostname
		let schoolKey = 'default';
		if (typeof window !== 'undefined') {
			const parts = window.location.hostname.split('.');
			if (parts.length >= 3) {
				schoolKey = parts[0];
			}
		}

		// Ensure semester_id/user_id are strings
		const safeParams = {
			...params,
			semester_id: String(params.semester_id),
			user_id: String(params.user_id),
			school_key: schoolKey
		};

		const qs = new URLSearchParams(safeParams).toString();
		const url = `${wsUrl}/ws/timetable?${qs}`;

		console.log('Connecting to WS:', url);
		socket = new WebSocket(url);

		socket.onopen = () => {
			console.log('WS Connected');
			isConnected.set(true);
			// Clear reconnect timer if any (redundant but safe)
			if (reconnectTimer) clearTimeout(reconnectTimer);
		};

		socket.onmessage = (event) => {
			try {
				const msg = JSON.parse(event.data);
				handleMessage(msg);
			} catch (e) {
				console.error('WS Parse Error', e);
			}
		};

		socket.onclose = () => {
			console.log('WS Disconnected');
			isConnected.set(false);
			// Clear state
			activeUsers.set([]);
			remoteCursors.set({});
			userDrags.set({});
			dragPositions.set({});
			remoteActivities.set({});

			// Auto Reconnect
			if (shouldReconnect) {
				console.log('Attempting to reconnect in 3s...');
				reconnectTimer = setTimeout(() => {
					if (shouldReconnect && lastParams) {
						connectTimetableSocket(lastParams);
					}
				}, 3000);
			}
		};

		socket.onerror = (err) => {
			console.error('WS Error', err);
			// On error, onclose usually fires too, but just in case
		};
	}, 50); // 50ms delay
}

export function disconnectTimetableSocket() {
	shouldReconnect = false;
	if (reconnectTimer) clearTimeout(reconnectTimer);
	if (connectionDebounceTimer) clearTimeout(connectionDebounceTimer); // Cancel pending connection

	if (socket) {
		// Prevent any pending callbacks
		socket.onclose = null;
		socket.onerror = null;
		socket.close();
		socket = null;
	}
}

export function sendTimetableEvent(event: TimetableEvent) {
	if (socket && socket.readyState === WebSocket.OPEN) {
		socket.send(JSON.stringify(event));
	}
}

export function sendUserActivity(activityType: string, target?: unknown) {
	if (!currentUserId) return;
	sendTimetableEvent({
		type: 'UserActivity',
		payload: { user_id: currentUserId, activity_type: activityType, target }
	});
}

export function sendUserActivityEnd() {
	if (!currentUserId) return;
	sendTimetableEvent({
		type: 'UserActivityEnd',
		payload: { user_id: currentUserId }
	});
}

/** Phase 2: broadcast optimistic drop intent — server relays to other clients
 *  so they apply the same optimistic mutation before DB confirms. */
export function sendDropIntent(payload: {
	kind: 'move' | 'swap';
	entry_id: string;
	day_of_week: string;
	period_id: string;
	room_id?: string | null;
	swap_partner_id?: string | null;
	swap_partner_day?: string | null;
	swap_partner_period_id?: string | null;
}) {
	if (!currentUserId) return;
	sendTimetableEvent({
		type: 'DropIntent',
		payload: {
			user_id: currentUserId,
			...payload
		}
	});
}

function handleMessage(msg: SeqEvent & { seq?: number }) {
	const { type, payload, seq } = msg;

	// Patch events: เช็ค seq + gap detection
	const isMutation = [
		'TableRefresh',
		'EntryCreated',
		'EntryUpdated',
		'EntryDeleted',
		'EntriesSwapped',
		'EntryInstructorAdded',
		'EntryInstructorRemoved',
		'CourseTeamChanged'
	].includes(type);

	if (isMutation && typeof seq === 'number') {
		if (seq <= lastSeq) {
			// Duplicate หรือ out-of-order เก่า — ignore
			return;
		}
		if (seq > lastSeq + 1 && lastSeq > 0) {
			// Gap detected — reconcile
			const semId = lastParams?.semester_id;
			if (semId) triggerReconcile(semId);
			return;
		}
		// Sequential — apply
		applyPatchFromSeqEvent(msg);
		return;
	}

	switch (type) {
		case 'StateSync': {
			const { users = [], drags = {}, activities = {}, current_seq } = payload;
			// Filter out self
			const others = users.filter((u: UserPresence) => u.user_id !== currentUserId);
			activeUsers.set(others);

			// Sync drags (filter self if necessary, but usually drag store is by user_id ok)
			if (currentUserId && drags[currentUserId]) {
				delete drags[currentUserId];
			}
			userDrags.set(drags);

			// Sync activities
			const otherActivities = { ...activities };
			if (currentUserId) delete otherActivities[currentUserId];
			remoteActivities.set(otherActivities);

			// Seq reconciliation — handle restart/reconnect
			if (typeof current_seq === 'number') {
				if (current_seq < lastSeq) {
					// Server restart detected — seq counter reset ลง → full reset
					console.log('[WS] Server restart detected (seq reset):', lastSeq, '->', current_seq);
					lastSeq = current_seq;
					refreshTrigger.update((n) => n + 1);
				} else if (current_seq > lastSeq) {
					// Gap — events หายช่วง disconnect หรือระหว่าง API→WS → replay
					const semId = lastParams?.semester_id;
					if (semId) {
						console.log('[WS] Gap detected on StateSync:', lastSeq, '->', current_seq);
						triggerReconcile(semId);
					} else {
						lastSeq = current_seq;
					}
				}
				// current_seq === lastSeq → no-op
			}
			break;
		}
		case 'UserJoined': {
			const user = payload as UserPresence;
			if (user.user_id === currentUserId) return; // Ignore reflection if any

			// Add if not exists
			activeUsers.update((users) => {
				if (users.find((u) => u.user_id === user.user_id)) return users;
				return [...users, user];
			});
			break;
		}
		case 'UserLeft': {
			const { user_id } = payload;
			if (!user_id) return;
			activeUsers.update((users) => users.filter((u) => u.user_id !== user_id));

			// Allow cleanup of cursor/drag/activity
			remoteCursors.update((cursors) => {
				const newCursors = { ...cursors };
				delete newCursors[user_id];
				return newCursors;
			});
			userDrags.update((drags) => {
				const newDrags = { ...drags };
				delete newDrags[user_id];
				return newDrags;
			});
			dragPositions.update((pos) => {
				const newPos = { ...pos };
				delete newPos[user_id];
				return newPos;
			});
			remoteActivities.update((acts) => {
				const next = { ...acts };
				delete next[user_id];
				return next;
			});
			break;
		}
		case 'UserActivity': {
			const { user_id, activity_type, target } = payload;
			if (!user_id || user_id === currentUserId) return;
			if (!activity_type) return;
			remoteActivities.update((acts) => ({
				...acts,
				[user_id]: { activity_type, target }
			}));
			break;
		}
		case 'UserActivityEnd': {
			const { user_id } = payload;
			if (!user_id || user_id === currentUserId) return;
			remoteActivities.update((acts) => {
				const next = { ...acts };
				delete next[user_id];
				return next;
			});
			break;
		}
		case 'CursorMove': {
			const { user_id, x, y, context } = payload;
			if (!user_id || user_id === currentUserId) return;
			if (typeof x !== 'number' || typeof y !== 'number') return;

			// Update user context in activeUsers list too?
			activeUsers.update((users) =>
				users.map((u) => (u.user_id === user_id ? { ...u, context } : u))
			);

			remoteCursors.update((cursors) => ({
				...cursors,
				[user_id]: { x, y, context }
			}));
			break;
		}
		case 'DragStart': {
			const { user_id, course_id, entry_id, info } = payload;
			if (!user_id || user_id === currentUserId) return;

			userDrags.update((drags) => ({
				...drags,
				[user_id]: { course_id, entry_id, info }
			}));
			break;
		}
		case 'DragMove': {
			const { user_id, x, y, target_day, target_period_id } = payload;
			if (!user_id || user_id === currentUserId) return;
			if (typeof x !== 'number' || typeof y !== 'number') return;

			// Update cursor position during drag
			remoteCursors.update((cursors) => ({
				...cursors,
				[user_id]: { ...cursors[user_id], x, y }
			}));

			dragPositions.update((pos) => ({
				...pos,
				[user_id]: { x, y, target_day, target_period_id }
			}));
			break;
		}
		case 'DragEnd': {
			const { user_id } = payload;
			if (!user_id || user_id === currentUserId) return;

			userDrags.update((drags) => {
				const newDrags = { ...drags };
				delete newDrags[user_id];
				return newDrags;
			});
			dragPositions.update((pos) => {
				const newPos = { ...pos };
				delete newPos[user_id];
				return newPos;
			});
			break;
		}
		case 'DropIntent':
		case 'DropRejected':
			// Phase 2 ephemeral — page subscribes via lastPatch
			applyPatchFromSeqEvent(msg);
			break;
		// TableRefresh + patch events จัดการใน isMutation branch ด้านบน
	}
}
