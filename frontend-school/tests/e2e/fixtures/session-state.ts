import type { SessionState } from '../../../src/lib/api/auth.js';

/** Synthetic metadata required by the authenticated current-user/login contract. */
export function sessionState(
	now = Date.now(),
	overrides: Partial<SessionState> = {}
): SessionState {
	return {
		sessionId: '01900000-0000-7000-8000-000000000001',
		serverNow: new Date(now).toISOString(),
		idleExpiresAt: new Date(now + 2 * 60 * 60_000).toISOString(),
		absoluteExpiresAt: new Date(now + 12 * 60 * 60_000).toISOString(),
		nextRenewalAt: new Date(now + 5 * 60_000).toISOString(),
		...overrides
	};
}
