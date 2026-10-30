import type { SessionState } from './session-activity.js';

export function sessionAnnouncementKey(origin: string) {
	return `schoolorbit:current-session:${origin}`;
}

/** Successful login/me is the sole source of cross-tab session identity announcements. */
export function announceAuthenticatedSession(session: SessionState): void {
	if (typeof window === 'undefined') return;
	const origin = window.location.origin;
	const value = { type: 'observed', sessionId: session.sessionId, serverNow: session.serverNow };
	try {
		const previous = window.localStorage.getItem(sessionAnnouncementKey(origin));
		if (previous) {
			const parsed: unknown = JSON.parse(previous);
			if (
				parsed &&
				typeof parsed === 'object' &&
				'serverNow' in parsed &&
				typeof parsed.serverNow === 'string' &&
				Date.parse(parsed.serverNow) > Date.parse(session.serverNow)
			)
				return;
		}
		window.localStorage.setItem(sessionAnnouncementKey(origin), JSON.stringify(value));
	} catch {
		/* BroadcastChannel also works when local storage is disabled. */
	}
	try {
		const channel = new BroadcastChannel(`schoolorbit:session-activity:${origin}`);
		channel.postMessage(value);
		channel.close();
	} catch {
		/* Storage events provide identity propagation on older browsers. */
	}
}
