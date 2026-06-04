/**
 * Departments Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'โครงสร้างองค์กร',
		icon: 'Building2',
		group: 'personnel',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ROLES
	}
};

export const load = async () => {
	return {
		title: 'จัดการฝ่าย'
	};
};
