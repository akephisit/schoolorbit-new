import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { getLeaveRequest } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.STAFF_LEAVE }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		title: 'รายละเอียดคำขอ',
		id: params.id,
		record: captureLeaveRead(
			() => get(can).hasModule(PERMISSION_MODULES.STAFF_LEAVE),
			() => getLeaveRequest(params.id, { requestFetch: fetch }),
			null
		)
	};
};
