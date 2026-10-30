import type { components } from '../api/generated/school-api.js';

export type SessionState = components['schemas']['SessionState'];
export const ACTIVITY_WINDOW_MS = 5 * 60_000;
const WARNING_WINDOW_MS = 4 * 60_000;

export type ActivityResult =
	| { kind: 'renewed'; session: SessionState }
	| { kind: 'retry'; retryAfterMs?: number }
	| { kind: 'reconciled' }
	| { kind: 'unauthenticated' };

export interface SessionWarning {
	kind: 'idle' | 'absolute';
	remainingMinutes: number;
	pending: boolean;
	unavailable: boolean;
}

interface ActivityDependencies {
	now(): number;
	wallNow(): number;
	setTimer(callback: () => void, delay: number): ReturnType<typeof setTimeout>;
	clearTimer(timer: ReturnType<typeof setTimeout>): void;
	renew(session: SessionState, signal: AbortSignal): Promise<ActivityResult>;
	resyncClock(): Promise<boolean>;
	accept(session: SessionState): void;
	warning(warning: SessionWarning | null): void;
}

/** Timing and input policy only. The auth store owns the authoritative session snapshot. */
export function createSessionActivityController(deps: ActivityDependencies) {
	let session: SessionState | null = null;
	let anchor = 0;
	let serverAnchor = 0;
	let epoch = 0;
	let inputSequence = 0;
	let acknowledgedSequence = 0;
	let lastInput = -Infinity;
	let visible = false;
	let online = true;
	let disposed = false;
	let pending: AbortController | null = null;
	let timer: ReturnType<typeof setTimeout> | null = null;
	let failures = 0;
	let retryAt = 0;
	let unavailable = false;
	let clockRecheckPending = false;
	let clockUncertain = false;
	let clockRecheckNeeded = false;
	let wallReference = deps.wallNow();
	let monotonicReference = deps.now();

	const serverNow = () => serverAnchor + Math.max(0, deps.now() - anchor);
	const stopTimer = () => {
		if (timer !== null) deps.clearTimer(timer);
		timer = null;
	};
	const freshInput = () =>
		inputSequence > acknowledgedSequence && deps.now() - lastInput <= ACTIVITY_WINDOW_MS;

	function renderWarning() {
		if (!session || !visible) {
			deps.warning(null);
			return;
		}
		const idle = Date.parse(session.idleExpiresAt);
		const absolute = Date.parse(session.absoluteExpiresAt);
		const remaining = Math.min(idle, absolute) - serverNow();
		deps.warning(
			remaining <= WARNING_WINDOW_MS
				? {
						kind: absolute <= idle ? 'absolute' : 'idle',
						remainingMinutes: Math.max(0, Math.ceil(remaining / 60_000)),
						pending: pending !== null,
						unavailable
					}
				: null
		);
	}

	function schedule() {
		stopTimer();
		renderWarning();
		if (disposed || !session || !visible) return;
		const now = serverNow();
		const expiry = Math.min(
			Date.parse(session.idleExpiresAt),
			Date.parse(session.absoluteExpiresAt)
		);
		const delays: number[] = [];
		if (expiry > now) {
			delays.push(expiry - now > WARNING_WINDOW_MS ? expiry - now - WARNING_WINDOW_MS : 60_000);
		}
		if (!pending && !clockRecheckPending && online && freshInput()) {
			delays.push(
				Math.max(
					0,
					clockUncertain ? 0 : Date.parse(session.nextRenewalAt) - now,
					retryAt - deps.now()
				)
			);
		}
		if (!delays.length) return;
		timer = deps.setTimer(
			() => {
				timer = null;
				void tick();
			},
			Math.max(1, Math.min(...delays))
		);
	}

	async function tick() {
		// A resumed timeout can be the first callback after sleep, with no focus/pageshow event.
		if (!disposed && session && visible && online) checkClock();
		if (
			disposed ||
			!session ||
			!visible ||
			!online ||
			pending ||
			clockRecheckPending ||
			!freshInput()
		) {
			schedule();
			return;
		}
		if (
			(!clockUncertain && serverNow() < Date.parse(session.nextRenewalAt)) ||
			deps.now() < retryAt
		) {
			schedule();
			return;
		}
		const owner = epoch;
		const expectedId = session.sessionId;
		const capturedInput = inputSequence;
		const operation = new AbortController();
		pending = operation;
		renderWarning();
		let result: ActivityResult;
		try {
			result = await deps.renew(session, operation.signal);
		} catch {
			result = { kind: 'retry' };
		}
		if (
			disposed ||
			owner !== epoch ||
			operation.signal.aborted ||
			session?.sessionId !== expectedId
		)
			return;
		pending = null;
		if (result.kind === 'renewed' && result.session.sessionId === expectedId) {
			acknowledgedSequence = capturedInput;
			failures = 0;
			retryAt = 0;
			unavailable = false;
			clockUncertain = false;
			deps.accept(result.session);
		} else if (result.kind === 'retry') {
			failures++;
			unavailable = true;
			retryAt =
				deps.now() +
				Math.max(
					result.retryAfterMs ?? 0,
					Math.min(60_000, 5_000 * 2 ** Math.min(failures - 1, 4))
				);
		} else {
			acknowledgedSequence = capturedInput;
			retryAt = deps.now() + ACTIVITY_WINDOW_MS;
		}
		schedule();
	}

	function checkClock() {
		const now = deps.now();
		const wall = deps.wallNow();
		const gap = Math.abs(wall - wallReference - (now - monotonicReference)) > 30_000;
		wallReference = wall;
		monotonicReference = now;
		if (gap) clockRecheckNeeded = true;
		if (!clockRecheckNeeded || !session || disposed) return;
		acknowledgedSequence = inputSequence;
		clockUncertain = true;
		if (clockRecheckPending || !online) return;
		clockRecheckNeeded = false;
		const owner = ++epoch;
		pending?.abort();
		pending = null;
		clockRecheckPending = true;
		void (async () => {
			let available = false;
			try {
				available = await deps.resyncClock();
			} catch {
				/* Preserve identity during unavailable validation. */
			}
			if (disposed || owner !== epoch) return;
			clockRecheckPending = false;
			clockUncertain = !available;
			unavailable = !available;
			schedule();
		})();
	}

	return {
		setSession(next: SessionState | null) {
			if (disposed) return;
			if (session?.sessionId !== next?.sessionId) {
				epoch++;
				pending?.abort();
				pending = null;
				acknowledgedSequence = inputSequence;
				failures = 0;
				retryAt = 0;
				unavailable = false;
				clockRecheckPending = false;
				clockUncertain = false;
				clockRecheckNeeded = false;
				wallReference = deps.wallNow();
				monotonicReference = deps.now();
			}
			if (
				next &&
				session?.sessionId === next.sessionId &&
				Date.parse(next.serverNow) < Date.parse(session.serverNow)
			)
				return;
			if (next !== session) {
				const estimatedNow = session?.sessionId === next?.sessionId ? serverNow() : 0;
				session = next;
				anchor = deps.now();
				serverAnchor = next ? Math.max(estimatedNow, Date.parse(next.serverNow)) : 0;
			}
			schedule();
		},
		trustedActivity() {
			if (disposed || !session || !visible || !online) return;
			checkClock();
			inputSequence++;
			lastInput = deps.now();
			schedule();
		},
		setEnvironment(nextVisible: boolean, nextOnline: boolean) {
			const wasOnline = online;
			if (!nextVisible || !nextOnline || visible !== nextVisible || online !== nextOnline) {
				// Returning to a page or network is not user input. Never replay offline activity.
				acknowledgedSequence = inputSequence;
			}
			visible = nextVisible;
			online = nextOnline;
			if (online && !wasOnline) checkClock();
			schedule();
		},
		wake() {
			// Focus/pageshow is never proof that input before a suspension is still recent.
			acknowledgedSequence = inputSequence;
			checkClock();
			schedule();
		},
		dispose() {
			disposed = true;
			epoch++;
			stopTimer();
			pending?.abort();
			pending = null;
			deps.warning(null);
		}
	};
}
