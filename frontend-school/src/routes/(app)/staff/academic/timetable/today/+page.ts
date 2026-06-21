import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตารางสอนวันนี้',
		icon: 'CalendarClock',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY,
		order: 50,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
