/**
 * Academic Structure Management Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'โครงสร้างวิชาการ',
		icon: 'Framer',
		group: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_STRUCTURE_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
