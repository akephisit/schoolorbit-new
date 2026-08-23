/**
 * Enrollment Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดห้องเรียน',
		icon: 'Users',
		group: 'academic',
		workspace: 'academic',
		order: 30,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
