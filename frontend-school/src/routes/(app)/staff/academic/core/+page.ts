import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ปีการศึกษา ภาคเรียน และเวลาเรียน',
		icon: 'CalendarRange',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_YEAR
	}
};

export const load = () => ({ title: _meta.menu.title });
