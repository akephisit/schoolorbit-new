import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'หลักสูตรและแผนการเรียน',
		icon: 'BookCopy',
		group: 'academic',
		workspace: 'academic',
		order: 4,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load = () => ({ title: _meta.menu.title });
