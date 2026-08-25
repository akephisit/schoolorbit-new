/**
 * Student Management Page (Staff)
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'year_required',
	menu: {
		title: 'รายชื่อนักเรียน',
		icon: 'GraduationCap',
		group: 'academic',
		workspace: 'academic',
		order: 5, // Top priority in academic
		user_type: 'staff',
		permission: PERMISSION_MODULES.STUDENT
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
