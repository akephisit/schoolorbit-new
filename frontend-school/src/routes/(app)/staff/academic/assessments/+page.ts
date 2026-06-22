/**
 * Academic Assessment Structure Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'โครงสร้างคะแนน',
		icon: 'ClipboardList',
		group: 'academic',
		workspace: 'academic',
		order: 36,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_ASSESSMENT
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
