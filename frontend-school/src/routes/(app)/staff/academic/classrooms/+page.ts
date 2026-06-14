/**
 * Classroom Management Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดการห้องเรียน',
		icon: 'LayoutGrid',
		group: 'academic',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_CLASSROOM_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
