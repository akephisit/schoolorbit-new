/**
 * Classroom Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดการห้องเรียน',
		icon: 'LayoutGrid',
		group: 'academic',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CLASSROOM
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
