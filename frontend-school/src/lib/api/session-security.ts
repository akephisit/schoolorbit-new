const CSRF_HEADER = 'X-CSRF-Token';
const unsafeMethods = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);
let csrfToken: string | null = null;

export function captureSessionSecurityHeaders(headers: Headers): void {
	const value = headers.get(CSRF_HEADER)?.trim();
	if (value) csrfToken = value;
}

export function withSessionSecurityHeaders(method: string, headers: Headers): Headers {
	const result = new Headers(headers);
	result.delete(CSRF_HEADER);
	if (csrfToken && unsafeMethods.has(method.toUpperCase())) {
		result.set(CSRF_HEADER, csrfToken);
	}
	return result;
}

export function clearSessionSecurity(): void {
	csrfToken = null;
}

export function retryAfterSeconds(headers: Headers): number | undefined {
	const value = headers.get('Retry-After');
	if (!value || !/^\d+$/.test(value)) return undefined;
	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 30 ? parsed : undefined;
}
