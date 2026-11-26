import type { PageLoad } from './$types';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { loadNewLeaveRequest } from '#lib/staff-leave/route-data.js';

export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_LEAVE_CREATE_OWN }
};

export const load: PageLoad = (event) => loadNewLeaveRequest(event, 'exit');
