import type { AuthRefreshResult } from '#lib/auth/auth-refresh-policy.js';

export async function realtimeAuthRecovery(
	refresh: () => Promise<AuthRefreshResult>
): Promise<'reconnect' | 'retry' | 'stop'> {
	const result = await refresh();
	if (result === 'authenticated') return 'reconnect';
	if (result === 'unavailable') return 'retry';
	return 'stop';
}
