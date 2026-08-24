import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required',
	menu: {
		title: 'จัดการชุดและกลุ่มเรียน',
		icon: 'Workflow',
		group: 'academic',
		workspace: 'academic',
		order: 40,
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load = () => ({ title: _meta.menu.title });
