import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required',
	menu: {
		title: 'รายวิชาและกิจกรรมที่เปิดสอน',
		icon: 'Workflow',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load = () => ({ title: _meta.menu.title });
