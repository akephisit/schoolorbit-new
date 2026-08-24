import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ตั้งค่าปีและภาคเรียน',
		icon: 'CalendarRange',
		group: 'academic',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_YEAR
	}
};

export const load = () => ({ title: _meta.menu.title });
