import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดตารางสอน',
		icon: 'CalendarDays',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_COURSE_PLAN,
		order: 51,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
