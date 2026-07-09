import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คลังข้อสอบ',
		icon: 'BookOpenCheck',
		group: 'academic',
		workspace: 'academic',
		order: 54,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_QUESTION_BANK
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
