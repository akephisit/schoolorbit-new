/**
 * Menu Administration Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดการเมนู',
		icon: 'Menu',
		group: 'settings',
		workspace: 'settings',
		order: 1001,
		user_type: 'staff',
		permission: PERMISSION_MODULES.MENU
	}
};

export const load = async () => {
	return {
		title: 'จัดการเมนู'
	};
};
