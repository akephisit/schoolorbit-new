import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { getLeaveOptions, getLeaveRequest } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_LEAVE_CREATE_OWN }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const allowed = () => get(can).has(PERMISSIONS.STAFF_LEAVE_CREATE_OWN);
	return {
		title: 'จัดเตรียมคำขอ',
		id: params.id,
		options: captureLeaveRead(allowed, () => getLeaveOptions({ requestFetch: fetch }), null),
		record: captureLeaveRead(
			allowed,
			() => getLeaveRequest(params.id, { requestFetch: fetch }),
			null
		)
	};
};
