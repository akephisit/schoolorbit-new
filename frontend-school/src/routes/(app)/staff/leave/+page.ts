import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { listLeaveRequests } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
import { leaveListFilters } from '#lib/staff-leave/filters.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'การลา',
		icon: 'CalendarOff',
		group: 'personnel',
		workspace: 'personnel',
		order: 510,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STAFF_LEAVE
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = leaveListFilters(url, 'leave');
	return {
		title: 'การลา',
		kind: 'leave' as const,
		query,
		requestKey: JSON.stringify(query),
		requests: captureLeaveRead(
			() => get(can).hasModule(PERMISSION_MODULES.STAFF_LEAVE),
			() => listLeaveRequests({ requestFetch: fetch, query }),
			null
		)
	};
};
