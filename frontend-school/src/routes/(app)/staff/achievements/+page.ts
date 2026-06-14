/**
 * Staff Achievements Management Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	menu: {
		title: 'บันทึกเกียรติบัตร',
		icon: 'Award',
		group: 'personnel',
		workspace: 'personnel',
		order: 30, // After "Manage Staff" (10)
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACHIEVEMENT
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
