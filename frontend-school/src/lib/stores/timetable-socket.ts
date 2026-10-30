import { browserVisibilityDependencies } from '#lib/realtime/visibility-idle.js';
import { writable, type Writable } from 'svelte/store';
import { BACKEND_WS_URL, getSchoolSubdomainHint } from '#lib/api/client.js';
import { realtimeAuthRecovery } from '#lib/realtime/auth-recovery.js';
import {
	createDraftCollaborationSession,
	type DraftSessionState,
	type OwnedDraftLease
} from '#lib/academic/timetable/collaboration-session.js';
import type {
	DraftEventEnvelope,
	DraftReservationPurpose,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource
} from '#lib/api/timetable.js';
import { decodeDraftEvent } from '#lib/academic/timetable/collaboration-decoder.js';
import {
	createTimetableSocketRuntime,
	type TimetableSocketParams
} from '#lib/utils/timetable-socket-runtime.js';

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
		timetable_version_id: string;
		block_id?: string | null;
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
	AcademicCoreChangedEvent | LearningDeliveryChangedEvent | TimetableChangedEvent;

export const activeUsers: Writable<UserPresence[]> = writable([]);
export const remoteCursors: Writable<
	Record<string, { x: number; y: number; context?: UserContext }>
> = writable({});
export const refreshTrigger: Writable<number> = writable(0);
export const isConnected: Writable<boolean> = writable(false);
export const draftSessionState = writable<DraftSessionState>({
	versionId: null,
	connectionId: null,
	epoch: null,
	seq: 0,
	ready: false,
	users: [],
	reservations: [],
	availability: null,
	leaseRevision: 0
});
export const draftReconcileSignal = writable<{
	versionId: string;
	reason: string;
	revision: number;
} | null>(null);
const draftEventListeners = new Set<(event: DraftEventEnvelope) => void>();
const draftLeaseLostListeners = new Set<(lease: OwnedDraftLease, error: Error) => void>();
let currentSocketParams: TimetableSocketParams | null = null;
let invalidDraftMessages = 0;

const draftSession = createDraftCollaborationSession({
	send: (message) => timetableSocketRuntime.send(JSON.stringify(message)),
	uuid: () => crypto.randomUUID(),
	setTimer: (callback, delay) => setTimeout(callback, delay),
	clearTimer: (timer) => clearTimeout(timer),
	onState: (state) => draftSessionState.set(state),
	onEvent: (event) => {
		if (event.type === 'DraftStateSync') needsResumeReconcile = false;
		for (const listener of draftEventListeners) listener(event);
	},
	onReconcile: (reason) => {
		const params = currentSocketParams;
		if (!params?.timetableVersionId) return;
		draftSession.reset(params.timetableVersionId);
		timetableSocketRuntime.disconnect();
		timetableSocketRuntime.connect(params);
		draftReconcileSignal.update((previous) => ({
			versionId: params.timetableVersionId!,
			reason,
			revision: (previous?.revision ?? 0) + 1
		}));
	},
	onLeaseLost: (lease, error) => {
		for (const listener of draftLeaseLostListeners) listener(lease, error);
	}
});

export async function claimDraftLease(
	versionId: string,
	source: TimetableBlockPlacementSource,
	candidate: TimetableBlockPlacementCandidate,
	purpose: DraftReservationPurpose,
	signal?: AbortSignal
): Promise<OwnedDraftLease> {
	if (draftSession.state.versionId && draftSession.state.versionId !== versionId)
		throw new Error('รุ่นตารางสอนเปลี่ยนไป กรุณาเลือกคาบใหม่');
	if (!draftSession.state.ready || draftSession.state.versionId !== versionId) {
		await new Promise<void>((resolve, reject) => {
			let finished = false;
			let unsubscribe = () => {};
			const timer = setTimeout(() => finish(false), 4000);
			function finish(ready: boolean) {
				if (finished) return;
				finished = true;
				clearTimeout(timer);
				unsubscribe();
				signal?.removeEventListener('abort', abort);
				if (ready) resolve();
				else reject(new Error('ยังเชื่อมต่อผู้ร่วมจัดตารางไม่ได้ กรุณาลองใหม่'));
			}
			function abort() {
				finish(false);
			}
			if (signal?.aborted) {
				finish(false);
				return;
			}
			signal?.addEventListener('abort', abort, { once: true });
			unsubscribe = draftSessionState.subscribe((state) => {
				if (state.versionId === versionId && state.ready) finish(true);
				else if (state.versionId && state.versionId !== versionId) finish(false);
			});
		});
	}
	return draftSession.claim(source, candidate, purpose, signal);
}
export const targetDraftLease = draftSession.target;
export const releaseDraftLease = draftSession.release;
export const ownsDraftLease = draftSession.owns;
export const draftMutationIdentity = draftSession.mutation;
export const sendDraftPresence = draftSession.presence;
export type { OwnedDraftLease };

export function subscribeDraftEvents(listener: (event: DraftEventEnvelope) => void) {
	draftEventListeners.add(listener);
	return () => {
		draftEventListeners.delete(listener);
	};
}
export function subscribeDraftLeaseLost(listener: (lease: OwnedDraftLease, error: Error) => void) {
	draftLeaseLostListeners.add(listener);
	return () => {
		draftLeaseLostListeners.delete(listener);
	};
}

let currentUserId: string | null = null;
let currentAcademicTermId: string | null = null;
let lastSeq = 0;
let needsResumeReconcile = false;

export function getLastSeq(): number {
	return lastSeq;
}

function clearRealtimeState() {
	isConnected.set(false);
	activeUsers.set([]);
	remoteCursors.set({});
	draftSession.reset(currentSocketParams?.timetableVersionId ?? null);
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
	if (needsResumeReconcile || currentSeq !== lastSeq) triggerReconcile(currentSeq);
	needsResumeReconcile = false;
}

function handleMessage(event: SequencedTimetableEvent | DraftEventEnvelope) {
	if ('epoch' in event) {
		draftSession.apply(event);
		return;
	}
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
		const { authAPI } = await import('#lib/api/auth.js');
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
		if (params.timetableVersionId) {
			url.searchParams.set('timetableVersionId', params.timetableVersionId);
		}
		const schoolSubdomain = getSchoolSubdomainHint();
		if (schoolSubdomain) url.searchParams.set('school_subdomain', schoolSubdomain);
		return new WebSocket(url);
	},
	setTimer: (callback, delay) => setTimeout(callback, delay),
	clearTimer: (timer) => clearTimeout(timer),
	isOnline: () => navigator.onLine,
	addOnlineListener: (listener) => window.addEventListener('online', listener),
	removeOnlineListener: (listener) => window.removeEventListener('online', listener),
	visibility: browserVisibilityDependencies(),
	onPause: () => {
		needsResumeReconcile = true;
		clearRealtimeState();
	},
	onResume: async (isCurrent) => {
		const { authAPI } = await import('#lib/api/auth.js');
		if (!isCurrent()) return false;
		const result = await authAPI.refreshCurrentUser({ silent: true });
		if (!isCurrent()) return false;
		if (result === 'unauthenticated') disconnectTimetableSocket();
		if (result !== 'authenticated') return false;
		return true;
	},
	onOpen: () => isConnected.set(true),
	onMessage: (data) => {
		if (currentSocketParams?.timetableVersionId) {
			const event = decodeDraftEvent(data);
			if (event) {
				draftSession.apply(event);
				return;
			}
			const params = currentSocketParams;
			invalidDraftMessages++;
			timetableSocketRuntime.disconnect();
			clearRealtimeState();
			if (invalidDraftMessages === 1) timetableSocketRuntime.connect(params);
			return;
		}
		try {
			handleMessage(JSON.parse(String(data)) as SequencedTimetableEvent);
		} catch {
			console.error('Failed to parse timetable realtime event');
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
	if (
		currentAcademicTermId !== params.academicTermId ||
		currentUserId !== params.currentUserId ||
		currentSocketParams?.timetableVersionId !== params.timetableVersionId
	) {
		lastSeq = 0;
		invalidDraftMessages = 0;
		currentSocketParams = { ...params };
		clearRealtimeState();
	}
	currentSocketParams = { ...params };
	timetableSocketRuntime.connect(params);
}

export function disconnectTimetableSocket() {
	needsResumeReconcile = false;
	currentUserId = null;
	currentAcademicTermId = null;
	lastSeq = 0;
	currentSocketParams = null;
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
