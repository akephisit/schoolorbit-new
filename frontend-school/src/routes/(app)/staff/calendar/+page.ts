import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_optional' as const,
	menu: {
		title: 'ปฏิทินโรงเรียน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 7,
		user_type: 'staff',
		permission: PERMISSION_MODULES.CALENDAR
	}
};

export const load = async () => ({ title: _meta.menu.title });
