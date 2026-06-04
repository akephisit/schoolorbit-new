/**
 * Subject Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คลังรายวิชา',
		icon: 'BookOpen',
		group: 'academic',
		order: 2,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
