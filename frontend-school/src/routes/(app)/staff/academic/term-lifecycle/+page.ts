import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ปิดและเปลี่ยนภาคเรียน',
		icon: 'CalendarCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 60,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL
	}
};
export const load = () => ({ title: _meta.menu.title });
