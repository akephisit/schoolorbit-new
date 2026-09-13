import { PERMISSIONS } from '$lib/permissions/registry';
export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'เลื่อนชั้นและเตรียมปีใหม่',
		icon: 'GraduationCap',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 85,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL
	}
};
export const load = () => ({ title: _meta.menu.title });
