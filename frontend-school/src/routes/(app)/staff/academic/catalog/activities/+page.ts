import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ทะเบียนกิจกรรม',
		icon: 'Sparkles',
		group: 'academic',
		workspace: 'academic',
		order: 3,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load = () => ({ title: _meta.menu.title });
