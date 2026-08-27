import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอนวันนี้',
		icon: 'CalendarClock',
		group: 'academic_delivery',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY,
		order: 30,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
