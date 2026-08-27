import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'year_required',
	menu: {
		title: 'นักเรียนประจำปี',
		icon: 'UsersRound',
		group: 'academic_registry',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR
	}
};

export const load = () => ({ title: _meta.menu.title });
