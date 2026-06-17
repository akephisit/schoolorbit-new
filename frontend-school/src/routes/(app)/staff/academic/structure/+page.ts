/**
 * Academic Structure Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'โครงสร้างวิชาการ',
		icon: 'Framer',
		group: 'academic',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_STRUCTURE
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
