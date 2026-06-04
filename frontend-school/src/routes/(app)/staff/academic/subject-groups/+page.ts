import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'กลุ่มสาระการเรียนรู้',
		icon: 'GraduationCap',
		group: 'academic',
		order: 1,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
