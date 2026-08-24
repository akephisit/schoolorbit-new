import { writable, type Writable } from 'svelte/store';
import { BACKEND_WS_URL, getSchoolSubdomainHint } from '$lib/api/client';
import { realtimeAuthRecovery } from '$lib/realtime/auth-recovery';
import {
	createTimetableSocketRuntime,
	type TimetableSocketParams
} from '$lib/utils/timetable-socket-runtime';

export interface UserContext {
	view_mode: string;
	view_id?: string;
}

export interface UserPresence {
	user_id: string;
	name: string;
	color: string;
	context?: UserContext;
}

type StateSyncEvent = {
	type: 'StateSync';
	payload: { users: UserPresence[]; current_seq: number };
};

type UserJoinedEvent = { type: 'UserJoined'; payload: UserPresence };
type UserLeftEvent = { type: 'UserLeft'; payload: { user_id: string } };
type CursorMoveEvent = {
	type: 'CursorMove';
	payload: { user_id: string; x: number; y: number; context?: UserContext };
};
type AcademicCoreChangedEvent = {
	type: 'AcademicCoreChanged';
	payload: {
		user_id: string;
		entity_type: string;
		entity_id?: string | null;
		academic_year_id?: string | null;
		academic_term_id?: string | null;
	};
};
type LearningDeliveryChangedEvent = {
	type: 'LearningDeliveryChanged';
	payload: {
		user_id: string;
		academic_term_id: string;
		learning_offering_id: string;
		learning_group_id?: string | null;
		revision: number;
	};
};
type TimetableChangedEvent = {
	type: 'TimetableChanged';
	payload: {
		user_id: string;
		academic_term_id: string;
		learning_group_id?: string | null;
		revision: number;
	};
};

export type TimetableEvent =
	| StateSyncEvent
	| UserJoinedEvent
	| UserLeftEvent
	| CursorMoveEvent
	| AcademicCoreChangedEvent
	| LearningDeliveryChangedEvent
	| TimetableChangedEvent;

type SequencedTimetableEvent = TimetableEvent & { seq?: number };
type MutationEvent =
	| AcademicCoreChangedEvent
	| LearningDeliveryChangedEvent
	| TimetableChangedEvent;

export const activeUsers: Writable<UserPresence[]> = writable([]);
export const remoteCursors: Writable<
	Record<string, { x: number; y: number; context?: UserContext }>
> = writable({});
export const refreshTrigger: Writable<number> = writable(0);
export const isConnected: Writable<boolean> = writable(false);

let currentUserId: string | null = null;
let currentAcademicTermId: string | null = null;
let lastSeq = 0;

export function getLastSeq(): number {
	return lastSeq;
}

function clearRealtimeState() {
	isConnected.set(false);
	activeUsers.set([]);
	remoteCursors.set({});
}

function triggerReconcile(currentSeq?: number) {
	if (typeof currentSeq === 'number') lastSeq = currentSeq;
	refreshTrigger.update((count) => count + 1);
}

function isMutationEvent(
	event: SequencedTimetableEvent
): event is MutationEvent & { seq?: number } {
	return (
		event.type === 'AcademicCoreChanged' ||
		event.type === 'LearningDeliveryChanged' ||
		event.type === 'TimetableChanged'
	);
}

function mutationMatchesSelectedTerm(event: MutationEvent): boolean {
	if (!currentAcademicTermId) return false;
	if (event.type === 'AcademicCoreChanged') {
		return (
			event.payload.academic_term_id == null ||
			event.payload.academic_term_id === currentAcademicTermId
		);
	}
	return event.payload.academic_term_id === currentAcademicTermId;
}

function handleMutation(event: MutationEvent & { seq?: number }) {
	if (!mutationMatchesSelectedTerm(event)) return;
	const seq = event.seq;
	if (typeof seq !== 'number') {
		triggerReconcile();
		return;
	}
	if (seq <= lastSeq) return;
	if (lastSeq > 0 && seq > lastSeq + 1) {
		triggerReconcile(seq);
		return;
	}
	lastSeq = seq;
	triggerReconcile();
}

function handleStateSync(event: StateSyncEvent) {
	activeUsers.set(event.payload.users.filter((user) => user.user_id !== currentUserId));
	const currentSeq = event.payload.current_seq;
	if (currentSeq !== lastSeq) triggerReconcile(currentSeq);
}

function handleMessage(event: SequencedTimetableEvent) {
	if (isMutationEvent(event)) {
		handleMutation(event);
		return;
	}

	switch (event.type) {
		case 'StateSync':
			handleStateSync(event);
			break;
		case 'UserJoined':
			if (event.payload.user_id === currentUserId) return;
			activeUsers.update((users) =>
				users.some((user) => user.user_id === event.payload.user_id)
					? users
					: [...users, event.payload]
			);
			break;
		case 'UserLeft':
			activeUsers.update((users) => users.filter((user) => user.user_id !== event.payload.user_id));
			remoteCursors.update((cursors) => {
				const next = { ...cursors };
				delete next[event.payload.user_id];
				return next;
			});
			break;
		case 'CursorMove':
			if (event.payload.user_id === currentUserId) return;
			activeUsers.update((users) =>
				users.map((user) =>
					user.user_id === event.payload.user_id
						? { ...user, context: event.payload.context }
						: user
				)
			);
			remoteCursors.update((cursors) => ({
				...cursors,
				[event.payload.user_id]: {
					x: event.payload.x,
					y: event.payload.y,
					context: event.payload.context
				}
			}));
			break;
	}
}

async function recoverTimetableAuth() {
	try {
		const { authAPI } = await import('$lib/api/auth');
		await realtimeAuthRecovery(() => authAPI.refreshCurrentUser({ silent: true }));
	} catch (error) {
		console.error('Failed to refresh auth after timetable policy close', error);
	}
}

const timetableSocketRuntime = createTimetableSocketRuntime({
	createSocket: (params) => {
		currentUserId = params.currentUserId;
		currentAcademicTermId = params.academicTermId;
		const url = new URL('/ws/timetable', BACKEND_WS_URL);
		url.searchParams.set('academicTermId', String(params.academicTermId));
		const schoolSubdomain = getSchoolSubdomainHint();
		if (schoolSubdomain) url.searchParams.set('school_subdomain', schoolSubdomain);
		return new WebSocket(url);
	},
	setTimer: (callback, delay) => setTimeout(callback, delay),
	clearTimer: (timer) => clearTimeout(timer),
	isOnline: () => navigator.onLine,
	addOnlineListener: (listener) => window.addEventListener('online', listener),
	removeOnlineListener: (listener) => window.removeEventListener('online', listener),
	onOpen: () => isConnected.set(true),
	onMessage: (data) => {
		try {
			handleMessage(JSON.parse(String(data)) as SequencedTimetableEvent);
		} catch (error) {
			console.error('Failed to parse timetable realtime event', error);
		}
	},
	onClose: (event) => {
		clearRealtimeState();
		if (event.code !== 1008) return;
		void recoverTimetableAuth();
	},
	onError: (error) => console.error('Timetable realtime connection failed', error)
});

export function connectTimetableSocket(params: TimetableSocketParams) {
	if (currentAcademicTermId !== params.academicTermId) {
		lastSeq = 0;
		clearRealtimeState();
	}
	timetableSocketRuntime.connect(params);
}

export function disconnectTimetableSocket() {
	currentUserId = null;
	currentAcademicTermId = null;
	lastSeq = 0;
	timetableSocketRuntime.disconnect();
	clearRealtimeState();
}

export function sendCursorMove(x: number, y: number, context?: UserContext) {
	if (!currentUserId) return false;
	const event: CursorMoveEvent = {
		type: 'CursorMove',
		payload: { user_id: currentUserId, x, y, context }
	};
	return timetableSocketRuntime.send(JSON.stringify(event));
}
