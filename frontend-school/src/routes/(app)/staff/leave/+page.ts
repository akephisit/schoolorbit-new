import type { PageLoad } from './$types';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { loadLeaveList } from '#lib/staff-leave/route-data.js';

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

export const load: PageLoad = (event) => loadLeaveList(event, 'leave');
