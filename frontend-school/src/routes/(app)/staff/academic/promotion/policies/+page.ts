import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'เกณฑ์การเลื่อนชั้น',
		icon: 'ListChecks',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 80,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL
	}
};
export const load = () => ({ title: _meta.menu.title });
