import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'จัดตารางสอน',
		icon: 'CalendarDays',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.LEARNING_OFFERING,
		order: 40,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
