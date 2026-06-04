/**
 * Activity Groups Page (กิจกรรมพัฒนาผู้เรียน)
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'กิจกรรมพัฒนาผู้เรียน',
		icon: 'Users',
		group: 'academic',
		order: 7,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACTIVITY
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
