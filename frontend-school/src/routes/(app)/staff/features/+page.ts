/**
 * Feature Toggles Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดการระบบงาน',
		icon: 'Zap',
		group: 'settings',
		workspace: 'settings',
		order: 1000,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SETTINGS
	}
};

export const load = async () => {
	return {
		title: 'จัดการระบบงาน'
	};
};
