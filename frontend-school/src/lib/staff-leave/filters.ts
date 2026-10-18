import type { LeaveKind, LeaveListQuery, LeaveStatus } from '#lib/api/staff-leave.js';

const statuses: LeaveStatus[] = [
	'draft',
	'submitted',
	'returned',
	'approved',
	'rejected',
	'cancelled',
	'cancellation_pending'
];
export const leavePageSize = 20;

export function leaveListFilters(url: URL, kind: LeaveKind): LeaveListQuery {
	const rawStatus = url.searchParams.get('status');
	const status = statuses.find((value) => value === rawStatus);
	const rawOffset = Number(url.searchParams.get('offset') ?? '0');
	const offset = Number.isSafeInteger(rawOffset) && rawOffset >= 0 ? rawOffset : 0;
	return { kind, status, limit: leavePageSize, offset };
}

export function leaveListDestination(path: string, status: string, offset = 0): string {
	const search = new URLSearchParams();
	if (statuses.some((value) => value === status)) search.set('status', status);
	if (offset) search.set('offset', String(offset));
	return `${path}${search.size ? `?${search}` : ''}`;
}

export function safeLeaveReturn(value: string | null, fallback: string): string {
	if (!value) return fallback;
	try {
		const url = new URL(value, 'https://leave.invalid');
		if (
			url.origin !== 'https://leave.invalid' ||
			!['/staff/leave', '/staff/exit-permissions'].includes(url.pathname)
		)
			return fallback;
		return `${url.pathname}${url.search}`;
	} catch {
		return fallback;
	}
}
