import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'ทะเบียนรายวิชา',
		icon: 'LibraryBig',
		group: 'academic',
		workspace: 'academic',
		order: 2,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CATALOG
	}
};

export const load = () => ({ title: _meta.menu.title });
