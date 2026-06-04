/**
 * Enrollment Management Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดห้องเรียน',
		icon: 'Users',
		group: 'academic',
		order: 30,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_ENROLLMENT_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
