export type AuthRefreshResult = 'authenticated' | 'unauthenticated' | 'unavailable';

export function authRefreshDecision(status: number): {
	result: AuthRefreshResult;
	clear: boolean;
} {
	if (status >= 200 && status < 300) return { result: 'authenticated', clear: false };
	if (status === 401) return { result: 'unauthenticated', clear: true };
	return { result: 'unavailable', clear: false };
}
