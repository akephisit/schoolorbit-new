import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'แก้ผลการเรียน',
		icon: 'History',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 50,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_RESULT_CORRECT_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL
		]
	}
};

export const load = async () => ({ title: _meta.menu.title });
