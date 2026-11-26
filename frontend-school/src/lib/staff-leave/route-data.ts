import type { LoadEvent } from '@sveltejs/kit';
import { get } from 'svelte/store';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import {
	getLeaveCoverageOptions,
	getLeaveOptions,
	listLeaveRequests,
	type LeaveKind
} from '#lib/api/staff-leave.js';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { leaveListFilters } from './filters.js';
import { leaveKindLabels, leavePageTitles } from './presentation.js';

export const leaveCompanionPermissions = [
	PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED,
	PERMISSIONS.STAFF_LEAVE_UPDATE_ASSIGNED
] as const;

export interface LeaveOwnedRead<T> {
	identity: string;
	value: T;
}

export function captureLeaveRead<T>(
	allowed: () => boolean,
	read: () => Promise<T>,
	fallback: T
): Promise<RouteLoadResult<LeaveOwnedRead<T>>> {
	return captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const identity = appIdentityKey();
			return {
				identity,
				value: user?.user_type === 'staff' && allowed() ? await read() : fallback
			};
		}),
		'โหลดข้อมูลไม่สำเร็จ'
	);
}

export function loadLeaveList(
	{ fetch, url, depends }: Pick<LoadEvent, 'fetch' | 'url' | 'depends'>,
	kind: LeaveKind
) {
	depends('school:app-identity');
	const query = leaveListFilters(url, kind);
	return {
		title: leavePageTitles[kind],
		kind,
		query,
		requestKey: JSON.stringify(query),
		requests: captureLeaveRead(
			() => get(can).hasModule(PERMISSION_MODULES.STAFF_LEAVE),
			() => listLeaveRequests({ requestFetch: fetch, query }),
			null
		)
	};
}

export function loadNewLeaveRequest(
	{ fetch, depends }: Pick<LoadEvent, 'fetch' | 'depends'>,
	kind: LeaveKind
) {
	depends('school:app-identity');
	const allowed = () => get(can).has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN);
	return {
		title: `สร้างคำขอ${leaveKindLabels[kind]}`,
		kind,
		coverageOptions: captureLeaveRead(
			allowed,
			() => getLeaveCoverageOptions({ requestFetch: fetch }),
			null
		),
		options: captureLeaveRead(allowed, () => getLeaveOptions({ requestFetch: fetch }), null)
	};
}

export async function leaveSource<T>(
	source: Promise<RouteLoadResult<LeaveOwnedRead<T>>>,
	identity: string
): Promise<RouteLoadResult<T>> {
	const result = await source;
	if (!result.ok) return result;
	if (!identity || identity !== result.data.identity)
		return { ok: false, data: null, error: 'บริบทผู้ใช้เปลี่ยนแปลง กรุณาเปิดหน้านี้อีกครั้ง' };
	return { ok: true, data: result.data.value, error: null };
}
