/**
 * Staff Management Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'บุคลากร',
		icon: 'Users',
		group: 'personnel',
		workspace: 'personnel',
		order: 10,
		user_type: 'staff',
		permission: PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
