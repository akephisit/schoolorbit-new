import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตารางสอบ',
		icon: 'CalendarClock',
		group: 'academic',
		workspace: 'academic',
		permission: PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL,
		order: 52,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
