/**
 * Staff Achievements Management Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	menu: {
		title: 'บันทึกเกียรติบัตร',
		icon: 'Award',
		group: 'personnel',
		order: 30, // After "Manage Staff" (10)
		user_type: 'staff',
		permission: PERMISSIONS.ACHIEVEMENT_READ_ALL
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
