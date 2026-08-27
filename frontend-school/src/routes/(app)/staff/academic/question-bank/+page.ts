import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'คลังข้อสอบ',
		icon: 'BookOpenCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_QUESTION_BANK
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
