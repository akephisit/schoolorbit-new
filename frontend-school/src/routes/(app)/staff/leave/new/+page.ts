import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { getLeaveOptions } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_LEAVE_CREATE_OWN }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'สร้างคำขอ',
		kind: 'leave' as const,
		options: captureLeaveRead(
			() => get(can).has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN),
			() => getLeaveOptions({ requestFetch: fetch }),
			null
		)
	};
};
