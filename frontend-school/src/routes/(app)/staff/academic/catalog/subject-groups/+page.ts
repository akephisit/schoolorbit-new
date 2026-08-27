import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'กลุ่มสาระการเรียนรู้',
		icon: 'Layers3',
		group: 'academic_curriculum',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load = () => ({ title: _meta.menu.title });
