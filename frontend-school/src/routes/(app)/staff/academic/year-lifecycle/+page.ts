import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'ปิดปีการศึกษา',
		icon: 'CalendarCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 70,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL
	}
};
export const load = () => ({ title: _meta.menu.title });
