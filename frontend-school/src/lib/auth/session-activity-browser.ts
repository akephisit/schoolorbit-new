import { authAPI } from '#lib/api/auth.js';
import { ApiClientError } from '#lib/api/client.js';
import { clearSessionSecurity } from '#lib/api/session-security.js';
import { authStore } from '#lib/stores/auth.js';
import {
	createSessionActivityController,
	type SessionState,
	type SessionWarning
} from './session-activity.js';
import {
	createSessionActivityCoordinator,
	readSharedSession,
	sessionActivityKeys
} from './session-activity-coordination.js';
import { sessionAnnouncementKey } from './session-awareness.js';

/** Mounted once by the authenticated School layout, across staff, student and parent routes. */
export function mountSessionActivity(warning: (value: SessionWarning | null) => void) {
	const origin = window.location.origin;
	let storage: Storage | null;
	try {
		storage = window.localStorage;
	} catch {
		storage = null;
	}
	const coordinator = createSessionActivityCoordinator({
		storage,
		locks: navigator.locks,
		now: Date.now,
		random: Math.random,
		wait: (delay, signal) =>
			new Promise<void>((resolve, reject) => {
				if (signal.aborted) {
					reject(signal.reason);
					return;
				}
				const abort = () => {
					clearTimeout(timer);
					reject(signal.reason);
				};
				const timer = setTimeout(() => {
					signal.removeEventListener('abort', abort);
					resolve();
				}, delay);
				signal.addEventListener('abort', abort, { once: true });
			})
	});
	let channel: BroadcastChannel | null = null;
	try {
		channel = new BroadcastChannel(`schoolorbit:session-activity:${origin}`);
	} catch {
		/* Storage events provide the fallback. */
	}
	let current: SessionState | null = null;
	let currentEpoch = authStore.sessionEpoch;
	let disposed = false;
	let reconciled = false;
	let receivingLifecycle = false;
	const send = (value: object) => channel?.postMessage(value);
	const reconcile = async () => {
		if (reconciled || disposed) return;
		reconciled = true;
		await authAPI.refreshCurrentUser({ silent: true, invalidate: true });
	};
	const controller = createSessionActivityController({
		now: () => performance.now(),
		setTimer: (callback, delay) => setTimeout(callback, delay),
		clearTimer: clearTimeout,
		warning,
		accept: (session) => {
			authStore.updateSession(session, currentEpoch);
			coordinator.publish(origin, session);
			send({ type: 'metadata', session });
		},
		renew: (session, signal) =>
			coordinator.run(origin, session, signal, async () => {
				const timeout = AbortSignal.timeout(10_000);
				try {
					const next = await authAPI.recordActivity(session.sessionId, {
						signal: AbortSignal.any([signal, timeout])
					});
					reconciled = false;
					return { kind: 'renewed', session: next };
				} catch (error) {
					if (error instanceof ApiClientError) {
						if (error.status === 401) return { kind: 'unauthenticated' };
						if (error.status === 409 || error.status === 403) {
							// Refresh identity/CSRF once; never replay another mutation or this input.
							await reconcile();
							return { kind: 'reconciled' };
						}
						return { kind: 'retry', retryAfterMs: (error.retryAfterSeconds ?? 0) * 1_000 };
					}
					return { kind: 'retry' };
				}
			})
	});
	function lifecycle(sessionId: string, type: 'logout' | 'replaced') {
		const message = { type, sessionId };
		send(message);
		try {
			storage?.setItem(sessionActivityKeys(origin, sessionId).lifecycle, JSON.stringify(message));
		} catch {
			/* BroadcastChannel is sufficient where storage is disabled. */
		}
	}
	const unsubscribe = authStore.subscribe((state) => {
		const previous = current;
		const changed = currentEpoch !== authStore.sessionEpoch;
		currentEpoch = authStore.sessionEpoch;
		current = state.session;
		if (changed) {
			reconciled = false;
			if (previous && !receivingLifecycle)
				lifecycle(previous.sessionId, current ? 'replaced' : 'logout');
		}
		controller.setSession(state.isAuthenticated ? current : null);
	});
	const receive = (value: unknown) => {
		if (!value || typeof value !== 'object' || !('type' in value)) return;
		if (
			value.type === 'observed' &&
			'sessionId' in value &&
			typeof value.sessionId === 'string' &&
			'serverNow' in value &&
			typeof value.serverNow === 'string'
		) {
			if (
				current &&
				value.sessionId !== current.sessionId &&
				Date.parse(value.serverNow) >= Date.parse(current.serverNow)
			)
				void reconcile();
		} else if (value.type === 'metadata' && 'session' in value) {
			const next = readSharedSession(JSON.stringify(value.session));
			if (next && next.sessionId === current?.sessionId)
				authStore.updateSession(next, currentEpoch);
		} else if ('sessionId' in value && value.sessionId === current?.sessionId) {
			if (value.type === 'logout') {
				receivingLifecycle = true;
				clearSessionSecurity();
				authStore.clearUser();
				receivingLifecycle = false;
			} else if (value.type === 'replaced') void reconcile();
		}
	};
	if (channel) channel.onmessage = (event: MessageEvent<unknown>) => receive(event.data);
	const storageEvent = (event: StorageEvent) => {
		if (!current || !event.key) return;
		if (event.key === sessionAnnouncementKey(origin) && event.newValue) {
			try {
				const value: unknown = JSON.parse(event.newValue);
				receive(value);
			} catch {
				/* Ignore malformed identity announcements. */
			}
			return;
		}
		const keys = sessionActivityKeys(origin, current.sessionId);
		if (event.key === keys.metadata) {
			const next = readSharedSession(event.newValue);
			if (next?.sessionId === current.sessionId) authStore.updateSession(next, currentEpoch);
		} else if (event.key === keys.lifecycle && event.newValue) {
			try {
				const value: unknown = JSON.parse(event.newValue);
				receive(value);
			} catch {
				/* Ignore unrelated malformed storage. */
			}
		}
	};
	const environment = () =>
		controller.setEnvironment(document.visibilityState === 'visible', navigator.onLine);
	const wake = () => {
		environment();
		controller.wake();
	};
	const activity = (event: Event) => {
		if (event.isTrusted) controller.trustedActivity();
	};
	let lastPointerAt = -Infinity;
	let pointerX = NaN,
		pointerY = NaN;
	const pointerMove = (event: PointerEvent) => {
		if (!event.isTrusted || (event.clientX === pointerX && event.clientY === pointerY)) return;
		pointerX = event.clientX;
		pointerY = event.clientY;
		if (performance.now() - lastPointerAt < 1_000) return;
		lastPointerAt = performance.now();
		controller.trustedActivity();
	};
	const events = ['pointerdown', 'keydown', 'input', 'scroll', 'wheel'] as const;
	for (const name of events)
		document.addEventListener(name, activity, { capture: true, passive: true });
	document.addEventListener('pointermove', pointerMove, { capture: true, passive: true });
	document.addEventListener('visibilitychange', environment);
	window.addEventListener('pageshow', wake);
	window.addEventListener('focus', wake);
	window.addEventListener('online', environment);
	window.addEventListener('offline', environment);
	window.addEventListener('storage', storageEvent);
	environment();
	return {
		continueSession: (event: MouseEvent) => {
			if (event.isTrusted) controller.trustedActivity();
		},
		dispose() {
			disposed = true;
			unsubscribe();
			controller.dispose();
			channel?.close();
			for (const name of events) document.removeEventListener(name, activity, { capture: true });
			document.removeEventListener('pointermove', pointerMove, { capture: true });
			document.removeEventListener('visibilitychange', environment);
			window.removeEventListener('pageshow', wake);
			window.removeEventListener('focus', wake);
			window.removeEventListener('online', environment);
			window.removeEventListener('offline', environment);
			window.removeEventListener('storage', storageEvent);
		}
	};
}
