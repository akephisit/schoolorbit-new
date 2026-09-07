import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ล็อกผลการเรียน',
		icon: 'LockKeyhole',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 40,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_RESULT_LOCK_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL
		]
	}
};

export const load = async () => ({ title: _meta.menu.title });
