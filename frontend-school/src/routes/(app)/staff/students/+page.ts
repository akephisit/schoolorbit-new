/**
 * Student Management Page (Staff)
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'รายชื่อนักเรียน',
		icon: 'GraduationCap',
		group: 'academic',
		order: 5, // Top priority in academic
		user_type: 'staff',
		permission: PERMISSIONS.STUDENT_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
