import type { PageLoad } from './$types';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { loadLeaveList } from '#lib/staff-leave/route-data.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ขออนุญาตออกนอกโรงเรียน',
		icon: 'DoorOpen',
		group: 'personnel',
		workspace: 'personnel',
		order: 520,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STAFF_LEAVE
	}
};

export const load: PageLoad = (event) => loadLeaveList(event, 'exit');
