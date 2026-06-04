/**
 * Staff Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'บุคลากร',
		icon: 'Users',
		group: 'personnel',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STAFF
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
