import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดตารางสอน',
		icon: 'CalendarDays',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSIONS.ACADEMIC_COURSE_PLAN_MANAGE_ALL,
		order: 51,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
