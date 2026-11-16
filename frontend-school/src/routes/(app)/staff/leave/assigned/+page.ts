import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { listAssignedLeaveCoverage } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const raw = Number(url.searchParams.get('offset') ?? '0');
	const offset = Number.isSafeInteger(raw) && raw >= 0 ? raw : 0;
	return {
		title: 'คาบสอนที่มอบหมายให้ฉัน',
		offset,
		tasks: captureLeaveRead(
			() => get(can).has(PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED),
			() => listAssignedLeaveCoverage({ requestFetch: fetch, query: { limit: 20, offset } }),
			null
		)
	};
};
