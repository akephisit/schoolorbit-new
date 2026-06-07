/**
 * School Organization Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'โครงสร้างโรงเรียน',
		icon: 'Building2',
		group: 'personnel',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ROLES
	}
};

export const load = async () => {
	return {
		title: 'โครงสร้างโรงเรียน'
	};
};
