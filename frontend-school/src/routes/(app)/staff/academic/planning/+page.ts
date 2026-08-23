/**
 * Course Planning Page
 */

import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดแผนการเรียน',
		icon: 'BookOpen',
		group: 'academic',
		workspace: 'academic',
		order: 30, // ถัดจาก ห้องเรียน (20)
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
